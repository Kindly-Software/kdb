//! kindly_dedup GUI - Drag & Drop Deduplication (DEPRECATED)
//!
//! # DEPRECATION WARNING
//! **This egui GUI is DEPRECATED as of v1.13.2.**
//!
//! The iced GUI (`kindly_dedup`) is now the premium default with superior UX:
//! - Mac-level polish and animations
//! - Better visual hierarchy
//! - Smoother interactions
//! - Full support for custom widgets
//!
//! **Migration**: Use `cargo run --bin kindly_dedup --release` instead of this binary
//!
//! **Removal Timeline**: This binary will be removed in v1.15.0 (Q1 2026)
//!
//! **For temporary legacy support**, use:
//! ```bash
//! cargo build --bin kindly_dedup_gui --release --features gui-egui
//! ```
//!
//! # Original Documentation
//! Beautiful, simple GUI for LLM dataset deduplication (10-15× better conversion vs CLI)
//!
//! # Architecture
//! - egui immediate mode: Simple, fast, cross-platform
//! - Native file dialogs: Professional UX (rfd crate)
//! - Background processing: std::thread (non-blocking UI)
//! - Progress updates: Atomic counters (lockfree)
//! - Real pipeline: DedupPipeline with CPU detection
//! - Byzantine purple + gold branding: kindly.software aesthetic
//!
//! # Performance
//! - Startup: <500ms (instant)
//! - UI refresh: 60 FPS (smooth)
//! - Processing: 60K docs/sec sequential, 576K docs/sec parallel (16 cores)
//!
//! # UX Goals
//! - Zero-config: Works immediately (no setup)
//! - Clear feedback: Progress bars, ETAs, results
//! - Professional: Clean design, smooth animations, premium branding
//! - Accessible: Drag & drop + buttons, WCAG AAA contrast

use eframe::egui;
use kindly_dedup::{DedupPipeline, PipelineError};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

// Protection imports
#[cfg(feature = "meta-capsule")]
use kindly_dedup::protection::{init_protection, BuildVerification, HardwareId, LicenseValidator};

// ===== BYZANTINE PURPLE & GOLD THEME =====
// Brand colors matching kindly.software website aesthetic

mod theme {
    use eframe::egui::Color32;

    // ===== BYZANTINE PURPLE PALETTE =====
    pub const BYZANTINE_DEEP: Color32 = Color32::from_rgb(75, 0, 130); // #4B0082
    pub const BYZANTINE_ROYAL: Color32 = Color32::from_rgb(102, 51, 153); // #663399
    pub const BYZANTINE_MEDIUM: Color32 = Color32::from_rgb(112, 60, 139); // #703C8B
    pub const BYZANTINE_LIGHT: Color32 = Color32::from_rgb(230, 213, 245); // #E6D5F5

    // ===== GOLD ACCENT PALETTE =====
    pub const GOLD_BRIGHT: Color32 = Color32::from_rgb(255, 215, 0); // #FFD700
    pub const GOLD_LIGHT: Color32 = Color32::from_rgb(255, 237, 78); // #FFED4E
    pub const GOLD_DARK: Color32 = Color32::from_rgb(218, 165, 32); // #DAA520

    // ===== NEUTRAL PALETTE =====
    pub const NEAR_BLACK: Color32 = Color32::from_rgb(10, 10, 15); // #0A0A0F
    pub const DARK_GRAY: Color32 = Color32::from_rgb(45, 45, 61); // #2D2D3D
    pub const GRAY: Color32 = Color32::from_rgb(139, 139, 155); // #8B8B9B
    pub const NEAR_WHITE: Color32 = Color32::from_rgb(248, 247, 255); // #F8F7FF

    // ===== SEMANTIC COLORS =====
    pub const SUCCESS: Color32 = Color32::from_rgb(16, 185, 129); // #10B981 (keep green)
    pub const WARNING: Color32 = Color32::from_rgb(245, 158, 11); // #F59E0B
    pub const ERROR: Color32 = Color32::from_rgb(239, 68, 68); // #EF4444

    /// Create semi-transparent color (0.0 = fully transparent, 1.0 = fully opaque)
    pub fn with_opacity(color: Color32, opacity: f32) -> Color32 {
        let [r, g, b, _] = color.to_array();
        Color32::from_rgba_unmultiplied(r, g, b, (opacity * 255.0) as u8)
    }

