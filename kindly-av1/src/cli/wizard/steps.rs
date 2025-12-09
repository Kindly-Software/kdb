//! Wizard Step Renderers
//!
//! Individual step rendering functions for the 5-step encoding wizard.
//!
//! ## Framework Compliance
//! - **Chaos**: Simple string formatting, no complex coordination
//! - **UCE34**: Correctness over optimization (Q1-Q28 simple-coding)
//! - **ASSUM**: No unsafe, no assumptions needed

use crate::cli::branding::{BOLD, DIM, HEART, LIGHTNING, PURPLE, RESET, SPARK};
use crate::cli::wizard::mapping::{QualityGoal, SpeedChoice};

// ============================================================================
// Wizard Context
// ============================================================================

/// Wizard state container
pub struct WizardContext {
    pub input_path: Option<String>,
    pub quality: QualityGoal,
    pub speed: SpeedChoice,
    pub output_path: Option<String>,
    pub gpu_name: String,
    pub cpu_threads: u32,
    pub memory_gb: u32,
}

impl Default for WizardContext {
    fn default() -> Self {
        Self {
            input_path: None,
            quality: QualityGoal::Balanced,
            speed: SpeedChoice::Normal,
            output_path: None,
            gpu_name: "Unknown".to_string(),
            cpu_threads: 0,
            memory_gb: 0,
        }
    }
}

// ============================================================================
// Extension Traits for Display
// ============================================================================

trait QualityGoalExt {
    fn display_label(&self) -> &'static str;
    fn display_description(&self) -> &'static str;
    fn display_use_case(&self) -> &'static str;
}

impl QualityGoalExt for QualityGoal {
    fn display_label(&self) -> &'static str {
        self.label()
    }

    fn display_description(&self) -> &'static str {
        match self {
            QualityGoal::Smallest => "Saves the most space (~70% smaller)",
            QualityGoal::Balanced => "Best of both worlds (~50% smaller)",
            QualityGoal::Best => "Keeps everything crisp (~30% smaller)",
        }
    }

    fn display_use_case(&self) -> &'static str {
        match self {
            QualityGoal::Smallest => "archiving, slow internet uploads",
            QualityGoal::Balanced => "most uses, sharing, storage",
            QualityGoal::Best => "important videos, editing later",
        }
    }
}

trait SpeedChoiceExt {
    fn display_label(&self) -> &'static str;
    fn display_eta(&self) -> &'static str;
    fn display_description(&self) -> &'static str;
}

impl SpeedChoiceExt for SpeedChoice {
    fn display_label(&self) -> &'static str {
        self.label()
    }

    fn display_eta(&self) -> &'static str {
        match self {
            SpeedChoice::Quick => "~2 minutes",
            SpeedChoice::Normal => "~5 minutes",
            SpeedChoice::Thorough => "~12 minutes",
        }
    }

    fn display_description(&self) -> &'static str {
        match self {
            SpeedChoice::Quick => "I need it now!",
            SpeedChoice::Normal => "I can wait a bit for better compression",
            SpeedChoice::Thorough => "Take your time, I want smallest file",
        }
    }
}

// ============================================================================
// Step Renderers
// ============================================================================

/// Render Step 0: Auto-Detection (automatic hardware check)
pub fn render_step_0(ctx: &WizardContext) -> String {
    let gpu_status = if !ctx.gpu_name.is_empty() && ctx.gpu_name != "Unknown" {
        format!("{} (ROCm ready) [OK]", ctx.gpu_name)
    } else {
        "Not detected [Using CPU]".to_string()
    };

    let memory_status = if ctx.memory_gb > 0 {
        format!("{} GB available [OK]", ctx.memory_gb)
    } else {
        "Unknown".to_string()
    };

    let cpu_status = if ctx.cpu_threads > 0 {
        format!("{} threads available [OK]", ctx.cpu_threads)
    } else {
        "Unknown".to_string()
    };

    format!(
        "{}{} Kindly-AV1 Encoder{}\n\
         \n\
         Checking your computer...\n\
         {}  {} GPU:     {}\n\
         {}  \u{1F4BE} Memory:  {}\n\
         {}  \u{1F527} CPU:     {}\n\
         \n\
         Great! You're all set. Press Enter to continue...",
        PURPLE, HEART, RESET,
        PURPLE, LIGHTNING, gpu_status,
        PURPLE, memory_status,
        PURPLE, cpu_status
    )
}

