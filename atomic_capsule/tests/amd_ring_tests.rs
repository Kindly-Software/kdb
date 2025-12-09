//! AMD CP Ring Buffer Integration Tests
//!
//! T28 Compliant test suite for AmdCpRingCapsule
//! Tests the PM4 packet ring buffer for AMD GPU command submission.

#![cfg(all(feature = "kgpu-driver", any(feature = "gpu-intel", feature = "gpu-rocm", feature = "gpu-cuda", feature = "gpu-all")))]

use atomic_capsule::gpu::kgpu_driver::{
    AmdCpRingCapsule, AmdCpRingSnapshot, AmdQueueType, CpRingState,
    Pm4Header, Pm4Opcode, Pm4PacketType, KgpuDriverError,
};
use core::mem;

// ============================================================================
// Tier 1: Unit Tests (Q1-Q7) - Struct Layout
// ============================================================================

#[test]
fn test_capsule_size() {
    // T28 Q1: Verify exact size is 256 bytes
    assert_eq!(mem::size_of::<AmdCpRingCapsule>(), 256);
}

#[test]
fn test_capsule_alignment() {
    // T28 Q2: Verify alignment is 256 bytes (4 cache lines)
    assert_eq!(mem::align_of::<AmdCpRingCapsule>(), 256);
}

#[test]
fn test_new_capsule_state() {
    // T28 Q3: Verify initial state is Uninitialized with generation 0
    let capsule = AmdCpRingCapsule::new();
    assert_eq!(capsule.state(), CpRingState::Uninitialized);
    assert_eq!(capsule.generation(), 0);
    assert_eq!(capsule.rptr(), 0);
    assert_eq!(capsule.wptr(), 0);
    assert_eq!(capsule.ring_base(), 0);
    assert_eq!(capsule.ring_size(), 0);
}

#[test]
fn test_default_impl() {
    // T28 Q4: Verify Default trait implementation
    let capsule: AmdCpRingCapsule = Default::default();
    assert_eq!(capsule.state(), CpRingState::Uninitialized);
}

#[test]
fn test_snapshot_size() {
    // T28 Q5: Verify snapshot is reasonably sized
    assert!(mem::size_of::<AmdCpRingSnapshot>() <= 128);
}

#[test]
fn test_pm4_header_size() {
    // T28 Q6: Verify PM4 header is exactly 4 bytes
    assert_eq!(mem::size_of::<Pm4Header>(), 4);
}

#[test]
fn test_queue_type_size() {
    // T28 Q7: Verify queue type enum is 1 byte
    assert_eq!(mem::size_of::<AmdQueueType>(), 1);
}

// ============================================================================
// Tier 2: PM4 Packet Tests (Q8-Q14)
// ============================================================================

#[test]
fn test_pm4_header_type3() {
    // T28 Q8: Verify Type 3 header encoding
    let header = Pm4Header::type3(Pm4Opcode::Nop, 4);
    assert_eq!(header.packet_type(), 3);
    assert_eq!(header.opcode(), Pm4Opcode::Nop as u8);
    assert_eq!(header.count(), 4);
    assert_eq!(header.total_dwords(), 5); // header + 4 payload
}

#[test]
fn test_pm4_header_type2_nop() {
    // T28 Q9: Verify Type 2 NOP encoding
    let header = Pm4Header::type2_nop();
    assert_eq!(header.packet_type(), 2);
    assert_eq!(header.total_dwords(), 1);
}

#[test]
fn test_pm4_header_nop_alias() {
    // T28 Q10: Verify NOP convenience function
    let header = Pm4Header::nop(8);
    assert_eq!(header.packet_type(), 3);
    assert_eq!(header.opcode(), Pm4Opcode::Nop as u8);
    assert_eq!(header.count(), 8);
}

#[test]
fn test_pm4_header_with_shader() {
    // T28 Q11: Verify shader type bit
    let gfx = Pm4Header::type3_with_shader(Pm4Opcode::DispatchDirect, 3, false);
    let compute = Pm4Header::type3_with_shader(Pm4Opcode::DispatchDirect, 3, true);

    assert!(!gfx.is_compute());
    assert!(compute.is_compute());
}

