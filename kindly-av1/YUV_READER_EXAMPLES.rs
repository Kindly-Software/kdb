//! YUV/Y4M Reader Usage Examples
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! This file contains practical examples of using the YUV/Y4M reader
//! in kindly-av1. Copy these patterns into your encoder implementation.

#![allow(dead_code, unused_imports, unused_variables)]

use kindly_av1::file::{
    create_reader, detect_format, discover_videos, Frame, FrameReader, InputFormat, PixelFormat,
    RawYuvReader, VideoInfo, Y4mReader,
};
use std::path::Path;

// ============================================================================
// Example 1: Basic Y4M File Reading
// ============================================================================

/// Read and process all frames from a Y4M file
fn example_y4m_basic() -> Result<(), Box<dyn std::error::Error>> {
    // Open Y4M file (self-describing, no dimensions needed)
    let mut reader = Y4mReader::open("input.y4m")?;

    // Get video information
    let info = reader.info();
    println!(
        "Video: {}x{} @ {:.2} fps",
        info.width, info.height, info.frame_rate
    );
    println!(
        "Frames: {}, Duration: {:.2}s",
        info.frame_count, info.duration_secs
    );

    // Read frames sequentially
    let mut frame_count = 0;
    while let Some(frame) = reader.read_frame()? {
        println!(
            "Frame {}: Y={} U={} V={} bytes",
            frame.frame_num,
            frame.y.len(),
            frame.u.len(),
            frame.v.len()
        );

        // Process frame (e.g., encode)
        process_frame(&frame)?;

        frame_count += 1;
    }

    println!("Processed {} frames", frame_count);
    Ok(())
}

// ============================================================================
// Example 2: Raw YUV File Reading
// ============================================================================

/// Read raw YUV file with explicit dimensions
fn example_raw_yuv() -> Result<(), Box<dyn std::error::Error>> {
    // Raw YUV requires explicit dimensions
    let width = 1920;
    let height = 1080;
    let pixel_format = PixelFormat::Yuv420p;
    let frame_rate = 30.0;

    let mut reader = RawYuvReader::open("input.yuv", width, height, pixel_format, frame_rate)?;

    // Read all frames
    while let Some(frame) = reader.read_frame()? {
        process_frame(&frame)?;
    }

    Ok(())
}

// ============================================================================
// Example 3: Auto-Detection and Dynamic Reader Creation
// ============================================================================

/// Automatically detect format and create appropriate reader
fn example_auto_detect(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Detect format from file extension
    let format = detect_format(path).ok_or("Unknown format")?;

    println!("Detected format: {:?}", format);

    // Create reader (requires dimensions for raw YUV)
    let raw_config = if format == InputFormat::RawYuv {
        Some((1920, 1080, PixelFormat::Yuv420p, 30.0))
    } else {
        None
    };

    let mut reader: Box<dyn FrameReader> = create_reader(path, format, raw_config)?;

    // Read frames
    while let Some(frame) = reader.read_frame()? {
        process_frame(&frame)?;
    }

    Ok(())
}

// ============================================================================
// Example 4: Frame Seeking (Resume Capability)
// ============================================================================

/// Seek to a specific frame and resume encoding
fn example_seeking() -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = Y4mReader::open("input.y4m")?;

    // Simulate checkpoint: last encoded frame was 42
    let resume_frame = 43;

    // Seek to resume point
    reader.seek(resume_frame)?;
    println!("Resumed from frame {}", resume_frame);

    // Continue encoding
    while let Some(frame) = reader.read_frame()? {
        process_frame(&frame)?;

        // Periodic checkpoint (every 60 frames)
        if frame.frame_num % 60 == 0 {
            save_checkpoint(frame.frame_num)?;
        }
    }

    Ok(())
}

// ============================================================================
// Example 5: Encoding Loop with Progress
// ============================================================================

