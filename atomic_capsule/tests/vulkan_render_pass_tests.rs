// T28 Tests for VulkanRenderPassCapsule
// Tier 1 (Q1-Q7): Unit tests - Basic operations
// Tier 2 (Q8-Q14): Property tests - Layout transitions, dependency chains

use atomic_capsule::gpu::graphics::{
    VulkanRenderPassCapsule,
    LoadOp,
    StoreOp,
    ImageLayout,
    PipelineStage,
    AccessFlags,
    AttachmentDesc,
    AttachmentRef,
    SubpassDesc,
    SubpassDependency,
    ClearValue,
    DepthStencilClearValue,
    RenderPassBuilder,
};

// ============================================================================
// Q1-Q7: Unit Tests
// ============================================================================

#[test]
fn q1_capsule_initialization() {
    let capsule = VulkanRenderPassCapsule::new();

    assert_eq!(capsule.get_handle(), 0);
    assert_eq!(capsule.get_framebuffer(), 0);
    assert_eq!(capsule.attachment_count, 0);
    assert_eq!(capsule.subpass_count, 0);
    assert_eq!(capsule.dependency_count, 0);

    let (begins, ends, advances) = capsule.get_stats();
    assert_eq!(begins, 0);
    assert_eq!(ends, 0);
    assert_eq!(advances, 0);

    assert!(!capsule.is_dynamic_rendering());
}

#[test]
fn q2_handle_management() {
    let capsule = VulkanRenderPassCapsule::new();

    // Test VkRenderPass handle
    let test_handle = 0xDEADBEEF_12345678u64;
    capsule.set_handle(test_handle);
    assert_eq!(capsule.get_handle(), test_handle);

    // Test VkFramebuffer handle
    let test_fb = 0xCAFEBABE_87654321u64;
    capsule.set_framebuffer(test_fb);
    assert_eq!(capsule.get_framebuffer(), test_fb);

    // Handles should be independent
    assert_eq!(capsule.get_handle(), test_handle);
}

#[test]
fn q3_render_area_packing() {
    let capsule = VulkanRenderPassCapsule::new();

    // Test various render area configurations
    let test_cases = [
        (0, 0, 1920, 1080),
        (10, 20, 1280, 720),
        (100, 200, 3840, 2160),
        (u32::MAX >> 1, u32::MAX >> 1, u32::MAX >> 1, u32::MAX >> 1),
    ];

    for &(x, y, w, h) in &test_cases {
        capsule.set_render_area(x, y, w, h);
        let (rx, ry, rw, rh) = capsule.get_render_area();
        assert_eq!(rx, x, "X offset mismatch");
        assert_eq!(ry, y, "Y offset mismatch");
        assert_eq!(rw, w, "Width mismatch");
        assert_eq!(rh, h, "Height mismatch");
    }
}

#[test]
fn q4_begin_end_tracking() {
    let capsule = VulkanRenderPassCapsule::new();

    // Multiple begin/end cycles
    for i in 1..=10 {
        capsule.begin();
        let (begins, ends, _) = capsule.get_stats();
        assert_eq!(begins, i);
        assert_eq!(ends, i - 1);

        capsule.end();
        let (begins, ends, _) = capsule.get_stats();
        assert_eq!(begins, i);
        assert_eq!(ends, i);
    }
}

#[test]
fn q5_attachment_addition() {
    let mut capsule = VulkanRenderPassCapsule::new();

    // Add color attachment
    let color_desc = AttachmentDesc {
        format: 37,  // VK_FORMAT_R8G8B8A8_UNORM
        samples: 1,
        load_op: LoadOp::Clear,
        store_op: StoreOp::Store,
        stencil_load_op: LoadOp::DontCare,
        stencil_store_op: StoreOp::DontCare,
        initial_layout: ImageLayout::Undefined,
        final_layout: ImageLayout::ColorAttachment,
    };

    let idx = capsule.add_attachment(color_desc);
    assert_eq!(idx, Some(0));
    assert_eq!(capsule.attachment_count, 1);

    // Retrieve and verify
    let retrieved = capsule.get_attachment(0).unwrap();
    assert_eq!(retrieved.format, 37);
    assert_eq!(retrieved.load_op, LoadOp::Clear);
    assert_eq!(retrieved.store_op, StoreOp::Store);
    assert_eq!(retrieved.final_layout, ImageLayout::ColorAttachment);

    // Add depth attachment
    let depth_desc = AttachmentDesc {
        format: 124,  // VK_FORMAT_D32_SFLOAT
        samples: 1,
        load_op: LoadOp::Clear,
        store_op: StoreOp::DontCare,
        stencil_load_op: LoadOp::DontCare,
        stencil_store_op: StoreOp::DontCare,
        initial_layout: ImageLayout::Undefined,
        final_layout: ImageLayout::DepthStencilAttachment,
    };

    let idx = capsule.add_attachment(depth_desc);
    assert_eq!(idx, Some(1));
    assert_eq!(capsule.attachment_count, 2);
}