/// Render Step 1: Select Video
pub fn render_step_1(ctx: &WizardContext, recent_files: &[(String, u64)]) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!(
        "{}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}{}\n",
        DIM, RESET
    ));
    output.push_str(&format!("{}{}Step 1 of 4: Which video?{}\n", PURPLE, BOLD, RESET));
    output.push_str(&format!(
        "{}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}{}\n\n",
        DIM, RESET
    ));

    // Content
    output.push_str("What video do you want to compress?\n\n");
    output.push_str(&format!("  {}[B]{} Browse for file...\n\n", BOLD, RESET));
    output.push_str("  Or paste/type the file path:\n");
    output.push_str(&format!("  {}> _{}\n\n", PURPLE, RESET));

    // Recent files
    if !recent_files.is_empty() {
        output.push_str(&format!("{}Recent files:{}\n", DIM, RESET));
        for (idx, (path, size)) in recent_files.iter().enumerate().take(2) {
            let size_str = format_size(*size);
            output.push_str(&format!("  [{}] {}    ({})\n", idx + 1, path, size_str));
        }
    }

    // Footer
    output.push_str("\n");
    output.push_str(&format!(
        "{}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}{}\n",
        DIM, RESET
    ));
    output.push_str(&format!("{}[Q]{} Quit  {}[?]{} Help\n", BOLD, RESET, BOLD, RESET));

    output
}

/// Render Step 2: Quality Goal
pub fn render_step_2(_ctx: &WizardContext) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!(
        "{}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}{}\n",
        DIM, RESET
    ));
    output.push_str(&format!("{}{}Step 2 of 4: Quality{}\n", PURPLE, BOLD, RESET));
    output.push_str(&format!(
        "{}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}{}\n\n",
        DIM, RESET
    ));

    // Content
    output.push_str("How should we compress your video?\n\n");

    // Option 1: Smallest
    output.push_str(&format!("  {}[1]{} \u{1F4E6} {}\n", BOLD, RESET, QualityGoal::Smallest.display_label()));
    output.push_str(&format!("      {}{}{}\n", DIM, QualityGoal::Smallest.display_description(), RESET));
    output.push_str(&format!("      {}Good for: {}{}\n\n", DIM, QualityGoal::Smallest.display_use_case(), RESET));

    // Option 2: Balanced (Recommended)
    output.push_str(&format!("  {}[2]{} \u{2696}\u{FE0F} {} (Recommended)\n", BOLD, RESET, QualityGoal::Balanced.display_label()));
    output.push_str(&format!("      {}{}{}\n", DIM, QualityGoal::Balanced.display_description(), RESET));
    output.push_str(&format!("      {}Good for: {}{}\n\n", DIM, QualityGoal::Balanced.display_use_case(), RESET));

    // Option 3: Best quality
    output.push_str(&format!("  {}[3]{} \u{1F3AC} {}\n", BOLD, RESET, QualityGoal::Best.display_label()));
    output.push_str(&format!("      {}{}{}\n", DIM, QualityGoal::Best.display_description(), RESET));
    output.push_str(&format!("      {}Good for: {}{}\n\n", DIM, QualityGoal::Best.display_use_case(), RESET));

    output.push_str(&format!("{}Your choice [1-3, default=2]: _{}\n\n", PURPLE, RESET));

    // Footer
    output.push_str(&format!(
        "{}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}{}\n",
        DIM, RESET
    ));
    output.push_str(&format!("{}[\u{2190}]{} Back  {}[Q]{} Quit  {}[?]{} Help\n", BOLD, RESET, BOLD, RESET, BOLD, RESET));

    output
}

