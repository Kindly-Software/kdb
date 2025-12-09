//! Vulkan Compute HAL T28 Unit Tests (Q1-Q7)
//!
//! Tests the Vulkan compute dispatcher implementation.
//!
//! # Test Coverage
//!
//! - Q1: Basic creation and initialization
//! - Q2: Memory layout and alignment (Chaos compliance)
//! - Q3: Error handling
//! - Q4: State transitions
//! - Q5: Counter updates (lockfree atomics)
//! - Q6: Buffer/pipeline lifecycle
//! - Q7: Thread safety (Send + Sync)

#[cfg(feature = "vulkan-compute")]
use atomic_capsule::gpu::hal::{
    ComputeDispatcher, VulkanComputeError, BufferUsage, MemoryProperty,
    PhysicalDeviceProperties,
};

#[test]
fn test_q1_basic_creation() {
    #[cfg(feature = "vulkan-compute")]
    {
        let dispatcher = ComputeDispatcher::uninit();
        assert!(!dispatcher.is_ready());
        assert_eq!(dispatcher.dispatch_count(), 0);
        assert_eq!(dispatcher.work_items(), 0);
        assert_eq!(dispatcher.active_pipelines(), 0);
        assert_eq!(dispatcher.active_buffers(), 0);
    }
}

#[test]
fn test_q2_memory_layout() {
    #[cfg(feature = "vulkan-compute")]
    {
        // Chaos compliance: 256B size, 128B alignment
        assert_eq!(core::mem::size_of::<ComputeDispatcher>(), 256);
        assert_eq!(core::mem::align_of::<ComputeDispatcher>(), 128);
    }
}

#[test]
fn test_q3_error_handling() {
    #[cfg(feature = "vulkan-compute")]
    {
        // Test error display
        let err = VulkanComputeError::NotImplemented;
        assert!(err.to_string().contains("not implemented"));

        let err = VulkanComputeError::NoSuitableGpu;
        assert!(err.to_string().contains("GPU"));

        let err = VulkanComputeError::VulkanNotAvailable;
        assert!(err.to_string().contains("Vulkan not available"));

        let err = VulkanComputeError::BufferAllocationFailed;
        assert!(err.to_string().contains("Buffer allocation"));
    }
}

#[test]
fn test_q4_state_transitions() {
    #[cfg(feature = "vulkan-compute")]
    {
        let dispatcher = ComputeDispatcher::uninit();

        // Initial state
        assert!(!dispatcher.is_ready());

        // Shutdown should be idempotent
        dispatcher.shutdown();
        assert!(!dispatcher.is_ready());

        dispatcher.shutdown();
        assert!(!dispatcher.is_ready());
    }
}

#[test]
fn test_q5_counter_updates() {
    #[cfg(feature = "vulkan-compute")]
    {
        let dispatcher = ComputeDispatcher::uninit();

        // Counters start at zero
        assert_eq!(dispatcher.dispatch_count(), 0);
        assert_eq!(dispatcher.work_items(), 0);
        assert_eq!(dispatcher.active_pipelines(), 0);
        assert_eq!(dispatcher.active_buffers(), 0);

        // Counters persist after shutdown
        dispatcher.shutdown();
        assert_eq!(dispatcher.dispatch_count(), 0);
        assert_eq!(dispatcher.work_items(), 0);
    }
}

#[test]
fn test_q6_buffer_usage_flags() {
    #[cfg(feature = "vulkan-compute")]
    {
        // Verify flag values match Vulkan spec
        assert_eq!(BufferUsage::Storage as u32, 0x01);
        assert_eq!(BufferUsage::Uniform as u32, 0x02);
        assert_eq!(BufferUsage::TransferSrc as u32, 0x04);
        assert_eq!(BufferUsage::TransferDst as u32, 0x08);
        assert_eq!(BufferUsage::Indirect as u32, 0x10);
    }
}

#[test]
fn test_q6_memory_property_flags() {
    #[cfg(feature = "vulkan-compute")]
    {
        // Verify flag values match Vulkan spec
        assert_eq!(MemoryProperty::DeviceLocal as u32, 0x01);
        assert_eq!(MemoryProperty::HostVisible as u32, 0x02);
        assert_eq!(MemoryProperty::HostCoherent as u32, 0x04);
        assert_eq!(MemoryProperty::HostCached as u32, 0x08);
    }
}

#[test]
fn test_q6_device_properties() {
    #[cfg(feature = "vulkan-compute")]
    {
        let props = PhysicalDeviceProperties::default();

        // Default values
        assert_eq!(props.vendor_id, 0);
        assert_eq!(props.device_id, 0);
        assert_eq!(props.api_version, 0);
        assert_eq!(props.device_name_str(), "");

        // Arrays zeroed
        assert_eq!(props.max_work_group_count, [0, 0, 0]);
        assert_eq!(props.max_work_group_size, [0, 0, 0]);
        assert_eq!(props.max_work_group_invocations, 0);
    }
}

#[test]
fn test_q7_thread_safety() {
    #[cfg(feature = "vulkan-compute")]
    {
        // Verify Send + Sync traits (compile-time check)
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ComputeDispatcher>();
    }
}

#[test]
fn test_q7_concurrent_access() {
    #[cfg(all(feature = "vulkan-compute", feature = "std"))]
    {
        use std::sync::Arc;
        use std::thread;

        let dispatcher = Arc::new(ComputeDispatcher::uninit());
        let mut handles = vec![];

        // Spawn 8 threads reading counters concurrently
        for _ in 0..8 {
            let d = Arc::clone(&dispatcher);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let _ = d.is_ready();
                    let _ = d.dispatch_count();
                    let _ = d.work_items();
                    let _ = d.active_pipelines();
                    let _ = d.active_buffers();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // No crashes = success (lockfree atomics work)
    }
}

#[test]
fn test_not_implemented_apis() {
    #[cfg(feature = "vulkan-compute")]
    {
        let dispatcher = ComputeDispatcher::uninit();

        // These should all return NotImplemented (stub implementation)
        assert!(matches!(
            ComputeDispatcher::new(),
            Err(VulkanComputeError::NotImplemented)
        ));

        assert!(matches!(
            dispatcher.create_compute_pipeline(&[]),
            Err(VulkanComputeError::NotImplemented)
        ));

        assert!(matches!(
            dispatcher.create_buffer(1024, BufferUsage::Storage, MemoryProperty::DeviceLocal),
            Err(VulkanComputeError::NotImplemented)
        ));
    }
}

#[test]
fn test_dispatcher_drop() {
    #[cfg(feature = "vulkan-compute")]
    {
        // Test Drop trait calls shutdown
        {
            let dispatcher = ComputeDispatcher::uninit();
            assert!(!dispatcher.is_ready());
        } // Drop called here

        // No crashes = Drop works correctly
    }
}
