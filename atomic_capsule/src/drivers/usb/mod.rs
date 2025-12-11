//! USB Driver Framework for Capsule OS
//!
//! World's first 100% lockfree USB driver stack using Computational Capsule Architecture.
//!
//! # Architecture Overview
//!
//! This module provides a complete USB driver framework based on the xHCI (eXtensible
//! Host Controller Interface) specification. All components are implemented as lockfree
//! capsules following the Chaos framework principles:
//!
//! - **100% lockfree**: NO mutex, NO RwLock anywhere
//! - **Cache-aligned**: 64B/128B/256B/512B/1024B alignments for all capsules
//! - **Generation counters**: ABA prevention throughout
//! - **Atomic state machines**: CAS-based state transitions
//!
//! # Capsule Hierarchy
//!
//! ```text
//! UsbControllerCapsule (T1, 512B)
//!     │
//!     ├── xHCI Host Controller State
//!     ├── MMIO register management
//!     ├── Ring buffer pointers (DCBAA, Command, Event)
//!     └── Port status tracking
//!
//! UsbDeviceCapsule (T1, 256B)
//!     │
//!     ├── USB device state machine (USB 2.0 spec compliant)
//!     ├── Device descriptors (VID/PID, class, etc.)
//!     ├── Configuration management
//!     └── Transfer statistics
//!
//! UsbTransferCapsule (T5, 1024B)
//!     │
//!     ├── Transfer ring management
//!     ├── TD (Transfer Descriptor) tracking
//!     ├── Cycle bit management
//!     └── Stream support (USB 3.x)
//! ```
//!
//! # Tier Classification
//!
//! | Capsule | Tier | Size | Description |
//! |---------|------|------|-------------|
//! | UsbControllerCapsule | T1 Atomic | 512B | Host controller state |
//! | UsbDeviceCapsule | T1 Atomic | 256B | USB device state |
//! | UsbTransferCapsule | T5 Streaming | 1024B | Transfer ring management |
//!
//! # Performance Targets
//!
//! - Controller snapshot: <10ns
//! - Device state transition: <50ns
//! - TRB enqueue: <50ns
//! - TD completion: <20ns
//!
//! # xHCI Specification Compliance
//!
//! This implementation targets xHCI 1.0-1.2 compatibility:
//! - USB 1.x (Low/Full Speed)
//! - USB 2.0 (High Speed)
//! - USB 3.0 (SuperSpeed 5Gbps)
//! - USB 3.1 (SuperSpeed+ 10Gbps)
//! - USB 3.2 (SuperSpeed+ 20Gbps with 2x2)
//!
//! # Safety Assumptions (ASSUM Framework Summary)
//!
//! This module contains 50+ ASSUM tags documenting all safety assumptions:
//!
//! ## Controller Capsule (15+ ASSUM tags)
//! - MMIO validity and alignment
//! - PCI configuration access
//! - BAR mapping requirements
//! - DMA buffer constraints
//!
//! ## Device Capsule (15+ ASSUM tags)
//! - Slot context validity
//! - Device descriptor format
//! - Enumeration sequence
//! - State machine compliance
//!
//! ## Transfer Capsule (20+ ASSUM tags)
//! - Ring buffer alignment
//! - TRB format compliance
//! - Cycle bit management
//! - Stream support requirements
//!
//! # Usage Example
//!
//! ```rust,ignore
//! use atomic_capsule::drivers::usb::{
//!     UsbControllerCapsule, UsbDeviceCapsule, UsbTransferCapsule,
//!     UsbControllerState, UsbDeviceState, TransferRingState,
//!     UsbDeviceSpeed, TransferType,
//! };
//!
//! // Initialize controller
//! let controller = UsbControllerCapsule::new();
//! controller.begin_detection().unwrap();
//! controller.set_mmio_base(0xFED0_0000).unwrap();
//! controller.configure_from_caps(32, 0x0110, 64, 8, 4, true, true, 0x2000, 0x1000).unwrap();
//! controller.set_dcbaap(dcbaap_address).unwrap();
//! controller.set_command_ring(cmd_ring_address).unwrap();
//! controller.set_event_ring(erstba, erdp).unwrap();
//! controller.start().unwrap();
//!
//! // Enumerate device
//! let device = UsbDeviceCapsule::new();
//! device.attach(1, UsbDeviceSpeed::SuperSpeed).unwrap();
//! device.reset(slot_id).unwrap();
//! device.set_descriptor(vid, pid, class, subclass, protocol, usb_ver, num_configs, max_pkt);
//! device.set_address(10).unwrap();
//! device.configure(1, 2).unwrap();
//!
//! // Setup transfer ring
//! let transfer = UsbTransferCapsule::new();
//! transfer.initialize(ring_base, 256, slot_id, 2, TransferType::Bulk, 512).unwrap();
//!
//! // Enqueue transfer
//! let trb_data = [0u8; 16]; // Build TRB data
//! transfer.enqueue(&trb_data, true, callback_data).unwrap();
//!
//! // Ring doorbell to notify controller
//! unsafe { controller.ring_doorbell(slot_id, 2, 0); }
//! ```
//!
//! # References
//!
//! - [xHCI Specification](https://www.intel.com/content/www/us/en/products/docs/io/universal-serial-bus/extensible-host-controler-interface-usb-xhci.html)
//! - [USB 2.0 Specification](https://www.usb.org/document-library/usb-20-specification)
//! - [USB 3.2 Specification](https://www.usb.org/document-library/usb-32-specification-released-september-22-2017-and-ecns)
//! - [OSDev xHCI Wiki](https://wiki.osdev.org/EXtensible_Host_Controller_Interface)

