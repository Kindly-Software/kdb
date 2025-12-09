//! Legacy CLI argument parsing for kindly-av1
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! This module contains the original argument parsing code for backwards compatibility.
//! New code should prefer the `args` module with full branding support.

use std::path::PathBuf;

/// Result type for legacy CLI parsing
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Encoding preset (speed vs quality tradeoff)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EncodingPreset {
    /// Fastest encoding, lowest quality
    Ultrafast,
    /// Very fast encoding
    Superfast,
    /// Fast encoding
    Veryfast,
    /// Faster encoding
    Faster,
    /// Fast encoding
    Fast,
    /// Balanced (default)
    #[default]
    Medium,
    /// Slower encoding, better quality
    Slow,
    /// Even slower encoding
    Slower,
    /// Slowest encoding, best quality
    Veryslow,
}

impl EncodingPreset {
    /// Parse preset from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "ultrafast" => Some(Self::Ultrafast),
            "superfast" => Some(Self::Superfast),
            "veryfast" => Some(Self::Veryfast),
            "faster" => Some(Self::Faster),
            "fast" => Some(Self::Fast),
            "medium" => Some(Self::Medium),
            "slow" => Some(Self::Slow),
            "slower" => Some(Self::Slower),
            "veryslow" => Some(Self::Veryslow),
            _ => None,
        }
    }

    /// Get the rav1e speed setting for this preset
    #[inline]
    pub const fn to_speed(&self) -> u8 {
        match self {
            Self::Ultrafast => 10,
            Self::Superfast => 9,
            Self::Veryfast => 8,
            Self::Faster => 7,
            Self::Fast => 6,
            Self::Medium => 5,
            Self::Slow => 4,
            Self::Slower => 3,
            Self::Veryslow => 2,
        }
    }
}

/// GPU backend selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GpuBackend {
    /// Automatic GPU detection (prefer ROCm, fallback to Vulkan, then CPU)
    #[default]
    Auto,
    /// AMD ROCm (HIP)
    Rocm,
    /// Vulkan compute
    Vulkan,
    /// CPU only (no GPU acceleration)
    Cpu,
}

impl GpuBackend {
    /// Parse GPU backend from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "rocm" | "hip" | "amd" => Some(Self::Rocm),
            "vulkan" | "vk" => Some(Self::Vulkan),
            "cpu" | "none" => Some(Self::Cpu),
            _ => None,
        }
    }
}

/// Encode command arguments (legacy structure)
#[derive(Debug, Clone)]
pub struct EncodeArgs {
    /// Input file path
    pub input: PathBuf,
    /// Output file path
    pub output: PathBuf,
    /// Encoding preset
    pub preset: EncodingPreset,
    /// Constant Rate Factor (0-63, lower = better quality)
    pub crf: u8,
    /// Target bitrate in kbps (alternative to CRF)
    pub bitrate: Option<u32>,
    /// GPU backend
    pub gpu_backend: GpuBackend,
    /// Thread count (0 = auto)
    pub threads: u32,
    /// Checkpoint file for resume capability
    pub checkpoint: Option<PathBuf>,
    /// Resume from checkpoint
    pub resume: bool,
    /// Keyframe interval
    pub keyint: u32,
    /// Tile columns for parallelism
    pub tile_columns: u32,
    /// Tile rows for parallelism
    pub tile_rows: u32,
}

impl Default for EncodeArgs {
    fn default() -> Self {
        Self {
            input: PathBuf::new(),
            output: PathBuf::new(),
            preset: EncodingPreset::Medium,
            crf: 28,
            bitrate: None,
            gpu_backend: GpuBackend::Auto,
            threads: 0,
            checkpoint: None,
            resume: false,
            keyint: 250,
            tile_columns: 0,
            tile_rows: 0,
        }
    }
}

