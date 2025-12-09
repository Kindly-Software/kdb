//! User choice to technical parameter mapping
//!
//! Maps friendly wizard choices to technical encoding options.

use crate::cli::legacy::EncodingPreset;

/// Quality goal selection (user-friendly)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum QualityGoal {
    /// Smallest file size (~65-75% reduction, CRF 40-45)
    Smallest,
    /// Balanced quality/size (default, ~45-55% reduction, CRF 32-35)
    #[default]
    Balanced,
    /// Best quality (~25-35% reduction, CRF 24-28)
    Best,
}

impl QualityGoal {
    /// Convert to CRF value
    #[inline]
    pub const fn to_crf(&self) -> u8 {
        match self {
            Self::Smallest => 42,  // Mid-range of 40-45
            Self::Balanced => 33,  // Mid-range of 32-35
            Self::Best => 26,      // Mid-range of 24-28
        }
    }

    /// Get reduction percentage range (min, max)
    #[inline]
    pub const fn reduction_percent(&self) -> (u8, u8) {
        match self {
            Self::Smallest => (65, 75),
            Self::Balanced => (45, 55),
            Self::Best => (25, 35),
        }
    }

    /// Get user-facing label
    #[inline]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Smallest => "Smallest size",
            Self::Balanced => "Balanced",
            Self::Best => "Best quality",
        }
    }

    /// Get description
    #[inline]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Smallest => "Maximum compression, smaller file sizes (~65-75% reduction)",
            Self::Balanced => "Good quality/size balance (~45-55% reduction)",
            Self::Best => "Highest quality, larger files (~25-35% reduction)",
        }
    }
}

/// Speed choice selection (user-friendly)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SpeedChoice {
    /// Quick encoding (speed 8, 2x1 tiles, 1x time)
    Quick,
    /// Normal encoding (default, speed 5, 1x1 tiles, 2.5x time)
    #[default]
    Normal,
    /// Thorough encoding (speed 2, 0x0 tiles, 5-6x time)
    Thorough,
}

impl SpeedChoice {
    /// Convert to speed value
    #[inline]
    pub const fn to_speed(&self) -> u8 {
        match self {
            Self::Quick => 8,
            Self::Normal => 5,
            Self::Thorough => 2,
        }
    }

    /// Get tile configuration (columns, rows)
    #[inline]
    pub const fn to_tiles(&self) -> (u8, u8) {
        match self {
            Self::Quick => (2, 1),
            Self::Normal => (1, 1),
            Self::Thorough => (0, 0),
        }
    }

    /// Get time multiplier relative to Quick
    #[inline]
    pub const fn time_multiplier(&self) -> f32 {
        match self {
            Self::Quick => 1.0,
            Self::Normal => 2.5,
            Self::Thorough => 5.5,
        }
    }

    /// Get user-facing label
    #[inline]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Quick => "Quick",
            Self::Normal => "Normal",
            Self::Thorough => "Thorough",
        }
    }

    /// Get description
    #[inline]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Quick => "Fast encoding, good for previews",
            Self::Normal => "Balanced speed/quality (recommended)",
            Self::Thorough => "Slower encoding, better compression",
        }
    }
}

/// Complete encoding options (technical parameters)
#[derive(Debug, Clone, Copy)]
pub struct EncodingOptions {
    /// Constant Rate Factor (0-63)
    pub crf: u8,
    /// Encoding preset
    pub preset: EncodingPreset,
    /// Speed value (0-10)
    pub speed: u8,
    /// Tile columns
    pub tile_columns: u8,
    /// Tile rows
    pub tile_rows: u8,
}

/// Map user choices to technical encoding options
#[inline]
pub const fn map_to_encoding_options(quality: QualityGoal, speed: SpeedChoice) -> EncodingOptions {
    let (tile_columns, tile_rows) = speed.to_tiles();

    EncodingOptions {
        crf: quality.to_crf(),
        preset: EncodingPreset::Medium,  // Always Medium for consistency
        speed: speed.to_speed(),
        tile_columns,
        tile_rows,
    }
}

/// Estimate output size and savings
///
/// Returns (estimated_size_bytes, savings_bytes)
#[inline]
pub fn estimate_output_size(input_size_bytes: u64, quality: QualityGoal) -> (u64, u64) {
    let (min_reduction, max_reduction) = quality.reduction_percent();
    let avg_reduction = ((min_reduction + max_reduction) as f64) / 200.0;  // Convert to 0-1 range

    let estimated = (input_size_bytes as f64 * (1.0 - avg_reduction)) as u64;
    let savings = input_size_bytes.saturating_sub(estimated);

    (estimated, savings)
}

/// Estimate encoding time in seconds
///
/// Simple heuristic: base_time = file_mb * 3.0, adjusted by speed multiplier
#[inline]
pub fn estimate_time(input_size_bytes: u64, speed: SpeedChoice) -> u64 {
    const BASE_SECONDS_PER_MB: f32 = 3.0;

    let size_mb = (input_size_bytes as f32) / (1024.0 * 1024.0);
    let base_time = size_mb * BASE_SECONDS_PER_MB;
    let adjusted_time = base_time * speed.time_multiplier();

    adjusted_time as u64
}