// Sub-modules
pub mod controller_capsule;
pub mod device_capsule;
pub mod transfer_capsule;

// Re-exports for UsbControllerCapsule
pub use controller_capsule::{
    UsbControllerCapsule, UsbControllerSnapshot, UsbControllerState,
    usb_controller,
    XHCI_PCI_CLASS, XHCI_PCI_SUBCLASS, XHCI_PCI_INTERFACE,
    MAX_DEVICE_SLOTS, MAX_ROOT_PORTS,
};

// Re-exports for UsbDeviceCapsule
pub use device_capsule::{
    UsbDeviceCapsule, UsbDeviceSnapshot, UsbDeviceState,
    UsbDeviceSpeed, UsbDeviceClass,
    MAX_INTERFACES, MAX_CONFIGURATIONS, MAX_ENDPOINTS_PER_INTERFACE,
};

// Re-exports for UsbTransferCapsule
pub use transfer_capsule::{
    UsbTransferCapsule, TransferRingSnapshot, TransferRingState,
    TransferType, TransferRequest,
    DEFAULT_RING_SIZE, TRB_SIZE, MAX_TDS_IN_FLIGHT, NO_STREAM,
    TRANSFER_FLAG_IN, TRANSFER_FLAG_OUT, TRANSFER_FLAG_IOC,
    TRANSFER_FLAG_ISP, TRANSFER_FLAG_NO_SNOOP, TRANSFER_FLAG_CHAIN,
    TRANSFER_FLAG_SHORT_OK,
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
        assert_eq!(core::mem::size_of::<UsbControllerCapsule>(), 512);
        assert_eq!(core::mem::size_of::<UsbDeviceCapsule>(), 256);
        assert_eq!(core::mem::size_of::<UsbTransferCapsule>(), 1024);
    }

    /// Verify all capsule alignments meet specifications
    #[test]
    fn test_capsule_alignments() {
        assert_eq!(core::mem::align_of::<UsbControllerCapsule>(), 512);
        assert_eq!(core::mem::align_of::<UsbDeviceCapsule>(), 256);
        assert_eq!(core::mem::align_of::<UsbTransferCapsule>(), 1024);
    }

    /// Verify PCI constants match xHCI specification
    #[test]
    fn test_pci_constants() {
        assert_eq!(XHCI_PCI_CLASS, 0x0C);     // Serial Bus Controller
        assert_eq!(XHCI_PCI_SUBCLASS, 0x03);  // USB Controller
        assert_eq!(XHCI_PCI_INTERFACE, 0x30); // xHCI
    }

    /// Verify transfer ring constants
    #[test]
    fn test_transfer_constants() {
        assert_eq!(DEFAULT_RING_SIZE, 256);
        assert_eq!(TRB_SIZE, 16);
        assert_eq!(MAX_TDS_IN_FLIGHT, 64);
    }

    /// Test complete enumeration flow
    #[test]
    fn test_enumeration_flow() {
        // Controller init
        let ctrl = UsbControllerCapsule::new();
        assert_eq!(ctrl.state(), UsbControllerState::Uninitialized);

        ctrl.begin_detection().unwrap();
        ctrl.set_mmio_base(0xFED0_0000).unwrap();
        ctrl.configure_from_caps(32, 0x0110, 64, 8, 4, true, true, 0x2000, 0x1000).unwrap();
        ctrl.set_dcbaap(0x1000_0000).unwrap();
        ctrl.set_command_ring(0x1001_0000).unwrap();
        ctrl.set_event_ring(0x1002_0000, 0x1002_0000).unwrap();
        ctrl.start().unwrap();
        assert_eq!(ctrl.state(), UsbControllerState::Running);

        // Device enumeration
        let dev = UsbDeviceCapsule::new();
        dev.attach(1, UsbDeviceSpeed::SuperSpeed).unwrap();
        dev.reset(1).unwrap();
        dev.set_descriptor(0x8086, 0x1234, 0x08, 0x06, 0x50, 0x0310, 1, 512);
        dev.set_address(10).unwrap();
        dev.configure(1, 1).unwrap();
        assert!(dev.snapshot().is_configured());

        // Transfer ring
        let xfer = UsbTransferCapsule::new();
        xfer.initialize(0x2000_0000, 256, 1, 2, TransferType::Bulk, 1024).unwrap();
        assert!(xfer.snapshot().is_ready());
    }

    /// Test device speeds
    #[test]
    fn test_device_speeds() {
        assert_eq!(UsbDeviceSpeed::LowSpeed.default_max_packet_ep0(), 8);
        assert_eq!(UsbDeviceSpeed::FullSpeed.default_max_packet_ep0(), 8);
        assert_eq!(UsbDeviceSpeed::HighSpeed.default_max_packet_ep0(), 64);
        assert_eq!(UsbDeviceSpeed::SuperSpeed.default_max_packet_ep0(), 512);
        assert_eq!(UsbDeviceSpeed::SuperSpeedPlus.default_max_packet_ep0(), 512);

        assert!(!UsbDeviceSpeed::HighSpeed.is_superspeed());
        assert!(UsbDeviceSpeed::SuperSpeed.is_superspeed());
        assert!(UsbDeviceSpeed::SuperSpeedPlus.is_superspeed());
    }

    /// Test transfer types
    #[test]
    fn test_transfer_types() {
        assert_eq!(TransferType::Control.code(), 0);
        assert_eq!(TransferType::Isochronous.code(), 1);
        assert_eq!(TransferType::Bulk.code(), 2);
        assert_eq!(TransferType::Interrupt.code(), 3);

        for code in 0..4 {
            let t = TransferType::from_code(code);
            assert_eq!(t.code(), code);
        }
    }

    /// Test device classes
    #[test]
    fn test_device_classes() {
        let classes = [
            (UsbDeviceClass::Audio, 0x01),
            (UsbDeviceClass::Cdc, 0x02),
            (UsbDeviceClass::Hid, 0x03),
            (UsbDeviceClass::MassStorage, 0x08),
            (UsbDeviceClass::Hub, 0x09),
            (UsbDeviceClass::Video, 0x0E),
            (UsbDeviceClass::VendorSpecific, 0xFF),
        ];

        for (class, code) in classes {
            assert_eq!(class.code(), code);
            assert_eq!(UsbDeviceClass::from_code(code), class);
        }
    }
}
