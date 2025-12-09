use atomic_capsule::verify_capsule_properties;
use core::sync::atomic::{AtomicU64, Ordering};
use std::io::{Error as IoError, ErrorKind, Result as IoResult, Write};

use crate::tick::RenderSoaView;
use crate::world::{UnitCapsule, WORLD_PAGE_SIZE};

/// Streaming capsule to serialize frames to an external sink (io_uring/pinned buffer ready).
///
/// Example (io_uring NVMe write, feature `io-uring`):
/// ```ignore
/// use atomic_capsule::runtime::IoUringCapsule;
/// use kindly_engine::{FrameStreamCapsule, collect_world_render_slab};
///
/// // Build uring and register a pinned buffer (kernel-side registration handled externally)
/// let uring = IoUringCapsule::new(256, 0)?;
/// let mut buf = [0u8; 4096]; // registered/pinned buffer sized for frame
/// let stream = FrameStreamCapsule::new();
/// let view = collect_world_render_slab(&[&formations[..]], &mut render_slab)?;
/// let fd = /* open NVMe file */ 3;
/// let offset = 0;
/// stream.submit_render_frame_uring(&view, uring.batch(), fd, offset, &mut buf)?;
/// ```
#[repr(C, align(64))]
pub struct FrameStreamCapsule {
    frames_written: AtomicU64,
    bytes_written: AtomicU64,
    _padding: [u8; 48],
}

impl FrameStreamCapsule {
    pub const fn new() -> Self {
        Self {
            frames_written: AtomicU64::new(0),
            bytes_written: AtomicU64::new(0),
            _padding: [0; 48],
        }
    }

    /// Encode a render frame to the writer: [frame_len][entries...], little-endian.
    pub fn write_render_frame<W: Write>(&self, view: &RenderSoaView<'_>, mut w: W) -> IoResult<()> {
        let len = view.total_len as u64;
        w.write_all(&len.to_le_bytes())?;
        for page in &view.pages {
            for idx in 0..page.formation_ids.len() {
                w.write_all(&page.formation_ids[idx].to_le_bytes())?;
                w.write_all(&page.position_x_q16[idx].to_le_bytes())?;
                w.write_all(&page.position_z_q16[idx].to_le_bytes())?;
            }
        }
        self.frames_written.fetch_add(1, Ordering::AcqRel);
        self.bytes_written.fetch_add(8 + len * 12, Ordering::AcqRel); // rough accounting
        Ok(())
    }

    /// Encode a render frame into a caller-provided (pinned/registered) buffer.
    /// Returns the number of bytes written or an error if the buffer is too small.
    pub fn write_render_frame_into(
        &self,
        view: &RenderSoaView<'_>,
        buf: &mut [u8],
    ) -> IoResult<usize> {
        let required = 8u64
            .saturating_add(view.total_len as u64 * 12)
            .try_into()
            .unwrap_or(usize::MAX);
        if buf.len() < required {
            return Err(IoError::new(
                ErrorKind::WriteZero,
                "buffer too small for render frame",
            ));
        }
        buf[..8].copy_from_slice(&(view.total_len as u64).to_le_bytes());
        let mut cursor = 8;
        for page in &view.pages {
            for idx in 0..page.formation_ids.len() {
                buf[cursor..cursor + 4].copy_from_slice(&page.formation_ids[idx].to_le_bytes());
                cursor += 4;
                buf[cursor..cursor + 4].copy_from_slice(&page.position_x_q16[idx].to_le_bytes());
                cursor += 4;
                buf[cursor..cursor + 4].copy_from_slice(&page.position_z_q16[idx].to_le_bytes());
                cursor += 4;
            }
        }
        self.frames_written.fetch_add(1, Ordering::AcqRel);
        self.bytes_written
            .fetch_add(required as u64, Ordering::AcqRel);
        Ok(cursor)
    }

    /// Encode a world slab page to the writer (AoS), useful for checkpoints.
    pub fn write_world_page<W: Write>(&self, page: &[UnitCapsule], mut w: W) -> IoResult<()> {
        for unit in page.iter().take(WORLD_PAGE_SIZE) {
            let snap = unit.snapshot();
            w.write_all(&snap.pos_x_q16.to_le_bytes())?;
            w.write_all(&snap.pos_z_q16.to_le_bytes())?;
            w.write_all(&snap.heading_deg_q16.to_le_bytes())?;
            w.write_all(&snap.regiment_id.to_le_bytes())?;
            w.write_all(&[snap.state])?;
            w.write_all(&[snap.bloodiness])?;
        }
        Ok(())
    }

