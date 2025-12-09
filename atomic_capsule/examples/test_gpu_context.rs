use atomic_capsule::gui::render::{GpuBackend, GpuContextCapsule, GpuState};

fn main() {
    println!("Testing GpuContextCapsule...");

    let mut context = GpuContextCapsule::new();
    assert_eq!(context.state(), GpuState::Uninitialized);
    assert_eq!(context.backend(), GpuBackend::None);
    println!("✓ Creation");

    context.set_state(GpuState::Initializing);
    context.set_backend(GpuBackend::Vulkan);
    context.set_surface_size(1920, 1080);
    println!("✓ Initialization");

    context.set_state(GpuState::Ready);
    assert!(context.is_ready());
    println!("✓ Ready state");

    let frame1 = context.increment_frame();
    assert_eq!(frame1, 1);
    let frame2 = context.increment_frame();
    assert_eq!(frame2, 2);
    println!("✓ Frame counting");

    context.set_device_handle(0x1234_5678_9ABC_DEF0);
    context.set_queue_handle(0xFEDC_BA98_7654_3210);
    context.set_surface_handle(0xAAAA_BBBB_CCCC_DDDD);
    assert_eq!(context.device_handle(), 0x1234_5678_9ABC_DEF0);
    assert_eq!(context.queue_handle(), 0xFEDC_BA98_7654_3210);
    assert_eq!(context.surface_handle(), 0xAAAA_BBBB_CCCC_DDDD);
    println!("✓ Handle management");

    assert_eq!(core::mem::size_of::<GpuContextCapsule>(), 128);
    assert_eq!(core::mem::align_of::<GpuContextCapsule>(), 128);
    println!("✓ Size and alignment (128B)");

    println!("\nAll tests passed! ✓");
}