#[test]
fn q6_subpass_configuration() {
    let mut capsule = VulkanRenderPassCapsule::new();

    // Create subpass with color and depth attachments
    let mut subpass = SubpassDesc::default();
    subpass.color_attachments[0] = AttachmentRef {
        attachment: 0,
        layout: ImageLayout::ColorAttachment,
    };
    subpass.color_count = 1;
    subpass.depth_attachment = AttachmentRef {
        attachment: 1,
        layout: ImageLayout::DepthStencilAttachment,
    };

    let idx = capsule.add_subpass(subpass);
    assert_eq!(idx, Some(0));
    assert_eq!(capsule.subpass_count, 1);

    // Retrieve and verify
    let retrieved = capsule.get_subpass(0).unwrap();
    assert_eq!(retrieved.color_count, 1);
    assert_eq!(retrieved.color_attachments[0].attachment, 0);
    assert_eq!(retrieved.depth_attachment.attachment, 1);
}

#[test]
fn q7_dependency_setup() {
    let mut capsule = VulkanRenderPassCapsule::new();

    // External dependency (pre-renderpass)
    let ext_dep = SubpassDependency {
        src_subpass: 0xFFFFFFFF,  // VK_SUBPASS_EXTERNAL
        dst_subpass: 0,
        src_stage_mask: PipelineStage::ColorAttachmentOutput as u32,
        dst_stage_mask: PipelineStage::ColorAttachmentOutput as u32,
        src_access_mask: 0,
        dst_access_mask: AccessFlags::ColorAttachmentWrite as u32,
        dependency_flags: 0,
    };

    let idx = capsule.add_dependency(ext_dep);
    assert_eq!(idx, Some(0));
    assert_eq!(capsule.dependency_count, 1);

    // Internal dependency (subpass 0 -> 1)
    let int_dep = SubpassDependency {
        src_subpass: 0,
        dst_subpass: 1,
        src_stage_mask: PipelineStage::ColorAttachmentOutput as u32,
        dst_stage_mask: PipelineStage::FragmentShader as u32,
        src_access_mask: AccessFlags::ColorAttachmentWrite as u32,
        dst_access_mask: AccessFlags::InputAttachmentRead as u32,
        dependency_flags: 0x1,  // VK_DEPENDENCY_BY_REGION_BIT
    };

    let idx = capsule.add_dependency(int_dep);
    assert_eq!(idx, Some(1));
    assert_eq!(capsule.dependency_count, 2);
}

// ============================================================================
// Q8-Q14: Property Tests
// ============================================================================

#[test]
fn q8_attachment_limit_enforcement() {
    let mut capsule = VulkanRenderPassCapsule::new();

    // Add maximum attachments (8)
    for i in 0..8 {
        let desc = AttachmentDesc {
            format: 37 + i,
            ..Default::default()
        };
        let idx = capsule.add_attachment(desc);
        assert_eq!(idx, Some(i as usize));
    }

    // 9th attachment should fail
    let desc = AttachmentDesc::default();
    let idx = capsule.add_attachment(desc);
    assert_eq!(idx, None);
    assert_eq!(capsule.attachment_count, 8);
}

#[test]
fn q9_subpass_limit_enforcement() {
    let mut capsule = VulkanRenderPassCapsule::new();

    // Add maximum subpasses (4)
    for i in 0..4 {
        let mut subpass = SubpassDesc::default();
        subpass.color_count = i + 1;
        let idx = capsule.add_subpass(subpass);
        assert_eq!(idx, Some(i as usize));
    }

    // 5th subpass should fail
    let subpass = SubpassDesc::default();
    let idx = capsule.add_subpass(subpass);
    assert_eq!(idx, None);
    assert_eq!(capsule.subpass_count, 4);
}

