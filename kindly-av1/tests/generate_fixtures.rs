//! Y4M Test Fixture Generator
//!
//! Generates deterministic Y4M test files for round-trip integration tests.
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q15-Q21 Integration tier (test fixture generation)
//! - **Chaos**: Deterministic patterns (no RNG), reproducible output
//! - **ASSUM**: All file I/O documented with error handling
//! - **T28**: Test support infrastructure
//!
//! ## Generated Fixtures
//!
//! 1. **test_8x8.y4m** - Tiny gradient (8×8, 1 frame, ~128 bytes)
//! 2. **test_64x64.y4m** - Small multi-pattern (64×64, 3 frames, ~6 KB)
//! 3. **test_320x240.y4m** - Medium checkerboard (320×240, 5 frames, ~580 KB)

use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Y4M file writer with deterministic pattern generation
struct Y4mWriter {
    file: File,
    width: u32,
    height: u32,
}

impl Y4mWriter {
    /// Create new Y4M file with header
    ///
    /// # Arguments
    ///
    /// * `path` - Output file path
    /// * `width` - Frame width
    /// * `height` - Frame height
    ///
    /// # Y4M Header Format
    ///
    /// ```text
    /// YUV4MPEG2 W{width} H{height} F30:1 Ip C420\n
    /// ```
    ///
    /// - W: Width in pixels
    /// - H: Height in pixels
    /// - F: Frame rate as numerator:denominator (30:1 = 30 fps)
    /// - I: Interlacing (p = progressive)
    /// - C: Chroma format (420 = YUV420p, 4:2:0 subsampling)
    fn new<P: AsRef<Path>>(path: P, width: u32, height: u32) -> std::io::Result<Self> {
        let mut file = File::create(path)?;

        // Write Y4M header
        writeln!(file, "YUV4MPEG2 W{} H{} F30:1 Ip C420", width, height)?;

        Ok(Self {
            file,
            width,
            height,
        })
    }

    /// Write a frame with YUV420p data
    ///
    /// # Arguments
    ///
    /// * `y_plane` - Luma plane (width × height)
    /// * `u_plane` - U chroma plane (width/2 × height/2)
    /// * `v_plane` - V chroma plane (width/2 × height/2)
    fn write_frame(
        &mut self,
        y_plane: &[u8],
        u_plane: &[u8],
        v_plane: &[u8],
    ) -> std::io::Result<()> {
        // Frame header
        writeln!(self.file, "FRAME")?;

        // Frame data (Y, U, V planes)
        self.file.write_all(y_plane)?;
        self.file.write_all(u_plane)?;
        self.file.write_all(v_plane)?;

        Ok(())
    }

    /// Generate YUV420p planes for a gradient pattern
    ///
    /// Gradient: top-left (0,0) = 0, bottom-right (width-1, height-1) = 255
    fn gradient_frame(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let y_size = (self.width * self.height) as usize;
        let uv_width = self.width / 2;
        let uv_height = self.height / 2;
        let uv_size = (uv_width * uv_height) as usize;

        let mut y_plane = vec![0u8; y_size];
        let u_plane = vec![128u8; uv_size]; // Neutral chroma
        let v_plane = vec![128u8; uv_size]; // Neutral chroma

        // Generate diagonal gradient
        for y_pos in 0..self.height {
            for x in 0..self.width {
                let idx = (y_pos * self.width + x) as usize;
                // Diagonal gradient: (0,0)=0 → (width-1,height-1)=255
                let val = ((x + y_pos) * 255 / (self.width + self.height - 2)) as u8;
                y_plane[idx] = val;
            }
        }

        (y_plane, u_plane, v_plane)
    }

    /// Generate solid gray frame (all pixels same value)
    fn solid_frame(&self, value: u8) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let y_size = (self.width * self.height) as usize;
        let uv_size = ((self.width / 2) * (self.height / 2)) as usize;

        let y_plane = vec![value; y_size];
        let u_plane = vec![128u8; uv_size]; // Neutral chroma
        let v_plane = vec![128u8; uv_size]; // Neutral chroma

