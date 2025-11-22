// GPU Acceleration Foundation Demo
// Phase 5: T7 Heterogeneous Tier
//
// Demonstrates:
// - CUDA/ROCm capsule initialization
// - Multi-GPU coordination (lockfree round-robin)
// - Graceful degradation (CPU fallback)
// - Performance metrics (kernel launches, utilization)

#[cfg(any(feature = "gpu-cuda", feature = "gpu-rocm"))]
use atomic_capsule::gpu::{CudaComputeCapsule, GpuCoordinator, RocmComputeCapsule};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== GPU Acceleration Foundation Demo ===\n");

    // Check GPU availability
    #[cfg(any(feature = "gpu-cuda", feature = "gpu-rocm"))]
    {
        let device_count = atomic_capsule::gpu::device_count()?;
        let backend = atomic_capsule::gpu::backend();

        println!("GPU Backend: {:?}", backend);
        println!("GPU Devices: {}", device_count);

        if device_count == 0 {
            println!("\n⚠️  No GPU devices available - CPU fallback mode");
            println!("   To use GPU acceleration:");
            println!("   - Install CUDA Toolkit 11.0+ (NVIDIA GPU)");
            println!("   - Install ROCm 5.0+ (AMD GPU)");
            println!("   - Ensure GPU drivers are up to date");
            return Ok(());
        }

        println!("\n--- Single GPU Demo ---");
        demo_single_gpu()?;

        if device_count >= 2 {
            println!("\n--- Multi-GPU Demo ---");
            demo_multi_gpu(device_count)?;
        } else {
            println!("\n⚠️  Multi-GPU demo requires 2+ GPUs (only {} available)", device_count);
        }

        println!("\n--- Graceful Degradation Demo ---");
        demo_graceful_degradation();
    }

    #[cfg(not(any(feature = "gpu-cuda", feature = "gpu-rocm")))]
    {
        println!("⚠️  GPU features not enabled");
        println!("   Compile with:");
        println!("   - cargo run --example gpu_foundation_demo --features gpu-cuda");
        println!("   - cargo run --example gpu_foundation_demo --features gpu-rocm");
    }

    Ok(())
}

#[cfg(any(feature = "gpu-cuda", feature = "gpu-rocm"))]
fn demo_single_gpu() -> Result<(), Box<dyn std::error::Error>> {
    println!("Creating CUDA capsule for device 0...");

    #[cfg(feature = "gpu-cuda")]
    {
        match CudaComputeCapsule::new(0) {
            Ok(mut capsule) => {
                println!("✅ CUDA capsule initialized");
                println!("   Device ID: {}", capsule.device_id());
                println!("   Kernel launches: {}", capsule.kernel_launches());
                println!("   Completed kernels: {}", capsule.completed_kernels());

                // Set launch configuration
                capsule.set_launch_config((100, 1, 1), (256, 1, 1), 0);
                println!("\n   Launch config:");
                println!("   - Grid: {:?}", capsule.grid_dim());
                println!("   - Block: {:?}", capsule.block_dim());
                println!("   - Shared memory: {} bytes", capsule.shared_mem_bytes());

                // Synchronize
                capsule.synchronize()?;
                println!("\n✅ Stream synchronized");
                println!("   Completed kernels: {}", capsule.completed_kernels());
            }
            Err(e) => {
                println!("❌ Failed to initialize CUDA capsule: {}", e);
            }
        }
    }

    #[cfg(all(feature = "gpu-rocm", not(feature = "gpu-cuda")))]
    {
        match RocmComputeCapsule::new(0) {
            Ok(mut capsule) => {
                println!("✅ ROCm capsule initialized");
                println!("   Device ID: {}", capsule.device_id());

                capsule.set_launch_config((100, 1, 1), (256, 1, 1), 0);
                println!("\n   Launch config:");
                println!("   - Grid: {:?}", capsule.grid_dim());
                println!("   - Block: {:?}", capsule.block_dim());
            }
            Err(e) => {
                println!("❌ Failed to initialize ROCm capsule: {}", e);
                println!("   Note: ROCm backend FFI bindings pending implementation");
            }
        }
    }

    Ok(())
}

#[cfg(any(feature = "gpu-cuda", feature = "gpu-rocm"))]
fn demo_multi_gpu(device_count: u32) -> Result<(), Box<dyn std::error::Error>> {
    println!("Creating GPU coordinator for {} devices...", device_count);

    let coordinator = GpuCoordinator::new(device_count)?;
    println!("✅ GPU coordinator initialized");
    println!("   Backend: {:?}", coordinator.backend());
    println!("   Devices: {}", coordinator.device_count());

    // Simulate workload distribution
    println!("\nDistributing 100 tasks across {} GPUs...", device_count);

    for i in 0..100 {
        let device_id = coordinator.next_device();

        if i < 5 {
            println!("   Task {} → Device {}", i, device_id);
        } else if i == 5 {
            println!("   ... (95 more tasks)");
        }
    }

    println!("\n--- Utilization Metrics ---");
    for device_id in 0..device_count {
        let utilization = coordinator.utilization(device_id)?;
        println!("   Device {}: {} tasks", device_id, utilization);
    }

    let load_balance = coordinator.load_balance_factor();
    println!("\n   Load balance factor: {:.3}", load_balance);

    if load_balance < 1.05 {
        println!("   ✅ Load well-balanced (<5% imbalance)");
    } else {
        println!("   ⚠️  Load imbalance detected (>{:.1}%)", (load_balance - 1.0) * 100.0);
    }

    println!("\n   Total tasks: {}", coordinator.total_tasks());

    Ok(())
}

#[cfg(any(feature = "gpu-cuda", feature = "gpu-rocm"))]
fn demo_graceful_degradation() {
    println!("Attempting to create capsule for invalid device ID...");

    #[cfg(feature = "gpu-cuda")]
    {
        match CudaComputeCapsule::new(999) {
            Ok(_) => println!("❌ Unexpected success (device 999 should not exist)"),
            Err(e) => {
                println!("✅ Graceful error handling:");
                println!("   Error: {}", e);
                println!("   → Application continues with CPU fallback");
            }
        }
    }

    #[cfg(all(feature = "gpu-rocm", not(feature = "gpu-cuda")))]
    {
        match RocmComputeCapsule::new(999) {
            Ok(_) => println!("❌ Unexpected success (device 999 should not exist)"),
            Err(e) => {
                println!("✅ Graceful error handling:");
                println!("   Error: {}", e);
            }
        }
    }
}