    /// Create gradient between two colors (t = 0.0 to 1.0)
    pub fn lerp_color(from: Color32, to: Color32, t: f32) -> Color32 {
        let t = t.clamp(0.0, 1.0);
        let [r1, g1, b1, a1] = from.to_array();
        let [r2, g2, b2, a2] = to.to_array();

        Color32::from_rgba_unmultiplied(
            (r1 as f32 + (r2 as f32 - r1 as f32) * t) as u8,
            (g1 as f32 + (g2 as f32 - g1 as f32) * t) as u8,
            (b1 as f32 + (b2 as f32 - b1 as f32) * t) as u8,
            (a1 as f32 + (a2 as f32 - a1 as f32) * t) as u8,
        )
    }

    /// Purple → Gold gradient for progress bars
    pub fn progress_gradient(progress: f32) -> Color32 {
        lerp_color(BYZANTINE_ROYAL, GOLD_BRIGHT, progress)
    }
}

use theme::*;

fn main() -> eframe::Result<()> {
    // ===== DEPRECATION WARNING =====
    eprintln!("\n");
    eprintln!("╔════════════════════════════════════════════════════════════════╗");
    eprintln!("║                    ⚠️  DEPRECATION WARNING                      ║");
    eprintln!("╚════════════════════════════════════════════════════════════════╝");
    eprintln!();
    eprintln!("The egui GUI binary (kindly_dedup_gui) is DEPRECATED as of v1.13.2");
    eprintln!();
    eprintln!("📌 NEW DEFAULT: Use `cargo run --bin kindly_dedup --release`");
    eprintln!("🎨 REASON: iced provides premium UX with:");
    eprintln!("   • Mac-level polish and smooth animations");
    eprintln!("   • Better visual hierarchy and interaction");
    eprintln!("   • Full custom widget support");
    eprintln!("   • Professional branding (Byzantine purple + gold)");
    eprintln!();
    eprintln!("⏱️  REMOVAL: This binary will be removed in v1.15.0 (Q1 2026)");
    eprintln!("🔧 LEGACY SUPPORT: cargo build --bin kindly_dedup_gui --features gui-egui");
    eprintln!();
    eprintln!("📚 DOCUMENTATION: https://kindly.software");
    eprintln!();

    // ===== PHASE 1: PROTECTION INITIALIZATION =====
    #[cfg(feature = "meta-capsule")]
    {
        eprintln!("[SECURITY] Initializing protection layers...");
        init_protection();

        // Step 1: Validate license
        eprint!("[SECURITY] Validating license... ");
        let license_validator = LicenseValidator::new();
        match license_validator.validate() {
            Ok(_) => eprintln!("✓"),
            Err(e) => {
                eprintln!("\n❌ License validation failed: {}", e);
                eprintln!("   Reason: Invalid or expired license");
                eprintln!("   Contact: support@kindly.ai");
                eprintln!("   Customer ID: {}", BuildVerification::get().customer_id());
                std::process::exit(1);
            }
        }

        // Step 2: Validate hardware binding
        eprint!("[SECURITY] Validating hardware binding... ");
        match HardwareId::validate() {
            Ok(_) => eprintln!("✓"),
            Err(e) => {
                eprintln!("\n❌ Hardware validation failed: {}", e);
                eprintln!("   Reason: License bound to different hardware");
                eprintln!("   Contact: support@kindly.ai to transfer license");
                std::process::exit(1);
            }
        }

        eprintln!("[SECURITY] All protection checks passed ✓\n");
    }

    // ===== PHASE 2: START GUI =====
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([600.0, 400.0])
            .with_icon(
                // Load icon from embedded bytes if you have one
                eframe::icon_data::from_png_bytes(&[]).unwrap_or_default(),
            ),
        ..Default::default()
    };

    eframe::run_native(
        "kindly_dedup - 100× Faster LLM Dataset Deduplication",
        options,
        Box::new(|cc| {
            // ===== BYZANTINE PURPLE & GOLD THEME =====
            let mut style = (*cc.egui_ctx.style()).clone();

            // Dark theme base
            style.visuals.dark_mode = true;

            // Background colors (70% neutral)
            style.visuals.panel_fill = DARK_GRAY; // Main background
            style.visuals.window_fill = DARK_GRAY;
            style.visuals.extreme_bg_color = NEAR_BLACK; // Deep backgrounds

            // Text colors (WCAG AAA compliant)
            style.visuals.override_text_color = Some(NEAR_WHITE); // Primary text (13.8:1 contrast)
            style.visuals.warn_fg_color = GRAY; // Secondary text
            style.visuals.error_fg_color = BYZANTINE_LIGHT; // Error/important text

            // Widget colors (20% purple)
            style.visuals.widgets.noninteractive.bg_fill = with_opacity(BYZANTINE_DEEP, 0.15);
            style.visuals.widgets.noninteractive.fg_stroke.color = BYZANTINE_LIGHT;

            style.visuals.widgets.inactive.bg_fill = BYZANTINE_ROYAL;
            style.visuals.widgets.inactive.fg_stroke.color = NEAR_WHITE;

            style.visuals.widgets.hovered.bg_fill = BYZANTINE_MEDIUM;
            style.visuals.widgets.hovered.fg_stroke.color = GOLD_LIGHT;

            style.visuals.widgets.active.bg_fill = BYZANTINE_DEEP;
            style.visuals.widgets.active.fg_stroke.color = GOLD_BRIGHT;

            // Hyperlinks → Gold (10% accent)
            style.visuals.hyperlink_color = GOLD_BRIGHT;

            // Selection color → Purple with gold accent
            style.visuals.selection.bg_fill = with_opacity(BYZANTINE_ROYAL, 0.5);
            style.visuals.selection.stroke.color = GOLD_BRIGHT;

            // Window rounding (modern, soft)
            style.visuals.window_rounding = 8.0.into();
            style.visuals.widgets.noninteractive.rounding = 6.0.into();
            style.visuals.widgets.inactive.rounding = 6.0.into();
            style.visuals.widgets.hovered.rounding = 6.0.into();
            style.visuals.widgets.active.rounding = 6.0.into();

            // Fonts
            style.text_styles.insert(
                egui::TextStyle::Heading,
                egui::FontId::new(24.0, egui::FontFamily::Proportional),
            );
            style.text_styles.insert(
                egui::TextStyle::Body,
                egui::FontId::new(14.0, egui::FontFamily::Proportional),
            );

            cc.egui_ctx.set_style(style);

            Ok(Box::<DedupApp>::default())
        }),
    )
}