#[test]
fn test_pm4_opcode_names() {
    // T28 Q12: Verify opcode names
    assert_eq!(Pm4Opcode::Nop.name(), "NOP");
    assert_eq!(Pm4Opcode::DispatchDirect.name(), "DISPATCH_DIRECT");
    assert_eq!(Pm4Opcode::ReleaseMem.name(), "RELEASE_MEM");
    assert_eq!(Pm4Opcode::IndirectBuffer.name(), "INDIRECT_BUFFER");
}

#[test]
fn test_pm4_header_count_edge_cases() {
    // T28 Q13: Verify count handling at boundaries
    let zero = Pm4Header::type3(Pm4Opcode::Nop, 0);
    assert_eq!(zero.count(), 1); // count-1 stored, so 0 becomes 1

    let max = Pm4Header::type3(Pm4Opcode::Nop, 16384);
    assert_eq!(max.count(), 16384);
}

#[test]
fn test_pm4_header_display() {
    // T28 Q14: Verify Display implementation
    let header = Pm4Header::type3(Pm4Opcode::WriteData, 5);
    let display = format!("{}", header);
    assert!(display.contains("PM4"));
    assert!(display.contains("Type3"));
}

// ============================================================================
// Tier 3: State Transitions (Q15-Q21)
// ============================================================================

#[test]
fn test_initialize_success() {
    // T28 Q15: Verify Uninitialized -> Ready transition
    let capsule = AmdCpRingCapsule::new();

    let result = capsule.initialize(
        0x1000_0000,        // ring_base (256-byte aligned)
        256 * 1024,         // ring_size (256KB)
        0x2000_0000,        // fence_gpu_addr
        0x1000,             // doorbell_offset
        0,                  // queue_id
        AmdQueueType::Gfx,  // queue_type
        0,                  // pipe_id
        0,                  // vmid
    );

    assert!(result.is_ok());
    assert_eq!(capsule.state(), CpRingState::Ready);
    assert_eq!(capsule.generation(), 1);
    assert_eq!(capsule.ring_base(), 0x1000_0000);
    assert_eq!(capsule.ring_size(), 256 * 1024);
}

#[test]
fn test_initialize_invalid_alignment() {
    // T28 Q16: Verify alignment check
    let capsule = AmdCpRingCapsule::new();

    let result = capsule.initialize(
        0x1000_0001,        // NOT 256-byte aligned
        256 * 1024,
        0x2000_0000,
        0x1000,
        0,
        AmdQueueType::Gfx,
        0,
        0,
    );

    assert_eq!(result, Err(KgpuDriverError::InvalidAlignment));
}

#[test]
fn test_initialize_invalid_size() {
    // T28 Q17: Verify size validation
    let capsule = AmdCpRingCapsule::new();

    // Too small
    let result = capsule.initialize(
        0x1000_0000,
        1024,  // Less than 4KB
        0x2000_0000,
        0x1000,
        0,
        AmdQueueType::Gfx,
        0,
        0,
    );
    assert_eq!(result, Err(KgpuDriverError::InvalidSize));

    // Not power of 2
    let capsule2 = AmdCpRingCapsule::new();
    let result = capsule2.initialize(
        0x1000_0000,
        300 * 1024,  // Not power of 2
        0x2000_0000,
        0x1000,
        0,
        AmdQueueType::Gfx,
        0,
        0,
    );
    assert_eq!(result, Err(KgpuDriverError::InvalidSize));
}

#[test]
fn test_initialize_already_initialized() {
    // T28 Q18: Verify double initialization fails
    let capsule = AmdCpRingCapsule::new();
    capsule.initialize(
        0x1000_0000,
        256 * 1024,
        0x2000_0000,
        0x1000,
        0,
        AmdQueueType::Gfx,
        0,
        0,
    ).unwrap();

    let result = capsule.initialize(
        0x3000_0000,
        512 * 1024,
        0x4000_0000,
        0x2000,
        1,
        AmdQueueType::Compute,
        1,
        1,
    );
    assert_eq!(result, Err(KgpuDriverError::InvalidState));
}

#[test]
fn test_ring_state_predicates() {
    // T28 Q19: Verify state predicates
    assert!(!CpRingState::Uninitialized.is_operational());
    assert!(CpRingState::Ready.is_operational());
    assert!(CpRingState::Active.is_operational());
    assert!(!CpRingState::Error.is_operational());
    assert!(!CpRingState::Suspended.is_operational());
}