/// Format time duration as human-readable string
pub fn format_time(seconds: u64) -> String {
    if seconds < 60 {
        format!("~{} seconds", seconds)
    } else if seconds < 3600 {
        let minutes = seconds / 60;
        format!("~{} minute{}", minutes, if minutes == 1 { "" } else { "s" })
    } else {
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        if minutes == 0 {
            format!("~{} hour{}", hours, if hours == 1 { "" } else { "s" })
        } else {
            format!("~{} hour{} {} min", hours, if hours == 1 { "" } else { "s" }, minutes)
        }
    }
}

/// Format file size as human-readable string
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", (bytes as f64) / (GB as f64))
    } else if bytes >= MB {
        format!("{:.0} MB", (bytes as f64) / (MB as f64))
    } else if bytes >= KB {
        format!("{:.0} KB", (bytes as f64) / (KB as f64))
    } else {
        format!("{} bytes", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_goal_crf() {
        assert_eq!(QualityGoal::Smallest.to_crf(), 42);
        assert_eq!(QualityGoal::Balanced.to_crf(), 33);
        assert_eq!(QualityGoal::Best.to_crf(), 26);
    }

    #[test]
    fn test_quality_goal_reduction() {
        assert_eq!(QualityGoal::Smallest.reduction_percent(), (65, 75));
        assert_eq!(QualityGoal::Balanced.reduction_percent(), (45, 55));
        assert_eq!(QualityGoal::Best.reduction_percent(), (25, 35));
    }

    #[test]
    fn test_speed_choice_values() {
        assert_eq!(SpeedChoice::Quick.to_speed(), 8);
        assert_eq!(SpeedChoice::Normal.to_speed(), 5);
        assert_eq!(SpeedChoice::Thorough.to_speed(), 2);
    }

    #[test]
    fn test_speed_choice_tiles() {
        assert_eq!(SpeedChoice::Quick.to_tiles(), (2, 1));
        assert_eq!(SpeedChoice::Normal.to_tiles(), (1, 1));
        assert_eq!(SpeedChoice::Thorough.to_tiles(), (0, 0));
    }

    #[test]
    fn test_mapping() {
        let opts = map_to_encoding_options(QualityGoal::Balanced, SpeedChoice::Normal);
        assert_eq!(opts.crf, 33);
        assert_eq!(opts.speed, 5);
        assert_eq!(opts.tile_columns, 1);
        assert_eq!(opts.tile_rows, 1);
    }

    #[test]
    fn test_estimate_output_size() {
        let input_size = 1_000_000_000;  // 1 GB

        let (smallest_est, smallest_save) = estimate_output_size(input_size, QualityGoal::Smallest);
        assert!(smallest_est < input_size * 35 / 100);  // Should be ~30% of original
        assert!(smallest_save > input_size * 65 / 100);  // Should save ~70%

        let (balanced_est, balanced_save) = estimate_output_size(input_size, QualityGoal::Balanced);
        assert!(balanced_est < input_size * 55 / 100);  // Should be ~50% of original
        assert!(balanced_save > input_size * 45 / 100);  // Should save ~50%
    }

    #[test]
    fn test_estimate_time() {
        let input_100mb = 100 * 1024 * 1024;

        let quick_time = estimate_time(input_100mb, SpeedChoice::Quick);
        let normal_time = estimate_time(input_100mb, SpeedChoice::Normal);
        let thorough_time = estimate_time(input_100mb, SpeedChoice::Thorough);

        // Normal should be ~2.5x Quick
        assert!(normal_time > quick_time);
        assert!(normal_time < quick_time * 3);

        // Thorough should be ~5.5x Quick
        assert!(thorough_time > quick_time * 4);
        assert!(thorough_time < quick_time * 7);
    }

    #[test]
    fn test_format_time() {
        assert_eq!(format_time(30), "~30 seconds");
        assert_eq!(format_time(90), "~1 minute");
        assert_eq!(format_time(300), "~5 minutes");
        assert_eq!(format_time(3600), "~1 hour");
        assert_eq!(format_time(3660), "~1 hour 1 min");
        assert_eq!(format_time(7200), "~2 hours");
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 bytes");
        assert_eq!(format_size(5 * 1024), "5 KB");
        assert_eq!(format_size(150 * 1024 * 1024), "150 MB");
        assert_eq!(format_size(2 * 1024 * 1024 * 1024), "2.0 GB");
    }

    #[test]
    fn test_defaults() {
        let quality = QualityGoal::default();
        assert_eq!(quality, QualityGoal::Balanced);

        let speed = SpeedChoice::default();
        assert_eq!(speed, SpeedChoice::Normal);
    }
}