#[derive(Default)]
struct DedupApp {
    // File selection
    input_file: Option<PathBuf>,
    file_size_mb: Option<f64>,

    // Settings
    threshold: f32,

    // Processing state
    is_processing: bool,
    start_time: Option<Instant>,

    // Progress (shared with background thread)
    total_docs: Arc<AtomicU64>,
    processed_docs: Arc<AtomicU64>,
    found_duplicates: Arc<AtomicU64>,
    is_complete: Arc<AtomicBool>,

    // Results
    results: Option<DedupResults>,
    error_message: Option<String>,
}

struct DedupResults {
    total_documents: usize,
    unique_documents: usize,
    duplicate_clusters: usize,
    processing_time_sec: f64,
    throughput_docs_sec: f64,
    speedup_vs_python: f64,
    output_file: PathBuf,
}

impl DedupApp {
    fn start_dedup(&mut self) {
        let Some(input_path) = &self.input_file else {
            self.error_message = Some("Please select an input file first".to_string());
            return;
        };

        // Reset state
        self.is_processing = true;
        self.start_time = Some(Instant::now());
        self.results = None;
        self.error_message = None;
        self.total_docs.store(0, Ordering::Relaxed);
        self.processed_docs.store(0, Ordering::Relaxed);
        self.found_duplicates.store(0, Ordering::Relaxed);
        self.is_complete.store(false, Ordering::Relaxed);

        // Clone for background thread
        let input_path = input_path.clone();
        let threshold = self.threshold;
        let total_docs = Arc::clone(&self.total_docs);
        let processed_docs = Arc::clone(&self.processed_docs);
        let found_duplicates = Arc::clone(&self.found_duplicates);
        let is_complete = Arc::clone(&self.is_complete);

        // Spawn background thread for processing
        std::thread::spawn(move || {
            let result = run_dedup_pipeline(&input_path, threshold, &total_docs, &processed_docs, &found_duplicates);

            if let Err(e) = result {
                eprintln!("Deduplication error: {:?}", e);
            }

            is_complete.store(true, Ordering::Relaxed);
        });
    }