/// Full encoding loop with progress reporting
fn example_encoding_loop() -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = Y4mReader::open("input.y4m")?;
    let info = reader.info();

    println!("Encoding {} frames...", info.frame_count);

    let mut encoded_count = 0;
    let start_time = std::time::Instant::now();

    while let Some(frame) = reader.read_frame()? {
        // Encode frame
        let encoded_data = encode_frame(&frame)?;

        // Write to output
        write_encoded_frame(&encoded_data)?;

        encoded_count += 1;

        // Progress report every 30 frames (1 second @ 30fps)
        if encoded_count % 30 == 0 {
            let elapsed = start_time.elapsed().as_secs_f64();
            let fps = encoded_count as f64 / elapsed;
            let progress = (encoded_count as f64 / info.frame_count as f64) * 100.0;

            println!(
                "Progress: {:.1}% ({}/{}) @ {:.1} fps",
                progress, encoded_count, info.frame_count, fps
            );
        }
    }

    let total_time = start_time.elapsed().as_secs_f64();
    let avg_fps = encoded_count as f64 / total_time;

    println!(
        "Done! Encoded {} frames in {:.2}s ({:.1} fps)",
        encoded_count, total_time, avg_fps
    );

    Ok(())
}

// ============================================================================
// Example 6: Multi-File Processing
// ============================================================================

/// Process multiple video files in a directory
fn example_batch_processing(dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Discover all video files
    let videos = discover_videos(dir);

    println!("Found {} video files", videos.len());

    for video in videos {
        println!(
            "\nProcessing: {} ({}, {})",
            video.filename,
            video.format,
            video.size_display()
        );

        let mut reader: Box<dyn FrameReader> = create_reader(&video.path, video.format, None)?;

        let info = reader.info();
        println!(
            "  Resolution: {}x{} @ {:.2} fps",
            info.width, info.height, info.frame_rate
        );

        // Process all frames
        let mut count = 0;
        while let Some(frame) = reader.read_frame()? {
            process_frame(&frame)?;
            count += 1;
        }

        println!("  Processed {} frames", count);
    }

    Ok(())
}

// ============================================================================
// Example 7: Error Handling Patterns
// ============================================================================

/// Robust error handling for various failure modes
fn example_error_handling(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    use kindly_av1::file::FileError;

    // Attempt to open file
    let reader_result = Y4mReader::open(path);

    match reader_result {
        Ok(mut reader) => {
            // Success path
            while let Some(frame) = reader.read_frame()? {
                // Handle frame read errors
                match process_frame(&frame) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Frame processing error: {}", e);
                        // Continue or abort based on severity
                        continue;
                    }
                }
            }
        }
        Err(e) => {
            // Specific error handling
            match e {
                FileError::NotFound(_) => {
                    eprintln!("File not found: {}", path);
                    return Err("File not found".into());
                }
                FileError::PermissionDenied(_) => {
                    eprintln!("Permission denied: {}", path);
                    return Err("Permission denied".into());
                }
                FileError::InvalidY4mHeader { path, details } => {
                    eprintln!("Invalid Y4M header in {:?}: {}", path, details);
                    return Err("Invalid Y4M header".into());
                }
                _ => {
                    eprintln!("Unknown error: {}", e);
                    return Err(e.into());
                }
            }
        }
    }

    Ok(())
}

// ============================================================================
// Example 8: Frame Buffer Reuse (Memory Optimization)
// ============================================================================

/// Reuse frame buffers to reduce allocations
fn example_buffer_reuse() -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = Y4mReader::open("input.y4m")?;

    // Pre-allocate frame buffer
    let info = reader.info();
    let mut frame_buffer = Frame::new_uninit(info.width, info.height, info.pixel_format, 0);

    while let Some(frame) = reader.read_frame()? {
        // Copy into reused buffer (manual implementation needed)
        // This would require modifying read_frame() to accept &mut Frame
        // For now, just process the frame
        process_frame(&frame)?;
    }

    Ok(())
}

// ============================================================================
// Example 9: Parallel Frame Processing
// ============================================================================

/// Read frames sequentially but process in parallel
fn example_parallel_processing() -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::mpsc;
    use std::thread;

    let mut reader = Y4mReader::open("input.y4m")?;

    // Create channel for frame communication
    let (tx, rx) = mpsc::channel::<Frame>();

    // Spawn worker threads
    let num_workers = 4;
    let mut workers = Vec::new();

    for worker_id in 0..num_workers {
        let rx = rx.clone();
        let worker = thread::spawn(move || {
            for frame in rx {
                println!("Worker {} processing frame {}", worker_id, frame.frame_num);
                // Process frame
                let _ = process_frame(&frame);
            }
        });
        workers.push(worker);
    }

    // Read and distribute frames
    while let Some(frame) = reader.read_frame()? {
        tx.send(frame)?;
    }

    // Signal completion
    drop(tx);

    // Wait for workers
    for worker in workers {
        worker.join().unwrap();
    }

    Ok(())
}

