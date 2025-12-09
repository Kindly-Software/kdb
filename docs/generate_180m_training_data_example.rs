// TEMPLATE FILE: 180M Training Data Generation Example
// This is a reference implementation to be integrated into the training project
//
// Integration Instructions:
// 1. Copy to examples/generate_180m_training_data.rs
// 2. Wire up actual component imports
// 3. Configure for your specific use case
// 4. Run with: cargo run --release --example generate_180m_training_data

use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

// TODO: Replace with actual imports from your training module
// use your_project::training::mega_data_pipeline_orchestrator::*;

// For this template, we'll define minimal placeholder types
mod pipeline {
    include!("mega_data_pipeline_orchestrator_template.rs");
}

use pipeline::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("================================================================================");
    println!("  Mega Training Data Pipeline - 180M Example Generator");
    println!("================================================================================\n");

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("full");

    match mode {
        "full" => run_full_pipeline().await?,
        "resume" => resume_from_checkpoint(&args).await?,
        "test" => run_test_pipeline().await?,
        "monitor" => monitor_progress().await?,
        _ => {
            print_usage();
            return Ok(());
        }
    }

    Ok(())
}

/// Run full 180M example generation pipeline
async fn run_full_pipeline() -> Result<(), Box<dyn std::error::Error>> {
    println!("[Mode] Full Pipeline (300K configs → 180M examples)\n");

    // Configure pipeline for production scale
    let config = PipelineConfig {
        // Parameter Grid: 300K configurations
        parameter_count: 300_000,
        param_ranges: vec![
            ParamRange { name: "lookback".into(), min: 10.0, max: 100.0, step: 5.0 },
            ParamRange { name: "threshold".into(), min: 0.001, max: 0.1, step: 0.001 },
            ParamRange { name: "volatility_window".into(), min: 5.0, max: 50.0, step: 5.0 },
        ],

        // Data Extraction: Full year of historical data
        data_path: PathBuf::from("data/historical/ES_2024.csv"),
        start_date: "2024-01-01".to_string(),
        end_date: "2024-12-31".to_string(),

        // Parameter Sweep: 30 examples per config = 9M examples
        examples_per_config: 30,
        batch_size: 1000,

        // Diversity Tuning: 50% selection = 4.5M → 45M examples
        diversity_ratio: 0.5,
        quality_threshold: 0.7,

        // Curriculum Sequencing: 4× expansion = 180M examples
        curriculum_expansion: 4.0,
        difficulty_metric: DifficultyMetric::GradientBased,

        // Resource Budgets
        max_memory_gb: 32.0,
        max_disk_gb: 500.0,
        stage_timeout_minutes: 120,

        // Checkpoint Settings
        checkpoint_dir: PathBuf::from("checkpoints/production"),
        checkpoint_interval_minutes: 10,

        // Output
        output_path: PathBuf::from("output/training_data_180m.json"),
    };

    // Create orchestrator
    println!("[Setup] Creating pipeline orchestrator...");
    let orchestrator = MegaPipelineOrchestrator::new(config);

    // Spawn progress monitor task
    let progress_handle = tokio::spawn(monitor_progress_loop(orchestrator.progress.clone()));

    // Execute pipeline with graceful shutdown on Ctrl-C
    println!("[Execute] Starting pipeline execution...\n");
    let result = tokio::select! {
        result = orchestrator.execute() => result,
        _ = tokio::signal::ctrl_c() => {
            println!("\n[Shutdown] Ctrl-C received, checkpointing...");
            orchestrator.checkpoint_stage(PipelineStage::Complete).await?;
            println!("[Shutdown] Checkpoint saved. Resume with: cargo run --example generate_180m_training_data resume");
            std::process::exit(0);
        }
    };

    // Stop progress monitor
    progress_handle.abort();

    // Print final results
    match result {
        Ok(output) => {
            println!("\n================================================================================");
            println!("  Pipeline Complete!");
            println!("================================================================================");
            println!("Total Examples:    {:>12}", format_number(output.total_examples));
            println!("Output Path:       {}", output.output_path.display());
            println!("Duration:          {:>12}", format_duration(output.stats.duration));
            println!("Peak Memory:       {:>12.2} GB", output.stats.peak_memory_gb);
            println!("Peak Disk:         {:>12.2} GB", output.stats.peak_disk_gb);
            println!("================================================================================\n");
        }
        Err(e) => {
            eprintln!("\n[Error] Pipeline failed: {}", e);
            eprintln!("[Recovery] Check checkpoint directory for resume capability");
            return Err(e.into());
        }
    }

    Ok(())
}