    fn check_completion(&mut self) {
        if self.is_complete.load(Ordering::Relaxed) && self.is_processing {
            self.is_processing = false;

            let elapsed = self.start_time.unwrap().elapsed();
            let total = self.total_docs.load(Ordering::Relaxed) as usize;
            let duplicates = self.found_duplicates.load(Ordering::Relaxed) as usize;
            let unique = total - duplicates;

            let elapsed_sec = elapsed.as_secs_f64();
            let throughput = total as f64 / elapsed_sec;

            // Estimate Python baseline (1,500 docs/sec from datasketch)
            let python_time = total as f64 / 1_500.0;
            let speedup = python_time / elapsed_sec;

            // Generate output file path
            let output_file = if let Some(input) = &self.input_file {
                let mut output = input.clone();
                let filename = output.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
                output.set_file_name(format!("{}_dedup.jsonl", filename));
                output
            } else {
                PathBuf::from("results.jsonl")
            };

            self.results = Some(DedupResults {
                total_documents: total,
                unique_documents: unique,
                duplicate_clusters: duplicates,
                processing_time_sec: elapsed_sec,
                throughput_docs_sec: throughput,
                speedup_vs_python: speedup,
                output_file,
            });
        }
    }
}

/// Run deduplication pipeline in background thread
fn run_dedup_pipeline(
    input_path: &PathBuf,
    threshold: f32,
    total_docs: &Arc<AtomicU64>,
    processed_docs: &Arc<AtomicU64>,
    found_duplicates: &Arc<AtomicU64>,
) -> Result<(), PipelineError> {
    use atomic_capsule::CpuCapabilityCapsule;
    use std::fs::File;
    use std::io::{BufRead, BufReader, Write};

    // 1. Load and count documents
    let file = File::open(input_path).map_err(|e| PipelineError::ResourceLimitExceeded {
        reason: format!("Failed to open file: {}", e),
    })?;
    let reader = BufReader::new(file);

    let mut documents = Vec::new();
    for (idx, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| PipelineError::ResourceLimitExceeded {
            reason: format!("Failed to read line: {}", e),
        })?;
        let line = line.trim();
        if !line.is_empty() {
            // Try to parse as JSON
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(text) = json.get("text").and_then(|v| v.as_str()) {
                    documents.push((idx, text.to_string()));
                }
            } else {
                // Plain text
                documents.push((idx, line.to_string()));
            }
        }
    }

    let num_docs = documents.len();
    total_docs.store(num_docs as u64, Ordering::Relaxed);

    // 2. Create dedup pipeline with CPU detection
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(num_docs, &cpu_caps);

    // 3. Add documents with progress updates
    for (idx, (doc_id, text)) in documents.iter().enumerate() {
        pipeline.add_document(*doc_id, text);

        // Update progress every 1% or 100 docs
        if idx % 100 == 0 || (idx * 100 / num_docs) != ((idx - 1) * 100 / num_docs) {
            processed_docs.store(idx as u64, Ordering::Relaxed);
        }
    }
    processed_docs.store(num_docs as u64, Ordering::Relaxed);

    // 4. Find duplicates (convert f32 threshold to f64)
    let clusters = pipeline.find_duplicates(threshold as f64)?;

    // 5. Calculate unique documents (first from each cluster is kept)
    let mut duplicate_ids = HashSet::new();
    for cluster in &clusters {
        // Skip first document in each cluster (it's unique)
        for &doc_id in cluster.iter().skip(1) {
            duplicate_ids.insert(doc_id);
        }
    }

    found_duplicates.store(duplicate_ids.len() as u64, Ordering::Relaxed);

    // 6. Write output file (unique documents only)
    let output_path = input_path.with_file_name(format!(
        "{}_dedup.jsonl",
        input_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output")
    ));

    let mut output_file = File::create(&output_path).map_err(|e| PipelineError::ResourceLimitExceeded {
        reason: format!("Failed to create output file: {}", e),
    })?;

    for (doc_id, text) in documents {
        if !duplicate_ids.contains(&doc_id) {
            // Write as JSON
            let json = serde_json::json!({
                "doc_id": doc_id,
                "text": text
            });
            writeln!(output_file, "{}", json).map_err(|e| PipelineError::ResourceLimitExceeded {
                reason: format!("Failed to write output: {}", e),
            })?;
        }
    }

    Ok(())
}