// ============================================================================
// Example 10: Streaming Input (Non-File Sources)
// ============================================================================

/// Read from stdin pipe (e.g., ffmpeg | kindly-av1)
fn example_stdin_pipe() -> Result<(), Box<dyn std::error::Error>> {
    // This would require implementing a StdinYuvReader
    // For now, this is a placeholder showing the pattern

    use std::io::{stdin, BufReader};

    let stdin = stdin();
    let reader = BufReader::new(stdin);

    // Parse Y4M header from first line
    // Then read frames in a loop
    // Similar to Y4mReader but using stdin instead of File

    println!("Reading Y4M from stdin...");
    // Implementation would go here

    Ok(())
}

// ============================================================================
// Helper Functions (Stubs for Examples)
// ============================================================================

fn process_frame(frame: &Frame) -> Result<(), Box<dyn std::error::Error>> {
    // Placeholder: actual encoding logic would go here
    Ok(())
}

fn encode_frame(frame: &Frame) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Placeholder: actual encoding logic
    Ok(vec![])
}

fn write_encoded_frame(data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    // Placeholder: write to output file
    Ok(())
}

fn save_checkpoint(frame_num: u64) -> Result<(), Box<dyn std::error::Error>> {
    // Placeholder: save checkpoint to disk
    println!("Checkpoint saved at frame {}", frame_num);
    Ok(())
}

// ============================================================================
// Integration with Encoder Orchestrator
// ============================================================================

/// Example integration with KindlyAv1CliMetacapsule
#[cfg(feature = "example-integration")]
fn example_orchestrator_integration() -> Result<(), Box<dyn std::error::Error>> {
    use kindly_av1::encoder::KindlyAv1CliMetacapsule;

    // Initialize encoder metacapsule
    let mut encoder = KindlyAv1CliMetacapsule::new();

    // Open input file
    let input_path = "input.y4m";
    let format = detect_format(input_path).ok_or("Unknown format")?;
    let mut reader: Box<dyn FrameReader> = create_reader(input_path, format, None)?;

    // Get video info
    let info = reader.info();
    println!(
        "Encoding {}x{} @ {:.2} fps",
        info.width, info.height, info.frame_rate
    );

    // Encoding loop
    while let Some(frame) = reader.read_frame()? {
        // Convert Frame to encoder's internal format
        // let encoder_frame = convert_to_encoder_format(&frame);

        // Encode via wiring
        // encoder.wiring.encode_frame(&encoder_frame)?;

        // Update progress
        // encoder.progress.increment_frames_encoded();
    }

    // Finalize
    // encoder.finalize()?;

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_allocation() {
        let frame = Frame::new_uninit(1920, 1080, PixelFormat::Yuv420p, 0);
        assert_eq!(frame.y.len(), 1920 * 1080);
        assert_eq!(frame.u.len(), 960 * 540);
        assert_eq!(frame.v.len(), 960 * 540);
    }

    #[test]
    fn test_format_detection() {
        assert_eq!(detect_format("video.y4m"), Some(InputFormat::Y4m));
        assert_eq!(detect_format("video.yuv"), Some(InputFormat::RawYuv));
        assert_eq!(detect_format("video.mp4"), Some(InputFormat::Mp4));
        assert_eq!(detect_format("video.mkv"), Some(InputFormat::Mkv));
        assert_eq!(detect_format("video.unknown"), None);
    }
}

// ============================================================================
// Main Function (Run Examples)
// ============================================================================

#[cfg(not(test))]
fn main() {
    println!("YUV/Y4M Reader Examples\n");

    // Run examples (comment out as needed)
    // example_y4m_basic().unwrap();
    // example_raw_yuv().unwrap();
    // example_auto_detect("input.y4m").unwrap();
    // example_seeking().unwrap();
    // example_encoding_loop().unwrap();
    // example_batch_processing(".").unwrap();
    // example_error_handling("input.y4m").unwrap();

    println!("\nExamples completed. See source code for usage patterns.");
}