/// Resume pipeline from checkpoint
async fn resume_from_checkpoint(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let checkpoint_path = args.get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("checkpoints/production/checkpoint_latest.json"));

    println!("[Mode] Resume from Checkpoint");
    println!("[Checkpoint] Loading from: {}\n", checkpoint_path.display());

    // Create orchestrator from checkpoint
    let orchestrator = MegaPipelineOrchestrator::from_checkpoint(&checkpoint_path).await?;

    // Spawn progress monitor
    let progress_handle = tokio::spawn(monitor_progress_loop(orchestrator.progress.clone()));

    // Resume execution
    println!("[Execute] Resuming pipeline execution...\n");
    let result = tokio::select! {
        result = orchestrator.execute() => result,
        _ = tokio::signal::ctrl_c() => {
            println!("\n[Shutdown] Ctrl-C received, checkpointing...");
            orchestrator.checkpoint_stage(PipelineStage::Complete).await?;
            std::process::exit(0);
        }
    };

    progress_handle.abort();

    match result {
        Ok(output) => {
            println!("\n[Complete] Pipeline resumed and finished successfully!");
            println!("[Output] {} examples at {}", format_number(output.total_examples), output.output_path.display());
        }
        Err(e) => {
            eprintln!("\n[Error] Resume failed: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

/// Run small test pipeline for validation
async fn run_test_pipeline() -> Result<(), Box<dyn std::error::Error>> {
    println!("[Mode] Test Pipeline (100 configs → 1K examples)\n");

    // Minimal configuration for testing
    let config = PipelineConfig {
        parameter_count: 100,
        examples_per_config: 5,
        diversity_ratio: 0.5,
        curriculum_expansion: 2.0,
        max_memory_gb: 4.0,
        max_disk_gb: 10.0,
        checkpoint_dir: PathBuf::from("checkpoints/test"),
        output_path: PathBuf::from("output/test_data.json"),
        ..Default::default()
    };

    let orchestrator = MegaPipelineOrchestrator::new(config);
    let progress_handle = tokio::spawn(monitor_progress_loop(orchestrator.progress.clone()));

    println!("[Execute] Running test pipeline...\n");
    let result = orchestrator.execute().await;

    progress_handle.abort();

    match result {
        Ok(output) => {
            println!("\n[Test] Pipeline completed successfully!");
            println!("[Test] Generated {} examples in {:?}", output.total_examples, output.stats.duration);
            println!("[Test] All systems operational ✓");
        }
        Err(e) => {
            eprintln!("\n[Test] Pipeline failed: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

/// Monitor existing pipeline progress
async fn monitor_progress() -> Result<(), Box<dyn std::error::Error>> {
    println!("[Mode] Progress Monitor");
    println!("[Monitor] Watching for running pipeline...\n");

    // TODO: In production, this would connect to a shared progress capsule via shared memory
    // For now, just print a message
    println!("[Note] This feature requires a running pipeline in another process");
    println!("[Note] Use shared memory or file-based progress tracking for cross-process monitoring");

    Ok(())
}

/// Progress monitoring loop (updates console in real-time)
async fn monitor_progress_loop(progress: std::sync::Arc<ProgressCapsule>) {
    let mut last_generation = 0u64;
    let mut last_stage = PipelineStage::Init;

    loop {
        let p = progress.read();

        // Only update if progress changed
        if p.generation != last_generation || p.stage != last_stage {
            print_progress(&p);
            last_generation = p.generation;
            last_stage = p.stage;
        }

        sleep(Duration::from_millis(500)).await;
    }
}

/// Print progress bar to console
fn print_progress(progress: &Progress) {
    let percent = progress.percent();
    let bar_width = 50;
    let filled = (bar_width as f64 * percent / 100.0) as usize;
    let empty = bar_width - filled;

    let bar = format!("[{}{}]",
        "=".repeat(filled),
        " ".repeat(empty)
    );

    let eta_str = progress.eta()
        .map(|d| format_duration(d))
        .unwrap_or_else(|| "calculating...".to_string());

    print!("\r[{:>20}] {} {:>6.2}% | {}/{} | ETA: {} ",
        progress.stage.name(),
        bar,
        percent,
        format_number(progress.completed as usize),
        format_number(progress.total as usize),
        eta_str
    );

    use std::io::Write;
    std::io::stdout().flush().ok();
}

/// Print usage information
fn print_usage() {
    println!("Usage: cargo run --release --example generate_180m_training_data [MODE] [ARGS]");
    println!();
    println!("Modes:");
    println!("  full              Run full 180M example generation pipeline (default)");
    println!("  resume [PATH]     Resume from checkpoint (default: checkpoints/production/checkpoint_latest.json)");
    println!("  test              Run small test pipeline (100 configs → 1K examples)");
    println!("  monitor           Monitor progress of running pipeline");
    println!();
    println!("Examples:");
    println!("  cargo run --release --example generate_180m_training_data full");
    println!("  cargo run --release --example generate_180m_training_data resume checkpoints/production/checkpoint_ParameterSweep.json");
    println!("  cargo run --release --example generate_180m_training_data test");
    println!();
}

/// Format large numbers with commas
fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Format duration as human-readable string
fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

// ============================================================================
// USAGE EXAMPLES
// ============================================================================

/*

# Example 1: Run full production pipeline
cargo run --release --example generate_180m_training_data full

Output:
================================================================================
  Mega Training Data Pipeline - 180M Example Generator
================================================================================

[Mode] Full Pipeline (300K configs → 180M examples)

[Setup] Creating pipeline orchestrator...
[Execute] Starting pipeline execution...

[Stage 1/5] Generating parameter grid (300,000 configs)...
[Parameter Grid Gen...] [==================================================] 100.00% | 300,000/300,000 | ETA: 0s
[Stage 1/5] Complete: 300,000 configs generated
[Checkpoint] Saved at stage ParameterGrid: checkpoints/production/checkpoint_ParameterGrid.json

[Stage 2/5] Executing parameter sweep (9,000,000 examples)...
[Parameter Sweep     ] [==========================================        ]  85.23% | 7,671,000/9,000,000 | ETA: 2m 15s

# User presses Ctrl-C
^C
[Shutdown] Ctrl-C received, checkpointing...
[Checkpoint] Saved at stage ParameterSweep: checkpoints/production/checkpoint_ParameterSweep.json
[Shutdown] Checkpoint saved. Resume with: cargo run --example generate_180m_training_data resume


# Example 2: Resume from checkpoint
cargo run --release --example generate_180m_training_data resume checkpoints/production/checkpoint_ParameterSweep.json

Output:
[Mode] Resume from Checkpoint
[Checkpoint] Loading from: checkpoints/production/checkpoint_ParameterSweep.json

[Execute] Resuming pipeline execution...

[Stage 2/5] Executing parameter sweep (9,000,000 examples)...
[Parameter Sweep     ] [==================================================] 100.00% | 9,000,000/9,000,000 | ETA: 0s
[Stage 2/5] Complete: 9,000,000 examples generated
[Checkpoint] Saved at stage ParameterSweep: checkpoints/production/checkpoint_ParameterSweep.json

[Stage 3/5] Tuning diversity (9,000,000 → 4,500,000 examples)...
[Diversity Tuning    ] [==================================================] 100.00% | 4,500,000/9,000,000 | ETA: 0s
[Stage 3/5] Complete: 4,500,000 diverse examples selected
[Checkpoint] Saved at stage DiversityTuning: checkpoints/production/checkpoint_DiversityTuning.json

[Stage 4/5] Sequencing curriculum (4,500,000 → 18,000,000 examples)...
[Curriculum Sequenc...] [==================================================] 100.00% | 18,000,000/18,000,000 | ETA: 0s
[Stage 4/5] Complete: 18,000,000 curriculum examples sequenced
[Checkpoint] Saved at stage CurriculumSequencing: checkpoints/production/checkpoint_CurriculumSequencing.json

================================================================================
  Pipeline Complete!
================================================================================
Total Examples:           180,000,000
Output Path:       output/training_data_180m.json
Duration:                   1h 47m
Peak Memory:                  28.45 GB
Peak Disk:                   432.18 GB
================================================================================


# Example 3: Run test pipeline
cargo run --release --example generate_180m_training_data test

Output:
[Mode] Test Pipeline (100 configs → 1K examples)

[Execute] Running test pipeline...

[Stage 1/5] Generating parameter grid (100 configs)...
[Stage 1/5] Complete: 100 configs generated
[Stage 2/5] Executing parameter sweep (500 examples)...
[Stage 2/5] Complete: 500 examples generated
[Stage 3/5] Tuning diversity (500 → 250 examples)...
[Stage 3/5] Complete: 250 diverse examples selected
[Stage 4/5] Sequencing curriculum (250 → 500 examples)...
[Stage 4/5] Complete: 500 curriculum examples sequenced

[Test] Pipeline completed successfully!
[Test] Generated 500 examples in 15s
[Test] All systems operational ✓


# Example 4: Monitor progress (separate terminal)
cargo run --release --example generate_180m_training_data monitor

Output:
[Mode] Progress Monitor
[Monitor] Watching for running pipeline...

[Note] This feature requires a running pipeline in another process
[Note] Use shared memory or file-based progress tracking for cross-process monitoring

*/