    /// Encode a world slab slice into a caller-provided buffer (pinned/registered friendly).
    pub fn write_world_slice_into(&self, units: &[UnitCapsule], buf: &mut [u8]) -> IoResult<usize> {
        let len = units.len().min(WORLD_PAGE_SIZE);
        let required = len
            .checked_mul(14)
            .ok_or_else(|| IoError::new(ErrorKind::WriteZero, "overflow"))?;
        if buf.len() < required {
            return Err(IoError::new(
                ErrorKind::WriteZero,
                "buffer too small for world slice",
            ));
        }
        let mut cursor = 0;
        for unit in units.iter().take(len) {
            let snap = unit.snapshot();
            buf[cursor..cursor + 4].copy_from_slice(&snap.pos_x_q16.to_le_bytes());
            cursor += 4;
            buf[cursor..cursor + 4].copy_from_slice(&snap.pos_z_q16.to_le_bytes());
            cursor += 4;
            buf[cursor..cursor + 4].copy_from_slice(&snap.heading_deg_q16.to_le_bytes());
            cursor += 4;
            buf[cursor..cursor + 2].copy_from_slice(&snap.regiment_id.to_le_bytes());
            cursor += 2;
            buf[cursor] = snap.state;
            cursor += 1;
            buf[cursor] = snap.bloodiness;
            cursor += 1;
        }
        Ok(cursor)
    }

    pub fn snapshot(&self) -> FrameStreamSnapshot {
        FrameStreamSnapshot {
            frames_written: self.frames_written.load(Ordering::Relaxed),
            bytes_written: self.bytes_written.load(Ordering::Relaxed),
        }
    }
}

verify_capsule_properties!(FrameStreamCapsule, 64, 64);

#[derive(Debug, Clone, Copy)]
pub struct FrameStreamSnapshot {
    pub frames_written: u64,
    pub bytes_written: u64,
}

#[cfg(feature = "io-uring")]
mod uring_bridge {
    use super::*;
    use atomic_capsule::runtime::{IoUringBatchCapsule, IoUringError};

    impl FrameStreamCapsule {
        /// Encode and queue a render frame for `io_uring` NVMe write (single batch entry).
        pub fn submit_render_frame_uring(
            &self,
            view: &RenderSoaView<'_>,
            uring: &IoUringBatchCapsule,
            fd: i32,
            offset: u64,
            buf: &mut [u8],
        ) -> Result<u64, IoUringError> {
            let used = self
                .write_render_frame_into(view, buf)
                .map_err(|_| IoUringError::InvalidParameters)?;
            let ids = uring.batch_write(&[fd], &[&buf[..used]], &[offset])?;
            Ok(ids[0])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formation::FormationCapsule;
    use crate::tick::{collect_world_render_slab, RenderSoaSlabCapsule};

    #[test]
    fn frame_stream_writes_counts() {
        let mut slab = RenderSoaSlabCapsule::new(1, 1);
        let shard = [FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0)];
        let shards = [&shard[..]];
        let view = collect_world_render_slab(&shards, &mut slab).unwrap();
        let stream = FrameStreamCapsule::new();
        let mut buf: Vec<u8> = Vec::new();
        stream.write_render_frame(&view, &mut buf).unwrap();
        let snap = stream.snapshot();
        assert_eq!(snap.frames_written, 1);
        assert!(snap.bytes_written >= 8);
        assert!(!buf.is_empty());
    }

    #[test]
    fn frame_stream_encodes_into_buffer() {
        let mut slab = RenderSoaSlabCapsule::new(1, 1);
        let shard = [FormationCapsule::new(1, 0, 0, 0, 0, 0, 0, 0, 0, 0)];
        let shards = [&shard[..]];
        let view = collect_world_render_slab(&shards, &mut slab).unwrap();
        let stream = FrameStreamCapsule::new();
        let mut buf = [0u8; 32];
        let used = stream.write_render_frame_into(&view, &mut buf).unwrap();
        assert_eq!(used, 20);
        assert_eq!(u64::from_le_bytes(buf[..8].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(buf[8..12].try_into().unwrap()), 1);
    }
}