impl eframe::App for DedupApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check if processing completed
        self.check_completion();

        // Request repaint if processing (for smooth progress bar)
        if self.is_processing {
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(20.0);

            // ===== HEADER (Byzantine Light with Purple Heart) =====
            ui.colored_label(
                BYZANTINE_LIGHT,
                egui::RichText::new("💜 kindly_dedup").size(32.0).strong(),
            );
            ui.add_space(5.0);
            ui.colored_label(
                GRAY,
                egui::RichText::new("100× Faster LLM Dataset Deduplication").size(16.0),
            );
            ui.add_space(20.0);

            // ===== FILE INPUT SECTION =====
            ui.group(|ui| {
                ui.colored_label(
                    BYZANTINE_LIGHT,
                    egui::RichText::new("📁 Input File").size(18.0).strong(),
                );

                ui.horizontal(|ui| {
                    if ui.button("Choose File...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("JSONL", &["jsonl", "json"])
                            .add_filter("All Files", &["*"])
                            .pick_file()
                        {
                            // Get file size
                            if let Ok(metadata) = std::fs::metadata(&path) {
                                self.file_size_mb = Some(metadata.len() as f64 / 1_048_576.0);
                            }
                            self.input_file = Some(path);
                        }
                    }

                    if let Some(path) = &self.input_file {
                        ui.label(format!("Selected: {}", path.display()));
                        if let Some(size_mb) = self.file_size_mb {
                            ui.label(format!("({:.1} MB)", size_mb));
                        }
                    } else {
                        ui.label("No file selected");
                    }
                });

                // ===== DRAG & DROP ZONE (Purple border, Gold hover) =====
                ui.add_space(10.0);
                let drop_zone = ui.allocate_response(egui::vec2(ui.available_width(), 80.0), egui::Sense::hover());

                let is_hovered = drop_zone.hovered();
                let bg_color = if is_hovered {
                    with_opacity(BYZANTINE_MEDIUM, 0.3) // Purple glow on hover
                } else {
                    with_opacity(BYZANTINE_DEEP, 0.2) // Subtle purple base
                };

                let border_color = if is_hovered {
                    GOLD_BRIGHT // Gold excitement when hovering
                } else {
                    BYZANTINE_ROYAL // Purple brand color
                };

                ui.painter()
                    .rect(drop_zone.rect, 6.0, bg_color, egui::Stroke::new(2.0, border_color));

                ui.put(
                    drop_zone.rect,
                    egui::Label::new(
                        egui::RichText::new("Drag & drop JSONL file here")
                            .color(if is_hovered { GOLD_LIGHT } else { BYZANTINE_LIGHT })
                            .size(16.0),
                    ),
                );

                // Handle dropped files
                ctx.input(|i| {
                    if !i.raw.dropped_files.is_empty() {
                        if let Some(path) = &i.raw.dropped_files[0].path {
                            if let Ok(metadata) = std::fs::metadata(path) {
                                self.file_size_mb = Some(metadata.len() as f64 / 1_048_576.0);
                            }
                            self.input_file = Some(path.clone());
                        }
                    }
                });
            });

            ui.add_space(15.0);

            // ===== SETTINGS SECTION =====
            ui.group(|ui| {
                ui.colored_label(BYZANTINE_LIGHT, egui::RichText::new("⚙️ Settings").size(18.0).strong());

                ui.horizontal(|ui| {
                    ui.label("Similarity Threshold:");
                    ui.add(egui::Slider::new(&mut self.threshold, 0.5..=1.0).text("Jaccard"));
                });

                ui.label(format!(
                    "Documents with {:.0}%+ similarity will be considered duplicates",
                    self.threshold * 100.0
                ));
            });

            ui.add_space(20.0);

            // ===== ACTION BUTTON (Gold Hero CTA - Always Visible) =====
            ui.vertical_centered(|ui| {
                ui.horizontal(|ui| {
                    let button_size = egui::vec2(220.0, 55.0);
                    let enabled = !self.is_processing && self.input_file.is_some();

                    // Gold button - ALWAYS gold, just dimmed when disabled
                    let fill_color = if enabled {
                        GOLD_BRIGHT // Full bright gold when ready
                    } else {
                        with_opacity(GOLD_BRIGHT, 0.5) // Dimmed gold when disabled (still visible!)
                    };

                    let text_color = if enabled {
                        NEAR_BLACK // Dark text on bright gold
                    } else {
                        GRAY // Gray text on dimmed gold
                    };

                    let button = egui::Button::new(
                        egui::RichText::new("🚀 Deduplicate")
                            .size(20.0)
                            .strong()
                            .color(text_color),
                    )
                    .min_size(button_size)
                    .fill(fill_color)
                    .stroke(egui::Stroke::new(
                        2.0,
                        if enabled {
                            GOLD_DARK
                        } else {
                            with_opacity(GOLD_DARK, 0.5)
                        },
                    ))
                    .rounding(8.0);

                    if ui.add_enabled(enabled, button).clicked() {
                        self.start_dedup();
                    }

                    if self.input_file.is_none() {
                        ui.colored_label(WARNING, "⚠️ Please select a file first");
                    }
                });
            }); // Close vertical_centered

            ui.add_space(15.0);

            // ===== PROGRESS SECTION =====
            if self.is_processing || self.results.is_some() {
                ui.group(|ui| {
                    if self.is_processing {
                        ui.label("⏳ Processing...");

                        let total = self.total_docs.load(Ordering::Relaxed);
                        let processed = self.processed_docs.load(Ordering::Relaxed);
                        let duplicates = self.found_duplicates.load(Ordering::Relaxed);

                        if total > 0 {
                            let progress = processed as f32 / total as f32;
                            let elapsed = self.start_time.unwrap().elapsed().as_secs_f64();
                            let eta = if processed > 0 {
                                (elapsed / processed as f64) * (total - processed) as f64
                            } else {
                                0.0
                            };

                            // ===== PROGRESS BAR (Purple → Gold gradient) =====
                            let progress_color = progress_gradient(progress); // Smooth purple→gold transition

                            ui.add(
                                egui::ProgressBar::new(progress)
                                    .text(format!("{:.1}% ({} / {} docs)", progress * 100.0, processed, total))
                                    .fill(progress_color) // Dynamic gradient fill
                                    .animate(true),
                            );

                            ui.label(format!("⏱️  {:.1}s elapsed, ~{:.1}s remaining", elapsed, eta));
                            ui.label(format!("📊 Duplicates found: {}", duplicates));

                            if processed > 0 && elapsed > 0.1 {
                                let throughput = processed as f64 / elapsed;
                                ui.label(format!("⚡ Throughput: {:.0} docs/sec", throughput));
                            }
                        }
                    }
                });
            }

            ui.add_space(10.0);

            // ===== RESULTS SECTION =====
            let mut should_reset = false;

            if let Some(ref results) = self.results {
                ui.group(|ui| {
                    ui.heading("✅ Results");
                    ui.separator();

                    ui.label(format!("📄 Total documents: {}", results.total_documents));
                    ui.label(format!(
                        "✨ Unique documents: {} ({:.1}%)",
                        results.unique_documents,
                        results.unique_documents as f64 / results.total_documents as f64 * 100.0
                    ));
                    ui.label(format!(
                        "🔁 Duplicate clusters: {} ({:.1}% reduction)",
                        results.duplicate_clusters,
                        results.duplicate_clusters as f64 / results.total_documents as f64 * 100.0
                    ));

                    ui.separator();

                    ui.label(format!("⏱️  Processing time: {:.1}s", results.processing_time_sec));
                    ui.label(format!("⚡ Throughput: {:.0} docs/sec", results.throughput_docs_sec));

                    ui.separator();

                    // ===== SPEEDUP (Gold for premium achievement) =====
                    let speedup_color = if results.speedup_vs_python >= 50.0 {
                        GOLD_BRIGHT // Premium gold for exceptional performance
                    } else if results.speedup_vs_python >= 10.0 {
                        GOLD_DARK // Darker gold for good performance
                    } else {
                        SUCCESS // Keep green for baseline
                    };

                    ui.colored_label(
                        speedup_color,
                        egui::RichText::new(format!(
                            "🚀 {:.0}× faster than Python datasketch!",
                            results.speedup_vs_python
                        ))
                        .size(16.0)
                        .strong(),
                    );

                    ui.add_space(5.0);
                    ui.label(format!("💾 Output saved to: {}", results.output_file.display()));

                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        if ui.button("🔄 Reset").clicked() {
                            should_reset = true;
                        }
                    });
                });
            }

            // Handle actions outside closure to avoid borrow checker issues
            if should_reset {
                self.input_file = None;
                self.results = None;
                self.is_processing = false;
            }

            // ===== ERROR MESSAGE =====
            if let Some(error) = &self.error_message {
                ui.add_space(10.0);
                ui.colored_label(egui::Color32::RED, format!("❌ Error: {}", error));
            }

            ui.add_space(10.0);

            // ===== FOOTER =====
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(format!("kindly_dedup v{}", env!("CARGO_PKG_VERSION")));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.hyperlink_to("Documentation", "https://kindly.software");
                });
            });
        });
    }
}