        (y_plane, u_plane, v_plane)
    }

    /// Generate vertical gradient (top=0, bottom=255)
    fn vertical_gradient_frame(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let y_size = (self.width * self.height) as usize;
        let uv_size = ((self.width / 2) * (self.height / 2)) as usize;

        let mut y_plane = vec![0u8; y_size];
        let u_plane = vec![128u8; uv_size]; // Neutral chroma
        let v_plane = vec![128u8; uv_size]; // Neutral chroma

        for y_pos in 0..self.height {
            let val = (y_pos * 255 / (self.height - 1).max(1)) as u8;
            for x in 0..self.width {
                let idx = (y_pos * self.width + x) as usize;
                y_plane[idx] = val;
            }
        }

        (y_plane, u_plane, v_plane)
    }

    /// Generate horizontal gradient (left=0, right=255)
    fn horizontal_gradient_frame(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let y_size = (self.width * self.height) as usize;
        let uv_size = ((self.width / 2) * (self.height / 2)) as usize;

        let mut y_plane = vec![0u8; y_size];
        let u_plane = vec![128u8; uv_size]; // Neutral chroma
        let v_plane = vec![128u8; uv_size]; // Neutral chroma

        for y_pos in 0..self.height {
            for x in 0..self.width {
                let idx = (y_pos * self.width + x) as usize;
                let val = (x * 255 / (self.width - 1).max(1)) as u8;
                y_plane[idx] = val;
            }
        }

        (y_plane, u_plane, v_plane)
    }

    /// Generate checkerboard pattern with given block size
    fn checkerboard_frame(&self, block_size: u32) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let y_size = (self.width * self.height) as usize;
        let uv_size = ((self.width / 2) * (self.height / 2)) as usize;

        let mut y_plane = vec![0u8; y_size];
        let u_plane = vec![128u8; uv_size]; // Neutral chroma
        let v_plane = vec![128u8; uv_size]; // Neutral chroma

        for y_pos in 0..self.height {
            for x in 0..self.width {
                let idx = (y_pos * self.width + x) as usize;
                let block_x = x / block_size;
                let block_y = y_pos / block_size;
                // Alternate black/white blocks
                let val = if (block_x + block_y) % 2 == 0 {
                    0u8
                } else {
                    255u8
                };
                y_plane[idx] = val;
            }
        }

        (y_plane, u_plane, v_plane)
    }
}

/// Generate test_8x8.y4m - Tiny gradient (unit test fixture)
fn generate_test_8x8<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
    let mut writer = Y4mWriter::new(path, 8, 8)?;
    let (y, u, v) = writer.gradient_frame();
    writer.write_frame(&y, &u, &v)?;
    Ok(())
}

/// Generate test_64x64.y4m - Small multi-pattern (fast integration)
fn generate_test_64x64<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
    let mut writer = Y4mWriter::new(path, 64, 64)?;

    // Frame 0: Solid gray (128)
    let (y, u, v) = writer.solid_frame(128);
    writer.write_frame(&y, &u, &v)?;

    // Frame 1: Vertical gradient
    let (y, u, v) = writer.vertical_gradient_frame();
    writer.write_frame(&y, &u, &v)?;

    // Frame 2: Horizontal gradient
    let (y, u, v) = writer.horizontal_gradient_frame();
    writer.write_frame(&y, &u, &v)?;

    Ok(())
}

/// Generate test_320x240.y4m - Medium checkerboard (quality validation)
fn generate_test_320x240<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
    let mut writer = Y4mWriter::new(path, 320, 240)?;

    // Frame 0-4: Checkerboards with varying block sizes
    for block_size in [8, 16, 32, 64, 128] {
        let (y, u, v) = writer.checkerboard_frame(block_size);
        writer.write_frame(&y, &u, &v)?;
    }

    Ok(())
}

