// Minimal test to isolate the segfault
use atomic_capsule::CpuCapabilityCapsule;
use kindly_dedup::DedupPipeline;

fn main() {
    println!("Step 1: Detecting CPU capabilities...");
    let cpu_caps = CpuCapabilityCapsule::detect();
    println!("Step 2: CPU detected - {}", cpu_caps.best_simd_tier());

    println!("Step 3: Creating DedupPipeline for 10 documents...");
    let _pipeline = DedupPipeline::new(10, &cpu_caps);
    println!("Step 4: Pipeline created successfully!");
    println!("Step 5: Test completed!");
}
