//! Device Driver Framework for Atomic Capsule
//!
//! This module provides hardware device drivers implemented using the
//! Computational Capsule Architecture (Chaos framework).
//!
//! # Available Drivers
//!
//! ## USB Driver (`usb`)
//! World's first 100% lockfree USB/xHCI driver stack:
//! - `UsbControllerCapsule` (T1 Atomic, 512B) - xHCI host controller state
//! - `UsbDeviceCapsule` (T1 Atomic, 256B) - USB device management
//! - `UsbTransferCapsule` (T5 Streaming, 1KB) - Async transfer rings
//!
//! ## PCI Driver (`pci`)
//! World's first 100% lockfree PCI enumeration driver:
//! - `PciEnumeratorCapsule` (T4 Batch, 4KB) - Bus scanning with batch discovery
//! - `PciDeviceCapsule` (T1 Atomic, 256B) - Device configuration space
//! - `PciBarCapsule` (T1 Atomic, 128B) - BAR access and management
//!
//! # Architecture Principles
//!
//! All drivers follow Chaos framework requirements:
//! - **100% lockfree**: NO mutex, NO RwLock
//! - **Cache-aligned**: Prevent false sharing
//! - **Generation counters**: ABA prevention
//! - **Atomic state machines**: CAS-based transitions
//!
//! # Safety
//!
//! All drivers document their safety assumptions using the ASSUM framework.
//! Each capsule contains explicit #ASSUME and #VERIFY tags for audit compliance.

pub mod usb;
pub mod pci;

// Re-export USB driver components at drivers level
pub use usb::{
    // Controller
    UsbControllerCapsule, UsbControllerSnapshot, UsbControllerState,
    usb_controller,
    XHCI_PCI_CLASS, XHCI_PCI_SUBCLASS, XHCI_PCI_INTERFACE,

    // Device
    UsbDeviceCapsule, UsbDeviceSnapshot, UsbDeviceState,
    UsbDeviceSpeed, UsbDeviceClass,

    // Transfer
    UsbTransferCapsule, TransferRingSnapshot, TransferRingState,
    TransferType, TransferRequest,
};

// Re-export PCI driver components at drivers level
pub use pci::{
    // Enumerator
    PciEnumeratorCapsule, PciEnumeratorSnapshot, PciEnumeratorState,
    PciConfigAccess, EcamAccess, LegacyAccess, NullAccess,
    PCI_MAX_BUSES, PCI_MAX_DEVICES, PCI_MAX_FUNCTIONS,
    ECAM_DEVICE_STRIDE, CONFIG_SPACE_SIZE, EXT_CONFIG_SPACE_SIZE,

    // Device
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

    // BAR
    PciBarCapsule, PciBarSnapshot, PciBarState,
    BarType, BarWidth,
    BAR_TYPE_MEMORY, BAR_TYPE_IO, BAR_MEMORY_TYPE_MASK,
    BAR_MEMORY_32BIT, BAR_MEMORY_64BIT, BAR_MEMORY_PREFETCHABLE,
    BAR_MEMORY_BASE_MASK, BAR_IO_BASE_MASK,
};
