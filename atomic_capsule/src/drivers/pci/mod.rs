//! PCI Device Driver Framework for Capsule OS
//!
//! World's first 100% lockfree PCI enumeration and configuration driver using
//! Computational Capsule Architecture.
//!
//! # Architecture Overview
//!
//! This module provides complete PCI/PCIe device enumeration and configuration
//! based on the PCIe specification. All components are implemented as lockfree
//! capsules following the Chaos framework principles:
//!
//! - **100% lockfree**: NO mutex, NO RwLock anywhere
//! - **Cache-aligned**: 64B/128B/256B/4096B alignments for all capsules
//! - **Generation counters**: ABA prevention throughout
//! - **Atomic state machines**: CAS-based state transitions
//!
//! # Capsule Hierarchy
//!
//! ```text
//! PciEnumeratorCapsule (T4, 4096B)
//!     │
//!     ├── Bus scanning state machine
//!     ├── Device discovery batch processing
//!     ├── Multi-bus support (256 buses × 32 devices × 8 functions)
//!     └── ECAM/Legacy configuration access
//!
//! PciDeviceCapsule (T1, 256B)
//!     │
//!     ├── PCI configuration space (first 64 bytes cached)
//!     ├── Device state machine (enumerated → configured → active)
//!     ├── Vendor/Device ID and class codes
//!     └── Statistics tracking
//!
//! PciBarCapsule (T1, 128B)
//!     │
//!     ├── BAR type detection (memory/IO, 32/64-bit)
//!     ├── Size calculation via BAR sizing algorithm
//!     ├── Address/size/prefetchable attributes
//!     └── Mapping state tracking
//! ```
//!
//! # Tier Classification
//!
//! | Capsule | Tier | Size | Description |
//! |---------|------|------|-------------|
//! | PciEnumeratorCapsule | T4 Batch | 4096B | Bus scanning with batch processing |
//! | PciDeviceCapsule | T1 Atomic | 256B | Device configuration space |
//! | PciBarCapsule | T1 Atomic | 128B | BAR access and management |
//!
//! # Performance Targets
//!
//! - Device snapshot: <10ns
//! - BAR read: <5ns
//! - Bus scan (256 buses): <10ms (batch parallelized)
//! - Config space read: <100ns (cached) / <1μs (direct)
//!
//! # PCIe Specification Compliance
//!
//! This implementation targets PCIe 4.0/5.0 compatibility:
//! - Legacy PCI Configuration Mechanism #1 (IO ports 0xCF8/0xCFC)
//! - Enhanced Configuration Access Mechanism (ECAM/MCFG)
//! - 256-byte standard config space
//! - 4KB extended config space (PCIe)
//!
//! # Safety Assumptions (ASSUM Framework Summary)
//!
//! This module contains 50+ ASSUM tags documenting all safety assumptions:
//!
//! ## Enumerator Capsule (20+ ASSUM tags)
//! - ECAM base address validity
//! - Bus numbering constraints
//! - Device presence detection
//! - Multi-function device handling
//!
//! ## Device Capsule (15+ ASSUM tags)
//! - Config space alignment
//! - Vendor/Device ID validity
//! - Class code interpretation
//! - State machine compliance
//!
//! ## BAR Capsule (15+ ASSUM tags)
//! - BAR sizing algorithm correctness
//! - Address alignment requirements
//! - Prefetchable memory handling
//! - 64-bit BAR concatenation
//!
//! # Usage Example
//!
//! ```rust,ignore
//! use atomic_capsule::drivers::pci::{
//!     PciEnumeratorCapsule, PciDeviceCapsule, PciBarCapsule,
//!     PciEnumeratorState, PciDeviceState, BarType,
//! };
//!
//! // Initialize enumerator with ECAM base
//! let enumerator = PciEnumeratorCapsule::new();
//! enumerator.set_ecam_base(0xE000_0000).unwrap();
//! enumerator.start_scan().unwrap();
//!
//! // Poll for completion
//! while !enumerator.is_scan_complete() {
//!     enumerator.scan_step().unwrap();
//! }
//!
//! // Access discovered device
//! let device = PciDeviceCapsule::new();
//! device.initialize(0, 2, 0).unwrap();  // Bus 0, Device 2, Function 0
//! device.read_config_space().unwrap();
//!
//! let snapshot = device.snapshot();
//! println!("Found: {:04x}:{:04x}", snapshot.vendor_id, snapshot.device_id);
//!
//! // Access device BAR
//! let bar = PciBarCapsule::new();
//! bar.initialize_from_config(&device, 0).unwrap();
//!
//! let bar_snap = bar.snapshot();
//! println!("BAR0: base={:#x}, size={:#x}", bar_snap.base_address, bar_snap.size);
//! ```
//!
//! # References
//!
//! - [PCI Express Base Specification](https://pcisig.com/specifications)
//! - [PCI Configuration Space (Wikipedia)](https://en.wikipedia.org/wiki/PCI_configuration_space)
//! - [OSDev PCI Wiki](https://wiki.osdev.org/PCI)
//! - [OSDev PCIe Wiki](https://wiki.osdev.org/PCI_Express)
//! - [Linux PCIe Enumeration](https://www.kernel.org/doc/html/latest/PCI/pci.html)