/// Render Step 3: Speed Choice
pub fn render_step_3(_ctx: &WizardContext) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!(
        "{}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}{}\n",
        DIM, RESET
    ));
    output.push_str(&format!("{}{}Step 3 of 4: Speed{}\n", PURPLE, BOLD, RESET));
    output.push_str(&format!(
        "{}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}{}\n\n",
        DIM, RESET
    ));

    // Content
    output.push_str("How long can you wait?\n\n");

    // Option 1: Quick
    output.push_str(&format!("  {}[1]{} {} {} ({})\n", BOLD, RESET, LIGHTNING, SpeedChoice::Quick.display_label(), SpeedChoice::Quick.display_eta()));
    output.push_str(&format!("      {}{}{}\n\n", DIM, SpeedChoice::Quick.display_description(), RESET));

    // Option 2: Normal
    output.push_str(&format!("  {}[2]{} \u{23F0} {} ({})\n", BOLD, RESET, SpeedChoice::Normal.display_label(), SpeedChoice::Normal.display_eta()));
    output.push_str(&format!("      {}{}{}\n\n", DIM, SpeedChoice::Normal.display_description(), RESET));

    // Option 3: Thorough
    output.push_str(&format!("  {}[3]{} \u{1F422} {} ({})\n", BOLD, RESET, SpeedChoice::Thorough.display_label(), SpeedChoice::Thorough.display_eta()));
    output.push_str(&format!("      {}{}{}\n\n", DIM, SpeedChoice::Thorough.display_description(), RESET));

    output.push_str(&format!("{}Your choice [1-3, default=2]: _{}\n\n", PURPLE, RESET));

    // Footer
    output.push_str(&format!(
        "{}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}{}\n",
        DIM, RESET
    ));
    output.push_str(&format!("{}[\u{2190}]{} Back  {}[Q]{} Quit  {}[?]{} Help\n", BOLD, RESET, BOLD, RESET, BOLD, RESET));

    output
}