#[test]
fn test_state_names() {
    // T28 Q20: Verify state names
    assert_eq!(CpRingState::Uninitialized.name(), "Uninitialized");
    assert_eq!(CpRingState::Ready.name(), "Ready");
    assert_eq!(CpRingState::Active.name(), "Active");
    assert_eq!(CpRingState::Error.name(), "Error");
}

#[test]
fn test_queue_type_defaults() {
    // T28 Q21: Verify queue type default ring sizes
    assert_eq!(AmdQueueType::Gfx.default_ring_size(), 1024 * 1024);
    assert_eq!(AmdQueueType::Compute.default_ring_size(), 256 * 1024);
    assert_eq!(AmdQueueType::Dma.default_ring_size(), 256 * 1024);
}

// ============================================================================
// Tier 4: Ring Operations (Q22-Q28)
// ============================================================================

fn create_initialized_ring() -> AmdCpRingCapsule {
    let capsule = AmdCpRingCapsule::new();
    capsule.initialize(
        0x1000_0000,
        64 * 1024,  // 64KB = 16K DWORDs
        0x2000_0000,
        0x1000,
        0,
        AmdQueueType::Gfx,
        0,
        0,
    ).unwrap();
    capsule
}

#[test]
fn test_available_space_empty_ring() {
    // T28 Q22: Verify available space calculation for empty ring
    let ring = create_initialized_ring();
    let ring_size_dwords = ring.ring_size_dwords();

    // Empty ring: RPTR = WPTR = 0
    // Available = ring_size - 1 (reserve 1 to distinguish full/empty)
    assert_eq!(ring.available_space(), ring_size_dwords - 1);
}

#[test]
fn test_reserve_success() {
    // T28 Q23: Verify successful space reservation
    let ring = create_initialized_ring();

    let result = ring.reserve(100);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0); // First offset is 0

    // WPTR should advance
    assert_eq!(ring.wptr(), 100);
}

#[test]
fn test_reserve_updates_wptr() {
    // T28 Q24: Verify multiple reserves advance WPTR correctly
    let ring = create_initialized_ring();

    ring.reserve(100).unwrap();
    assert_eq!(ring.wptr(), 100);

    ring.reserve(50).unwrap();
    assert_eq!(ring.wptr(), 150);

    ring.reserve(200).unwrap();
    assert_eq!(ring.wptr(), 350);
}

#[test]
fn test_reserve_fails_not_initialized() {
    // T28 Q25: Verify reserve fails on uninitialized ring
    let ring = AmdCpRingCapsule::new();
    let result = ring.reserve(100);
    assert_eq!(result, Err(KgpuDriverError::InvalidState));
}

#[test]
fn test_submit_increments_fence() {
    // T28 Q26: Verify submit increments fence value
    let ring = create_initialized_ring();

    let offset = ring.reserve(100).unwrap();
    let fence1 = ring.submit(offset + 100).unwrap();
    assert_eq!(fence1, 1);

    let offset = ring.reserve(50).unwrap();
    let fence2 = ring.submit(offset + 50).unwrap();
    assert_eq!(fence2, 2);

    assert_eq!(ring.submit_count(), 2);
}

#[test]
fn test_update_rptr() {
    // T28 Q27: Verify RPTR update
    let ring = create_initialized_ring();

    ring.reserve(100).unwrap();
    ring.submit(100).unwrap();

    ring.update_rptr(100);
    assert_eq!(ring.rptr(), 100);

    // Available space should increase after RPTR update
    ring.reserve(100).unwrap();
    ring.update_rptr(200);
    assert_eq!(ring.rptr(), 200);
}

#[test]
fn test_rptr_wptr_wraparound() {
    // T28 Q28: Verify RPTR/WPTR wraparound
    let ring = create_initialized_ring();
    let ring_size_dwords = ring.ring_size_dwords();

    // Fill most of the ring
    let reserve_size = ring_size_dwords - 100;
    ring.reserve(reserve_size).unwrap();

    // Simulate GPU consuming commands
    ring.update_rptr(reserve_size);

    // Reserve more to trigger wraparound
    let offset = ring.reserve(200).unwrap();
    assert_eq!(offset, reserve_size);

    // WPTR should wrap around
    let expected_wptr = (reserve_size + 200) & (ring_size_dwords - 1);
    assert_eq!(ring.wptr(), expected_wptr);
}

