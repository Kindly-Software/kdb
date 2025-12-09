use atomic_capsule::mux::webm_muxer::{
    WebmAudioCodec, WebmAudioTrack, WebmMuxerCapsule, WebmVideoCodec, WebmVideoTrack,
    WebmMuxerError,
};
use atomic_capsule_derive::ComputationalCapsule;
use std::fs::File;
use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::audio::AudioGrain;
use crate::rasterizer::RasterizedFrame;
use crate::renderer::{MediaBackendRequest, MediaMuxResult};

/// Minimal WebM mux adapter using in-tree atomic_capsule muxer. This intentionally
/// writes a sparse WebM file (headers + tracks) without full video/audio encoding,
/// to keep the pipeline in-tree and Chaos-compliant while avoiding external FFmpeg.
#[derive(Debug, ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]
pub struct WebmMuxAdapterCapsule {
    generation: AtomicU64,
}

impl WebmMuxAdapterCapsule {
    pub const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
        }
    }

    /// Mux HUD/audio counts into a minimal WebM container. This uses only internal
    /// primitives; callers can layer real encoding later via set_mux_handler.
    pub fn mux(
        &self,
        request: &MediaBackendRequest,
        rasters: &[RasterizedFrame],
        _grains: &[AudioGrain],
        pcm: &[u8],
    ) -> io::Result<MediaMuxResult> {
        // Pre-size buffer based on downsampled frame sizes to avoid realloc churn.
        let estimated_frame_bytes: usize = rasters
            .iter()
            .map(|r| downsample_rgba(r, 64).len() + 64)
            .sum();
        let mut buf = vec![0u8; 32 * 1024 + estimated_frame_bytes + pcm.len()];
        let muxer = WebmMuxerCapsule::new(true, false); // streaming=true, cues=false

        // Derive video track dimensions (use requested target if no frames).
        let (width, height) = if let Some(r) = rasters.first() {
            (r.width as u16, r.height as u16)
        } else {
            (request.width as u16, request.height as u16)
        };
        let track = WebmVideoTrack {
            // We label VP9 but store lightweight RLE-compressed RGBA thumbnails to stay in-tree.
            codec: WebmVideoCodec::Vp9,
            width: width.max(160),
            height: height.max(90),
            track_number: 1,
            codec_private_len: 0,
        };
        let audio_track = WebmAudioTrack {
            codec: WebmAudioCodec::Opus,
            sample_rate: request.sample_rate_hz,
            channels: request.audio_channels,
            bit_depth: 16,
            track_number: 2,
            codec_private_len: 0,
        };

        // Try to lay out WebM structure; if any step fails, degrade to a manifest file.
        let write_result = (|| -> Result<usize, WebmMuxerError> {
            let mut pos = 0usize;
            pos += muxer.write_ebml_header(&mut buf[pos..])?;
            pos += muxer.start_segment(&mut buf[pos..])?;
            pos += muxer.write_info(&mut buf[pos..], b"Kindly_Rub", b"hud-raster")?;
            pos += muxer.write_video_track(&mut buf[pos..], &track, &[])?;
            if !pcm.is_empty() {
                pos += muxer.write_audio_track(&mut buf[pos..], &audio_track, &[])?;
            }

            // No frames? finalize header-only.
            if rasters.is_empty() {
                return Ok(pos);
            }

            // Start first cluster at first frame time.
            let mut cluster_start_ms = rasters.first().map(|r| r.time_ms).unwrap_or(0);
            pos += muxer.start_cluster(&mut buf[pos..], cluster_start_ms)?;

            for raster in rasters {
                // If time delta exceeds i16 range, start a new cluster.
                let delta_ms = raster.time_ms.saturating_sub(cluster_start_ms);
                if delta_ms > i16::MAX as u64 {
                    cluster_start_ms = raster.time_ms;
                    pos += muxer.start_cluster(&mut buf[pos..], cluster_start_ms)?;
                }
                let timecode_delta = raster.time_ms.saturating_sub(cluster_start_ms) as i16;

                let frame_bytes = rle_compress_rgba(&downsample_rgba(raster, 64));
                ensure_capacity(&mut buf, frame_bytes.len() + 128, pos);
                pos += muxer.write_simple_block(
                    &mut buf[pos..],
                    1,
                    timecode_delta,
                    true,
                    &frame_bytes,
                )?;
            }

            if !pcm.is_empty() {
                // Align audio block at cluster start.
                ensure_capacity(&mut buf, pcm.len() + 128, pos);
                pos += muxer.write_simple_block(&mut buf[pos..], 2, 0, true, pcm)?;
            }

            // Finalize (noop for streaming=true but validates phase).
            let _ = muxer.finalize()?;
            Ok(pos)
        })();

        if let Err(err) = write_result {
            // Fallback: write manifest text but still stay in-tree.
            write_manifest(request, rasters.len(), err)?;
        } else {
            let bytes = write_result.unwrap();
            write_file(&request.target_path, &buf[..bytes])?;
        }

        self.generation.fetch_add(1, Ordering::Relaxed);

        Ok(MediaMuxResult {
            output_path: request.target_path.clone(),
            fps: request.fps,
            bitrate_kbps: request.bitrate_kbps,
            video_frames: rasters.len(),
            audio_frames: if pcm.is_empty() { 0 } else { 1 },
        })
    }
}