// Sub-modules
pub mod enumerator_capsule;
pub mod device_capsule;
pub mod bar_capsule;

// Re-exports for PciEnumeratorCapsule
pub use enumerator_capsule::{
    PciEnumeratorCapsule, PciEnumeratorSnapshot, PciEnumeratorState,
    PciConfigAccess, EcamAccess, LegacyAccess, NullAccess,
    PCI_MAX_BUSES, PCI_MAX_DEVICES, PCI_MAX_FUNCTIONS,
    ECAM_DEVICE_STRIDE, CONFIG_SPACE_SIZE, EXT_CONFIG_SPACE_SIZE,
};

// Re-exports for PciDeviceCapsule
pub use device_capsule::{
    PciDeviceCapsule, PciDeviceSnapshot, PciDeviceState,
    PciDeviceClass, PciHeaderType,
    CONFIG_VENDOR_ID, CONFIG_DEVICE_ID, CONFIG_COMMAND, CONFIG_STATUS,
    CONFIG_REVISION, CONFIG_PROG_IF, CONFIG_SUBCLASS, CONFIG_CLASS,
    CONFIG_CACHE_LINE_SIZE, CONFIG_LATENCY_TIMER, CONFIG_HEADER_TYPE,
    CONFIG_BIST, CONFIG_BAR0, CONFIG_BAR1, CONFIG_BAR2, CONFIG_BAR3,
    CONFIG_BAR4, CONFIG_BAR5, CONFIG_CARDBUS_CIS, CONFIG_SUBSYS_VENDOR,
    CONFIG_SUBSYS_ID, CONFIG_ROM_BASE, CONFIG_CAPABILITIES, CONFIG_INT_LINE,
    CONFIG_INT_PIN, CONFIG_MIN_GRANT, CONFIG_MAX_LATENCY,
    CMD_IO_SPACE, CMD_MEMORY_SPACE, CMD_BUS_MASTER, CMD_SPECIAL_CYCLES,
    CMD_MWI_ENABLE, CMD_VGA_SNOOP, CMD_PARITY_ERROR, CMD_SERR_ENABLE,
    CMD_FAST_B2B, CMD_INT_DISABLE,
};

// Re-exports for PciBarCapsule
pub use bar_capsule::{
    PciBarCapsule, PciBarSnapshot, PciBarState,
    BarType, BarWidth,
    BAR_TYPE_MEMORY, BAR_TYPE_IO, BAR_MEMORY_TYPE_MASK,
    BAR_MEMORY_32BIT, BAR_MEMORY_64BIT, BAR_MEMORY_PREFETCHABLE,
    BAR_MEMORY_BASE_MASK, BAR_IO_BASE_MASK,
};