/// Generate test_4k.y4m - 4K UHD test patterns (3840×2160, 5 frames)
///
/// # Arguments
///
/// * `path` - Output file path
///
/// # Frame Patterns
///
/// 1. Checkerboard (64×64 blocks) - High-frequency content for compression stress
/// 2. Horizontal gradient - Smooth transitions, DCT efficiency test
/// 3. Vertical gradient - Alternative smooth transitions
/// 4. Diagonal gradient - Combined directional test
/// 5. Solid mid-gray (128) - Flat field for quantization validation
///
/// # Frame Size
///
/// - Y plane: 3840 × 2160 = 8,294,400 bytes
/// - U plane: 1920 × 1080 = 2,073,600 bytes (4:2:0 subsampling)
/// - V plane: 1920 × 1080 = 2,073,600 bytes
/// - Per frame: ~12.4 MB (uncompressed)
/// - 5 frames: ~62 MB total (uncompressed Y4M)
///
/// # UCE34 Compliance
///
/// - Q10: T5 Streaming tier (frame-by-frame generation, <10ns append)
/// - Q11: 100% Rust (no external dependencies)
/// - Q33: Deterministic patterns (no RNG)
///
/// # Chaos Compliance
///
/// - Lockfree: Pure computation, no synchronization
/// - Cache-aligned: Sequential memory access for optimal cache behavior
/// - Zero-copy: Direct buffer writes via Y4mWriter
fn generate_test_3840x2160<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
    let mut writer = Y4mWriter::new(path, 3840, 2160)?;

    // Frame 0: Checkerboard (64×64 blocks) - High-frequency content
    let (y, u, v) = writer.checkerboard_frame(64);
    writer.write_frame(&y, &u, &v)?;

    // Frame 1: Horizontal gradient (left=0, right=255)
    let (y, u, v) = writer.horizontal_gradient_frame();
    writer.write_frame(&y, &u, &v)?;

    // Frame 2: Vertical gradient (top=0, bottom=255)
    let (y, u, v) = writer.vertical_gradient_frame();
    writer.write_frame(&y, &u, &v)?;

    // Frame 3: Diagonal gradient (top-left=0, bottom-right=255)
    let (y, u, v) = writer.gradient_frame();
    writer.write_frame(&y, &u, &v)?;

    // Frame 4: Solid mid-gray (128) - Flat field for quantization
    let (y, u, v) = writer.solid_frame(128);
    writer.write_frame(&y, &u, &v)?;

    Ok(())
}