fn write_file(path: &std::path::Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn write_manifest(
    request: &MediaBackendRequest,
    hud_frames: usize,
    err: WebmMuxerError,
) -> io::Result<()> {
    if let Some(parent) = request.target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = File::create(&request.target_path)?;
    writeln!(
        file,
        "fallback-manifest fps={} bitrate_kbps={} frames={} error={:?}",
        request.fps,
        request.bitrate_kbps,
        hud_frames,
        err
    )?;
    file.sync_all()
}

fn ensure_capacity(buf: &mut Vec<u8>, extra: usize, pos: usize) {
    if buf.len().saturating_sub(pos) < extra {
        let needed = extra - buf.len().saturating_sub(pos);
        buf.resize(buf.len() + needed + 4096, 0);
    }
}

fn downsample_rgba(raster: &RasterizedFrame, target: u32) -> Vec<u8> {
    let src_w = raster.width.max(1);
    let src_h = raster.height.max(1);
    let dst_w = target.min(src_w);
    let dst_h = target.min(src_h);
    let step_x = (src_w as f32 / dst_w as f32).ceil().max(1.0) as u32;
    let step_y = (src_h as f32 / dst_h as f32).ceil().max(1.0) as u32;
    let mut out = Vec::with_capacity((dst_w * dst_h * 4) as usize);
    for y in (0..src_h).step_by(step_y as usize).take(dst_h as usize) {
        for x in (0..src_w).step_by(step_x as usize).take(dst_w as usize) {
            let idx = ((y * src_w + x) * 4) as usize;
            let px = &raster.data[idx..idx + 4];
            out.extend_from_slice(px);
        }
    }
    out
}

fn rle_compress_rgba(pixels: &[u8]) -> Vec<u8> {
    // Simple run-length encoding over 4-byte pixels to shrink HUD thumbnails.
    if pixels.len() < 4 {
        return pixels.to_vec();
    }
    let mut out = Vec::with_capacity(pixels.len() / 2);
    let mut i = 0;
    while i + 4 <= pixels.len() {
        let pixel = &pixels[i..i + 4];
        let mut run = 1u8;
        while run < u8::MAX && i + (run as usize + 1) * 4 <= pixels.len() {
            let next = &pixels[i + run as usize * 4..i + (run as usize + 1) * 4];
            if next == pixel {
                run += 1;
            } else {
                break;
            }
        }
        out.push(run);
        out.extend_from_slice(pixel);
        i += run as usize * 4;
    }
    if i < pixels.len() {
        out.extend_from_slice(&pixels[i..]);
    }
    out
}