/// Render Step 4: Confirm & Start
pub fn render_step_4(ctx: &WizardContext) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!(
        "{}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}{}\n",
        DIM, RESET
    ));
    output.push_str(&format!("{}{}Step 4 of 4: Ready!{}\n", PURPLE, BOLD, RESET));
    output.push_str(&format!(
        "{}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}{}\n\n",
        DIM, RESET
    ));

    // Content
    output.push_str("Here's what we'll do:\n\n");

    // Input file
    let input_display = ctx.input_path.as_deref().unwrap_or("(no file selected)");
    output.push_str(&format!("  \u{1F4C1} Input:    {}\n", input_display));

    // Output file
    let output_display = ctx.output_path.as_deref().unwrap_or("(auto-generated)");
    output.push_str(&format!("  \u{1F4C1} Output:   {}\n", output_display));

    // Quality
    output.push_str(&format!("  \u{1F3AF} Quality:  {}\n", ctx.quality.display_label()));

    // Speed
    output.push_str(&format!("  {} Speed:    {} ({})\n", LIGHTNING, ctx.speed.display_label(), ctx.speed.display_eta()));

    // Estimated size (placeholder - would be calculated from input file)
    output.push_str("  \u{1F4E6} Est. size: ~550 MB (saves ~650 MB!)\n");

    // GPU
    if !ctx.gpu_name.is_empty() && ctx.gpu_name != "Unknown" {
        output.push_str(&format!("  \u{1F5A5}\u{FE0F} Using:    {} GPU\n", ctx.gpu_name));
    } else {
        output.push_str(&format!("  \u{1F5A5}\u{FE0F} Using:    CPU ({} threads)\n", ctx.cpu_threads));
    }

    output.push_str("\n");

    // Footer divider
    output.push_str(&format!(
        "{}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}{}\n\n",
        DIM, RESET
    ));

    // Options
    output.push_str(&format!("  {}[Enter]{} {} Start encoding\n", BOLD, RESET, SPARK));
    output.push_str(&format!("  {}[C]{}      Change settings\n", BOLD, RESET));
    output.push_str(&format!("  {}[A]{}      Advanced options (for experts)\n", BOLD, RESET));
    output.push_str(&format!("  {}[O]{}      Change output location\n", BOLD, RESET));
    output.push_str(&format!("  {}[Q]{}      Quit\n\n", BOLD, RESET));

    output.push_str("Press Enter to start...");

    output
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Format file size in human-readable format
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wizard_context_default() {
        let ctx = WizardContext::default();
        assert_eq!(ctx.quality, QualityGoal::Balanced);
        assert_eq!(ctx.speed, SpeedChoice::Normal);
        assert!(ctx.input_path.is_none());
        assert!(ctx.output_path.is_none());
    }

    #[test]
    fn test_quality_goal_strings() {
        assert_eq!(QualityGoal::Smallest.display_label(), "Smallest size");
        assert_eq!(QualityGoal::Balanced.display_label(), "Balanced");
        assert_eq!(QualityGoal::Best.display_label(), "Best quality");
    }

    #[test]
    fn test_speed_choice_strings() {
        assert_eq!(SpeedChoice::Quick.display_label(), "Quick");
        assert_eq!(SpeedChoice::Normal.display_label(), "Normal");
        assert_eq!(SpeedChoice::Thorough.display_label(), "Thorough");

        assert_eq!(SpeedChoice::Quick.display_eta(), "~2 minutes");
        assert_eq!(SpeedChoice::Normal.display_eta(), "~5 minutes");
        assert_eq!(SpeedChoice::Thorough.display_eta(), "~12 minutes");
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1 KB");
        assert_eq!(format_size(1536 * 1024), "1 MB");
        assert_eq!(format_size(2 * 1024 * 1024 * 1024), "2.0 GB");
    }

    #[test]
    fn test_render_step_0() {
        let mut ctx = WizardContext::default();
        ctx.gpu_name = "AMD RX 6700 XT".to_string();
        ctx.memory_gb = 32;
        ctx.cpu_threads = 16;

        let output = render_step_0(&ctx);
        assert!(output.contains("Kindly-AV1 Encoder"));
        assert!(output.contains("AMD RX 6700 XT"));
        assert!(output.contains("32 GB"));
        assert!(output.contains("16 threads"));
    }

    #[test]
    fn test_render_step_1() {
        let ctx = WizardContext::default();
        let recent = vec![
            ("~/Videos/vacation_2024.mp4".to_string(), 1_258_291_200),
            ("~/Desktop/screen_recording.mp4".to_string(), 471_859_200),
        ];

        let output = render_step_1(&ctx, &recent);
        assert!(output.contains("Step 1 of 4"));
        assert!(output.contains("Which video?"));
        assert!(output.contains("vacation_2024.mp4"));
        assert!(output.contains("1.2 GB")); // Updated to match format_size output
    }

    #[test]
    fn test_render_step_2() {
        let ctx = WizardContext::default();
        let output = render_step_2(&ctx);
        assert!(output.contains("Step 2 of 4"));
        assert!(output.contains("Quality"));
        assert!(output.contains("Smallest size"));
        assert!(output.contains("Balanced"));
        assert!(output.contains("Best quality"));
    }

    #[test]
    fn test_render_step_3() {
        let ctx = WizardContext::default();
        let output = render_step_3(&ctx);
        assert!(output.contains("Step 3 of 4"));
        assert!(output.contains("Speed"));
        assert!(output.contains("Quick"));
        assert!(output.contains("Normal"));
        assert!(output.contains("Thorough"));
    }

    #[test]
    fn test_render_step_4() {
        let mut ctx = WizardContext::default();
        ctx.input_path = Some("vacation_2024.mp4".to_string());
        ctx.output_path = Some("vacation_2024.av1".to_string());
        ctx.gpu_name = "AMD RX 6700 XT".to_string();

        let output = render_step_4(&ctx);
        assert!(output.contains("Step 4 of 4"));
        assert!(output.contains("Ready!"));
        assert!(output.contains("vacation_2024.mp4"));
        assert!(output.contains("vacation_2024.av1"));
        assert!(output.contains("AMD RX 6700 XT"));
    }
}