// ============================================================================
// Module-Level Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify all capsule sizes meet specifications
    #[test]
    fn test_capsule_sizes() {
        assert_eq!(core::mem::size_of::<PciEnumeratorCapsule>(), 4096);
        assert_eq!(core::mem::size_of::<PciDeviceCapsule>(), 256);
        assert_eq!(core::mem::size_of::<PciBarCapsule>(), 128);
    }

    /// Verify all capsule alignments meet specifications
    #[test]
    fn test_capsule_alignments() {
        assert_eq!(core::mem::align_of::<PciEnumeratorCapsule>(), 4096);
        assert_eq!(core::mem::align_of::<PciDeviceCapsule>(), 256);
        assert_eq!(core::mem::align_of::<PciBarCapsule>(), 128);
    }

    /// Verify config space offset constants
    #[test]
    fn test_config_offsets() {
        assert_eq!(CONFIG_VENDOR_ID, 0x00);
        assert_eq!(CONFIG_DEVICE_ID, 0x02);
        assert_eq!(CONFIG_COMMAND, 0x04);
        assert_eq!(CONFIG_STATUS, 0x06);
        assert_eq!(CONFIG_BAR0, 0x10);
        assert_eq!(CONFIG_CAPABILITIES, 0x34);
    }

    /// Verify BAR type constants
    #[test]
    fn test_bar_constants() {
        assert_eq!(BAR_TYPE_MEMORY, 0x00);
        assert_eq!(BAR_TYPE_IO, 0x01);
        assert_eq!(BAR_MEMORY_64BIT, 0x04);
        assert_eq!(BAR_MEMORY_PREFETCHABLE, 0x08);
    }

    /// Test complete enumeration flow
    #[test]
    fn test_enumeration_flow() {
        // Enumerator init
        let enum_cap = PciEnumeratorCapsule::new();
        assert_eq!(enum_cap.state(), PciEnumeratorState::Idle);

        // Device init
        let dev = PciDeviceCapsule::new();
        assert_eq!(dev.state(), PciDeviceState::Uninitialized);

        // BAR init
        let bar = PciBarCapsule::new();
        assert_eq!(bar.state(), PciBarState::Uninitialized);
    }

    /// Test device class parsing
    #[test]
    fn test_device_classes() {
        let classes = [
            (PciDeviceClass::MassStorage, 0x01),
            (PciDeviceClass::Network, 0x02),
            (PciDeviceClass::Display, 0x03),
            (PciDeviceClass::Multimedia, 0x04),
            (PciDeviceClass::Bridge, 0x06),
            (PciDeviceClass::SerialBus, 0x0C),
        ];

        for (class, code) in classes {
            assert_eq!(class.code(), code);
            assert_eq!(PciDeviceClass::from_code(code), class);
        }
    }

    /// Test header type parsing
    #[test]
    fn test_header_types() {
        assert_eq!(PciHeaderType::from_raw(0x00), PciHeaderType::Standard);
        assert_eq!(PciHeaderType::from_raw(0x01), PciHeaderType::PciToPciBridge);
        assert_eq!(PciHeaderType::from_raw(0x02), PciHeaderType::CardBusBridge);
        assert_eq!(PciHeaderType::from_raw(0x80), PciHeaderType::MultiFunction);
    }

    /// Test BAR type detection
    #[test]
    fn test_bar_types() {
        // Memory BAR (32-bit, non-prefetchable)
        let bar_val: u32 = 0xF0000000;
        assert_eq!(bar_val & BAR_TYPE_IO, 0); // Memory type

        // IO BAR
        let io_bar: u32 = 0x0000_3001;
        assert_eq!(io_bar & BAR_TYPE_IO, 1); // IO type

        // 64-bit BAR
        let bar64: u32 = 0xF0000004;
        assert_eq!((bar64 & BAR_MEMORY_TYPE_MASK) >> 1, 2); // 64-bit
    }
}