#[test]
fn q10_dependency_limit_enforcement() {
    let mut capsule = VulkanRenderPassCapsule::new();

    // Add maximum dependencies (8)
    for i in 0..8 {
        let dep = SubpassDependency {
            src_subpass: i,
            dst_subpass: i + 1,
            ..Default::default()
        };
        let idx = capsule.add_dependency(dep);
        assert_eq!(idx, Some(i as usize));
    }

    // 9th dependency should fail
    let dep = SubpassDependency::default();
    let idx = capsule.add_dependency(dep);
    assert_eq!(idx, None);
    assert_eq!(capsule.dependency_count, 8);
}

#[test]
fn q11_layout_transition_sequence() {
    let mut capsule = VulkanRenderPassCapsule::new();

    // Test valid layout transition chain:
    // Undefined -> ColorAttachment -> ShaderReadOnly -> PresentSrc
    let layouts = [
        ImageLayout::Undefined,
        ImageLayout::ColorAttachment,
        ImageLayout::ShaderReadOnly,
        ImageLayout::PresentSrc,
    ];

    for i in 0..layouts.len() - 1 {
        let desc = AttachmentDesc {
            format: 37,
            samples: 1,
            load_op: LoadOp::Clear,
            store_op: StoreOp::Store,
            stencil_load_op: LoadOp::DontCare,
            stencil_store_op: StoreOp::DontCare,
            initial_layout: layouts[i],
            final_layout: layouts[i + 1],
        };
        capsule.add_attachment(desc);
    }

    // Verify all transitions stored
    for i in 0..layouts.len() - 1 {
        let att = capsule.get_attachment(i).unwrap();
        assert_eq!(att.initial_layout, layouts[i]);
        assert_eq!(att.final_layout, layouts[i + 1]);
    }
}

#[test]
fn q12_subpass_execution_order() {
    let mut capsule = VulkanRenderPassCapsule::new();

    // Setup 4 subpasses
    for _ in 0..4 {
        capsule.add_subpass(SubpassDesc::default());
    }

    capsule.begin();
    assert_eq!(capsule.current_subpass_index(), 0);

    // Advance through subpasses
    assert_eq!(capsule.next_subpass(), Some(1));
    assert_eq!(capsule.current_subpass_index(), 1);

    assert_eq!(capsule.next_subpass(), Some(2));
    assert_eq!(capsule.current_subpass_index(), 2);

    assert_eq!(capsule.next_subpass(), Some(3));
    assert_eq!(capsule.current_subpass_index(), 3);

    // No more subpasses
    assert_eq!(capsule.next_subpass(), None);

    let (_, _, advances) = capsule.get_stats();
    assert_eq!(advances, 4);
}

#[test]
fn q13_dependency_chain_validation() {
    let mut capsule = VulkanRenderPassCapsule::new();

    // Create valid dependency chain: External -> 0 -> 1 -> 2 -> External
    let deps = [
        SubpassDependency {
            src_subpass: 0xFFFFFFFF,  // External
            dst_subpass: 0,
            src_stage_mask: PipelineStage::TopOfPipe as u32,
            dst_stage_mask: PipelineStage::ColorAttachmentOutput as u32,
            src_access_mask: 0,
            dst_access_mask: AccessFlags::ColorAttachmentWrite as u32,
            dependency_flags: 0,
        },
        SubpassDependency {
            src_subpass: 0,
            dst_subpass: 1,
            src_stage_mask: PipelineStage::ColorAttachmentOutput as u32,
            dst_stage_mask: PipelineStage::FragmentShader as u32,
            src_access_mask: AccessFlags::ColorAttachmentWrite as u32,
            dst_access_mask: AccessFlags::InputAttachmentRead as u32,
            dependency_flags: 0x1,  // BY_REGION
        },
        SubpassDependency {
            src_subpass: 1,
            dst_subpass: 2,
            src_stage_mask: PipelineStage::ColorAttachmentOutput as u32,
            dst_stage_mask: PipelineStage::FragmentShader as u32,
            src_access_mask: AccessFlags::ColorAttachmentWrite as u32,
            dst_access_mask: AccessFlags::ShaderRead as u32,
            dependency_flags: 0x1,
        },
        SubpassDependency {
            src_subpass: 2,
            dst_subpass: 0xFFFFFFFF,  // External
            src_stage_mask: PipelineStage::ColorAttachmentOutput as u32,
            dst_stage_mask: PipelineStage::BottomOfPipe as u32,
            src_access_mask: AccessFlags::ColorAttachmentWrite as u32,
            dst_access_mask: 0,
            dependency_flags: 0,
        },
    ];

    for dep in &deps {
        capsule.add_dependency(*dep);
    }

    assert_eq!(capsule.dependency_count, 4);

    // Verify chain
    for i in 0..4 {
        let dep = capsule.get_dependency(i).unwrap();
        assert_eq!(dep.src_subpass, deps[i].src_subpass);
        assert_eq!(dep.dst_subpass, deps[i].dst_subpass);
    }
}