// ============================================================================
// Tier 5: Determinism Tests (Q29-Q35)
// ============================================================================

#[test]
fn test_generation_increments_on_state_change() {
    // T28 Q29: Verify generation increments on state transitions
    let ring = AmdCpRingCapsule::new();
    assert_eq!(ring.generation(), 0);

    ring.initialize(
        0x1000_0000,
        64 * 1024,
        0x2000_0000,
        0x1000,
        0,
        AmdQueueType::Gfx,
        0,
        0,
    ).unwrap();
    assert_eq!(ring.generation(), 1);
    assert_eq!(ring.state(), CpRingState::Ready);

    ring.reserve(10).unwrap();
    ring.submit(10).unwrap();
    // Submit should transition Ready -> Active
    assert_eq!(ring.state(), CpRingState::Active);
    assert_eq!(ring.generation(), 2);

    ring.mark_idle().unwrap();
    assert_eq!(ring.state(), CpRingState::Ready);
    assert_eq!(ring.generation(), 3);
}

#[test]
fn test_snapshot_captures_all_state() {
    // T28 Q30: Verify snapshot captures all fields
    let ring = create_initialized_ring();
    ring.reserve(100).unwrap();
    ring.submit(100).unwrap();

    let snap = ring.snapshot();
    assert_eq!(snap.state, CpRingState::Active);
    assert_eq!(snap.generation, 2);
    assert_eq!(snap.wptr, 100);
    assert_eq!(snap.ring_base, 0x1000_0000);
    assert_eq!(snap.ring_size, 64 * 1024);
    assert_eq!(snap.fence_value, 1);
    assert_eq!(snap.submit_count, 1);
    assert_eq!(snap.queue_type, AmdQueueType::Gfx);
}

#[test]
fn test_snapshot_utilization() {
    // T28 Q31: Verify snapshot utilization calculation
    let ring = create_initialized_ring();
    let snap1 = ring.snapshot();
    assert_eq!(snap1.utilization_percent(), 0);

    ring.reserve(8192).unwrap(); // 50% of 16K DWORDs
    let snap2 = ring.snapshot();
    assert_eq!(snap2.utilization_percent(), 50);
}

#[test]
fn test_error_state() {
    // T28 Q32: Verify error state transition
    let ring = create_initialized_ring();

    ring.mark_error().unwrap();
    assert_eq!(ring.state(), CpRingState::Error);
    assert_eq!(ring.error_count(), 1);

    // Should not be operational
    assert!(!ring.state().is_operational());
}

#[test]
fn test_emit_nop_calculation() {
    // T28 Q33: Verify NOP emission calculation
    let ring = create_initialized_ring();

    let (header, next_offset) = ring.emit_nop(0, 8);
    let pm4 = Pm4Header { value: header };
    assert_eq!(pm4.packet_type(), 3);
    assert_eq!(pm4.opcode(), Pm4Opcode::Nop as u8);
    assert_eq!(next_offset, 8);
}

#[test]
fn test_emit_release_mem() {
    // T28 Q34: Verify RELEASE_MEM packet calculation
    let ring = create_initialized_ring();

    let (packet, next_offset): ([u32; 7], u32) = ring.emit_release_mem(0, 0x2000_0000, 12345);
    assert_eq!(packet.len(), 7);
    assert_eq!(next_offset, 7);

    let header = Pm4Header { value: packet[0] };
    assert_eq!(header.packet_type(), 3);
    assert_eq!(header.opcode(), Pm4Opcode::ReleaseMem as u8);
}

#[test]
fn test_send_sync_traits() {
    // T28 Q35: Verify Send + Sync implementation
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<AmdCpRingCapsule>();
    assert_send_sync::<AmdCpRingSnapshot>();
}

#[test]
fn test_debug_impl() {
    // Bonus: Verify Debug implementation
    let ring = create_initialized_ring();
    let debug_str = format!("{:?}", ring);
    assert!(debug_str.contains("AmdCpRingCapsule"));
    assert!(debug_str.contains("Ready"));
}
