// Minimal test to isolate potential issues
use kindly_dedup::{Dedup, DedupMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Step 1: Creating Dedup instance with CpuStreaming mode...");
    let _dedup = Dedup::with_mode(DedupMode::CpuStreaming, 10)?;
    println!("Step 2: Dedup instance created successfully!");

    println!("Step 3: Creating Dedup instance with Auto mode...");
    let _dedup2 = Dedup::new(10)?;
    println!("Step 4: Auto mode instance created successfully!");

    println!("Step 5: Test completed - no crashes!");
    Ok(())
}