/// Parse encode command arguments (legacy parser)
pub fn parse_encode_args(args: &[String]) -> Result<EncodeArgs> {
    if args.is_empty() {
        return Err("No input file specified. Usage: kindly-av1 encode <INPUT> [OPTIONS]".into());
    }

    let mut encode_args = EncodeArgs::default();
    encode_args.input = PathBuf::from(&args[0]);

    // Default output: input stem + .av1
    encode_args.output = encode_args
        .input
        .with_extension("av1");

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                if i >= args.len() {
                    return Err("Missing value for --output".into());
                }
                encode_args.output = PathBuf::from(&args[i]);
            }
            "--preset" => {
                i += 1;
                if i >= args.len() {
                    return Err("Missing value for --preset".into());
                }
                encode_args.preset = EncodingPreset::from_str(&args[i])
                    .ok_or_else(|| format!("Invalid preset: {}", args[i]))?;
            }
            "--crf" => {
                i += 1;
                if i >= args.len() {
                    return Err("Missing value for --crf".into());
                }
                let crf: u8 = args[i].parse()
                    .map_err(|_| format!("Invalid CRF value: {}", args[i]))?;
                if crf > 63 {
                    return Err(format!("CRF must be 0-63, got {}", crf).into());
                }
                encode_args.crf = crf;
            }
            "--bitrate" => {
                i += 1;
                if i >= args.len() {
                    return Err("Missing value for --bitrate".into());
                }
                encode_args.bitrate = Some(args[i].parse()
                    .map_err(|_| format!("Invalid bitrate: {}", args[i]))?);
            }
            "--gpu" => {
                i += 1;
                if i >= args.len() {
                    return Err("Missing value for --gpu".into());
                }
                encode_args.gpu_backend = GpuBackend::from_str(&args[i])
                    .ok_or_else(|| format!("Invalid GPU backend: {}", args[i]))?;
            }
            "--threads" => {
                i += 1;
                if i >= args.len() {
                    return Err("Missing value for --threads".into());
                }
                if args[i] == "auto" {
                    encode_args.threads = 0;
                } else {
                    encode_args.threads = args[i].parse()
                        .map_err(|_| format!("Invalid thread count: {}", args[i]))?;
                }
            }
            "--checkpoint" => {
                i += 1;
                if i >= args.len() {
                    return Err("Missing value for --checkpoint".into());
                }
                encode_args.checkpoint = Some(PathBuf::from(&args[i]));
            }
            "--resume" => {
                encode_args.resume = true;
            }
            "--keyint" => {
                i += 1;
                if i >= args.len() {
                    return Err("Missing value for --keyint".into());
                }
                encode_args.keyint = args[i].parse()
                    .map_err(|_| format!("Invalid keyint: {}", args[i]))?;
            }
            "--tile-columns" => {
                i += 1;
                if i >= args.len() {
                    return Err("Missing value for --tile-columns".into());
                }
                encode_args.tile_columns = args[i].parse()
                    .map_err(|_| format!("Invalid tile-columns: {}", args[i]))?;
            }
            "--tile-rows" => {
                i += 1;
                if i >= args.len() {
                    return Err("Missing value for --tile-rows".into());
                }
                encode_args.tile_rows = args[i].parse()
                    .map_err(|_| format!("Invalid tile-rows: {}", args[i]))?;
            }
            "-h" | "--help" => {
                print_encode_help();
                std::process::exit(0);
            }
            arg => {
                return Err(format!("Unknown option: {}", arg).into());
            }
        }
        i += 1;
    }

    // Validate input exists
    if !encode_args.input.exists() {
        return Err(format!("Input file not found: {}", encode_args.input.display()).into());
    }

    Ok(encode_args)
}

/// Print encode command help (legacy, no branding)
fn print_encode_help() {
    println!("kindly-av1 encode - Encode video file to AV1 format");
    println!();
    println!("USAGE:");
    println!("    kindly-av1 encode <INPUT> [OPTIONS]");
    println!();
    println!("ARGS:");
    println!("    <INPUT>    Input video file");
    println!();
    println!("OPTIONS:");
    println!("    -o, --output <FILE>       Output file path [default: <input>.av1]");
    println!("    --preset <PRESET>         Encoding preset [default: medium]");
    println!("                              (ultrafast/superfast/veryfast/faster/fast/");
    println!("                               medium/slow/slower/veryslow)");
    println!("    --crf <0-63>              Constant Rate Factor [default: 28]");
    println!("    --bitrate <KBPS>          Target bitrate in kbps (alternative to CRF)");
    println!("    --gpu <BACKEND>           GPU backend [default: auto]");
    println!("                              (auto/rocm/vulkan/cpu)");
    println!("    --threads <N|auto>        Thread count [default: auto]");
    println!("    --checkpoint <FILE>       Checkpoint file for resume capability");
    println!("    --resume                  Resume from checkpoint");
    println!("    --keyint <N>              Keyframe interval [default: 250]");
    println!("    --tile-columns <N>        Tile columns for parallelism");
    println!("    --tile-rows <N>           Tile rows for parallelism");
    println!("    -h, --help                Print help information");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_from_str() {
        assert_eq!(EncodingPreset::from_str("medium"), Some(EncodingPreset::Medium));
        assert_eq!(EncodingPreset::from_str("FAST"), Some(EncodingPreset::Fast));
        assert_eq!(EncodingPreset::from_str("invalid"), None);
    }

    #[test]
    fn test_gpu_backend_from_str() {
        assert_eq!(GpuBackend::from_str("auto"), Some(GpuBackend::Auto));
        assert_eq!(GpuBackend::from_str("rocm"), Some(GpuBackend::Rocm));
        assert_eq!(GpuBackend::from_str("vulkan"), Some(GpuBackend::Vulkan));
        assert_eq!(GpuBackend::from_str("cpu"), Some(GpuBackend::Cpu));
    }

    #[test]
    fn test_default_encode_args() {
        let args = EncodeArgs::default();
        assert_eq!(args.preset, EncodingPreset::Medium);
        assert_eq!(args.crf, 28);
        assert_eq!(args.gpu_backend, GpuBackend::Auto);
        assert_eq!(args.keyint, 250);
    }

    #[test]
    fn test_preset_to_speed() {
        assert_eq!(EncodingPreset::Ultrafast.to_speed(), 10);
        assert_eq!(EncodingPreset::Medium.to_speed(), 5);
        assert_eq!(EncodingPreset::Veryslow.to_speed(), 2);
    }
}