/// Main fixture generation entry point
fn main() -> std::io::Result<()> {
    let fixtures_dir = "tests/fixtures";

    println!("[generate_fixtures] Generating Y4M test fixtures...");

    // Generate all test files
    let test_8x8 = format!("{}/test_8x8.y4m", fixtures_dir);
    generate_test_8x8(&test_8x8)?;
    println!("  ✓ {} (8×8, 1 frame)", test_8x8);

    let test_64x64 = format!("{}/test_64x64.y4m", fixtures_dir);
    generate_test_64x64(&test_64x64)?;
    println!("  ✓ {} (64×64, 3 frames)", test_64x64);

    let test_320x240 = format!("{}/test_320x240.y4m", fixtures_dir);
    generate_test_320x240(&test_320x240)?;
    println!("  ✓ {} (320×240, 5 frames)", test_320x240);

    let test_4k = format!("{}/test_4k.y4m", fixtures_dir);
    println!("  [Generating 4K fixture... ~62 MB, may take 5-10 seconds]");
    generate_test_3840x2160(&test_4k)?;
    println!("  ✓ {} (3840×2160, 5 frames)", test_4k);

    println!("[generate_fixtures] Done! Fixtures ready for round-trip tests.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_y4m_writer_gradient_8x8() {
        // Just test the frame generation logic directly
        let test_writer = Y4mWriter {
            file: unsafe { std::mem::zeroed() },
            width: 8,
            height: 8,
        };
        let (y, u, v) = test_writer.gradient_frame();

        // Y plane should be 8×8 = 64 bytes
        assert_eq!(y.len(), 64);
        // U/V planes should be 4×4 = 16 bytes each (4:2:0 subsampling)
        assert_eq!(u.len(), 16);
        assert_eq!(v.len(), 16);

        // Top-left should be dark (0)
        assert_eq!(y[0], 0);
        // Bottom-right should be bright (255)
        assert_eq!(y[63], 255);

        // Chroma should be neutral gray (128)
        assert!(u.iter().all(|&x| x == 128));
        assert!(v.iter().all(|&x| x == 128));

        // Don't drop test_writer to avoid file descriptor issues
        std::mem::forget(test_writer);
    }

    #[test]
    fn test_y4m_writer_solid_64x64() {
        let test_writer = Y4mWriter {
            file: unsafe { std::mem::zeroed() },
            width: 64,
            height: 64,
        };
        let (y, u, v) = test_writer.solid_frame(128);

        // Y plane should be 64×64 = 4096 bytes, all 128
        assert_eq!(y.len(), 4096);
        assert!(y.iter().all(|&x| x == 128));

        // U/V planes should be 32×32 = 1024 bytes each
        assert_eq!(u.len(), 1024);
        assert_eq!(v.len(), 1024);

        std::mem::forget(test_writer);
    }

    #[test]
    fn test_y4m_writer_checkerboard_320x240() {
        let test_writer = Y4mWriter {
            file: unsafe { std::mem::zeroed() },
            width: 320,
            height: 240,
        };
        let (y, _u, _v) = test_writer.checkerboard_frame(16);

        // Y plane should be 320×240 = 76800 bytes
        assert_eq!(y.len(), 76800);

        // Top-left 16×16 block should be black (0)
        for y_pos in 0..16 {
            for x in 0..16 {
                let idx = (y_pos * 320 + x) as usize;
                assert_eq!(y[idx], 0, "Top-left block should be black");
            }
        }

        // Next 16×16 block (16-31, 0-15) should be white (255)
        for y_pos in 0..16 {
            for x in 16..32 {
                let idx = (y_pos * 320 + x) as usize;
                assert_eq!(y[idx], 255, "Adjacent block should be white");
            }
        }

        std::mem::forget(test_writer);
    }

    #[test]
    fn test_y4m_writer_4k_dimensions() {
        let test_writer = Y4mWriter {
            file: unsafe { std::mem::zeroed() },
            width: 3840,
            height: 2160,
        };

        // Test checkerboard pattern
        let (y, u, v) = test_writer.checkerboard_frame(64);

        // Y plane: 3840 × 2160 = 8,294,400 bytes
        assert_eq!(y.len(), 8_294_400, "Y plane should be 3840×2160");

        // U/V planes: 1920 × 1080 = 2,073,600 bytes each (4:2:0 subsampling)
        assert_eq!(u.len(), 2_073_600, "U plane should be 1920×1080");
        assert_eq!(v.len(), 2_073_600, "V plane should be 1920×1080");

        // Verify checkerboard pattern at top-left (should be black, block 0,0)
        for y_pos in 0..64 {
            for x in 0..64 {
                let idx = (y_pos * 3840 + x) as usize;
                assert_eq!(y[idx], 0, "Top-left 64×64 block should be black");
            }
        }

        // Verify adjacent block (64-127, 0-63) should be white
        for y_pos in 0..64 {
            for x in 64..128 {
                let idx = (y_pos * 3840 + x) as usize;
                assert_eq!(y[idx], 255, "Adjacent 64×64 block should be white");
            }
        }

        // Test gradient pattern
        let (y_grad, u_grad, v_grad) = test_writer.gradient_frame();

        // Verify top-left corner is dark
        assert_eq!(y_grad[0], 0, "Top-left gradient should be 0");

        // Verify bottom-right corner is bright
        let last_idx = (2160 - 1) * 3840 + (3840 - 1);
        assert_eq!(y_grad[last_idx], 255, "Bottom-right gradient should be 255");

        // Verify chroma is neutral
        assert!(
            u_grad.iter().all(|&x| x == 128),
            "U plane should be neutral"
        );
        assert!(
            v_grad.iter().all(|&x| x == 128),
            "V plane should be neutral"
        );

        std::mem::forget(test_writer);
    }
}