#[test]
fn q14_clear_value_storage() {
    let mut capsule = VulkanRenderPassCapsule::new();

    // Set clear colors for multiple attachments
    capsule.set_clear_color(0, 1.0, 0.0, 0.0, 1.0);  // Red
    capsule.set_clear_color(1, 0.0, 1.0, 0.0, 1.0);  // Green
    capsule.set_clear_color(2, 0.0, 0.0, 1.0, 1.0);  // Blue

    // Set clear depth/stencil
    capsule.set_clear_depth_stencil(3, 1.0, 0);
    capsule.set_clear_depth_stencil(4, 0.5, 255);

    // Verify colors
    unsafe {
        assert_eq!(capsule.clear_values[0].color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(capsule.clear_values[1].color, [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(capsule.clear_values[2].color, [0.0, 0.0, 1.0, 1.0]);

        assert_eq!(capsule.clear_values[3].depth_stencil.depth, 1.0);
        assert_eq!(capsule.clear_values[3].depth_stencil.stencil, 0);

        assert_eq!(capsule.clear_values[4].depth_stencil.depth, 0.5);
        assert_eq!(capsule.clear_values[4].depth_stencil.stencil, 255);
    }
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn integration_deferred_rendering_setup() {
    // Test complete deferred rendering configuration
    // Subpass 0: Geometry pass (write gbuffer)
    // Subpass 1: Lighting pass (read gbuffer as input attachments)

    let mut capsule = VulkanRenderPassCapsule::new();

    // Attachments: Albedo, Normal, Depth, Final Color
    let attachments = [
        AttachmentDesc {
            format: 37,  // RGBA8 for albedo
            samples: 1,
            load_op: LoadOp::Clear,
            store_op: StoreOp::DontCare,  // Transient
            stencil_load_op: LoadOp::DontCare,
            stencil_store_op: StoreOp::DontCare,
            initial_layout: ImageLayout::Undefined,
            final_layout: ImageLayout::ColorAttachment,
        },
        AttachmentDesc {
            format: 37,  // RGBA8 for normals
            samples: 1,
            load_op: LoadOp::Clear,
            store_op: StoreOp::DontCare,  // Transient
            stencil_load_op: LoadOp::DontCare,
            stencil_store_op: StoreOp::DontCare,
            initial_layout: ImageLayout::Undefined,
            final_layout: ImageLayout::ColorAttachment,
        },
        AttachmentDesc {
            format: 124,  // D32_SFLOAT
            samples: 1,
            load_op: LoadOp::Clear,
            store_op: StoreOp::DontCare,
            stencil_load_op: LoadOp::DontCare,
            stencil_store_op: StoreOp::DontCare,
            initial_layout: ImageLayout::Undefined,
            final_layout: ImageLayout::DepthStencilAttachment,
        },
        AttachmentDesc {
            format: 37,  // RGBA8 for final
            samples: 1,
            load_op: LoadOp::Clear,
            store_op: StoreOp::Store,  // Must store for present
            stencil_load_op: LoadOp::DontCare,
            stencil_store_op: StoreOp::DontCare,
            initial_layout: ImageLayout::Undefined,
            final_layout: ImageLayout::PresentSrc,
        },
    ];

    for att in &attachments {
        capsule.add_attachment(*att);
    }

    // Subpass 0: Geometry pass
    let mut geo_subpass = SubpassDesc::default();
    geo_subpass.color_attachments[0] = AttachmentRef {
        attachment: 0,
        layout: ImageLayout::ColorAttachment,
    };
    geo_subpass.color_attachments[1] = AttachmentRef {
        attachment: 1,
        layout: ImageLayout::ColorAttachment,
    };
    geo_subpass.color_count = 2;
    geo_subpass.depth_attachment = AttachmentRef {
        attachment: 2,
        layout: ImageLayout::DepthStencilAttachment,
    };
    capsule.add_subpass(geo_subpass);

    // Subpass 1: Lighting pass
    let mut light_subpass = SubpassDesc::default();
    light_subpass.color_attachments[0] = AttachmentRef {
        attachment: 3,
        layout: ImageLayout::ColorAttachment,
    };
    light_subpass.color_count = 1;
    light_subpass.input_attachments[0] = AttachmentRef {
        attachment: 0,  // Albedo input
        layout: ImageLayout::ShaderReadOnly,
    };
    light_subpass.input_attachments[1] = AttachmentRef {
        attachment: 1,  // Normal input
        layout: ImageLayout::ShaderReadOnly,
    };
    light_subpass.input_count = 2;
    capsule.add_subpass(light_subpass);

    // Dependencies
    let ext_dep = SubpassDependency {
        src_subpass: 0xFFFFFFFF,
        dst_subpass: 0,
        src_stage_mask: PipelineStage::ColorAttachmentOutput as u32,
        dst_stage_mask: PipelineStage::ColorAttachmentOutput as u32,
        src_access_mask: 0,
        dst_access_mask: AccessFlags::ColorAttachmentWrite as u32,
        dependency_flags: 0,
    };
    capsule.add_dependency(ext_dep);

    let subpass_dep = SubpassDependency {
        src_subpass: 0,
        dst_subpass: 1,
        src_stage_mask: PipelineStage::ColorAttachmentOutput as u32,
        dst_stage_mask: PipelineStage::FragmentShader as u32,
        src_access_mask: AccessFlags::ColorAttachmentWrite as u32,
        dst_access_mask: AccessFlags::InputAttachmentRead as u32,
        dependency_flags: 0x1,  // BY_REGION for tile GPU optimization
    };
    capsule.add_dependency(subpass_dep);

    // Verify setup
    assert_eq!(capsule.attachment_count, 4);
    assert_eq!(capsule.subpass_count, 2);
    assert_eq!(capsule.dependency_count, 2);
}

#[test]
fn integration_builder_pattern() {
    let capsule = RenderPassBuilder::new()
        .add_color_attachment(
            37,  // RGBA8
            LoadOp::Clear,
            StoreOp::Store,
            ImageLayout::Undefined,
            ImageLayout::PresentSrc,
        )
        .add_depth_stencil_attachment(
            124,  // D32
            LoadOp::Clear,
            StoreOp::DontCare,
            LoadOp::DontCare,
            StoreOp::DontCare,
            ImageLayout::Undefined,
            ImageLayout::DepthStencilAttachment,
        )
        .add_simple_subpass(0)
        .add_external_dependency()
        .build();

    assert_eq!(capsule.attachment_count, 2);
    assert_eq!(capsule.subpass_count, 1);
    assert_eq!(capsule.dependency_count, 1);

    let att0 = capsule.get_attachment(0).unwrap();
    assert_eq!(att0.format, 37);
    assert_eq!(att0.final_layout, ImageLayout::PresentSrc);
}

#[test]
fn integration_reset_reuse() {
    let mut capsule = VulkanRenderPassCapsule::new();

    // Configure and use
    capsule.set_handle(123);
    capsule.add_attachment(AttachmentDesc::default());
    capsule.begin();
    capsule.end();

    // Reset
    capsule.reset();

    // Verify clean state
    assert_eq!(capsule.get_handle(), 0);
    assert_eq!(capsule.attachment_count, 0);
    let (begins, ends, advances) = capsule.get_stats();
    assert_eq!(begins, 0);
    assert_eq!(ends, 0);
    assert_eq!(advances, 0);

    // Reuse
    capsule.set_handle(456);
    capsule.add_attachment(AttachmentDesc::default());
    assert_eq!(capsule.get_handle(), 456);
    assert_eq!(capsule.attachment_count, 1);
}

#[test]
fn integration_dynamic_rendering_mode() {
    let capsule = VulkanRenderPassCapsule::new();

    assert!(!capsule.is_dynamic_rendering());

    // Enable dynamic rendering (Vulkan 1.3+)
    capsule.enable_dynamic_rendering();
    assert!(capsule.is_dynamic_rendering());

    // Handle should be 0 in dynamic rendering mode (no VkRenderPass object)
    capsule.set_handle(0);
    assert_eq!(capsule.get_handle(), 0);
}
