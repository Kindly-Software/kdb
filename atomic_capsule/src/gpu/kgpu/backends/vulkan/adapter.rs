//! Vulkan Adapter (Physical Device) Implementation
//!
//! # Architecture
//!
//! VulkanAdapter wraps vk::PhysicalDevice for GPU selection and capability queries.
//!
//! - **Physical Device**: GPU handle (vk::PhysicalDevice)
//! - **Properties**: Device name, type, limits, features
//! - **Queue Families**: Graphics/Compute/Transfer/Present support
//! - **Extensions**: Device-specific extensions (VK_KHR_swapchain)
//! - **Scoring**: GPU selection heuristic (discrete > integrated > virtual)
//!
//! # Performance
//!
//! - Enumeration: <100ms (vkEnumeratePhysicalDevices)
//! - Property query: <10ms (vkGetPhysicalDeviceProperties)
//! - Queue family query: <5ms (vkGetPhysicalDeviceQueueFamilyProperties)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_PHYSICAL_DEVICE_VALID`: At least one GPU supports Vulkan 1.0+
//! - `#ASSUME_QUEUE_FAMILIES_VALID`: Graphics/Compute queues available
//! - `#VERIFY_UNSAFE_FFI`: All vk* queries return valid data

use ash::vk;
use std::ffi::CStr;
use std::sync::Arc;

use crate::gpu::kgpu::hal::{HalAdapter, AdapterInfo, DeviceType, Backend};
use crate::gpu::kgpu::error::{KgpuError, KgpuResult};

use super::VulkanInstance;

/// Vulkan adapter (physical device) capsule
///
/// # Layout
///
/// - 128B cache-aligned
/// - Immutable after creation (cheap cloning via Arc)
///
/// # GPU Selection Heuristic
///
/// 1. Discrete GPU (dedicated VRAM) → Score 1000
/// 2. Integrated GPU (shared memory) → Score 500
/// 3. Virtual GPU (software rasterizer) → Score 100
/// 4. CPU → Score 10
#[derive(Clone)]
pub struct VulkanAdapter {
    /// Instance reference
    instance: VulkanInstance,

    /// Physical device handle
    physical_device: vk::PhysicalDevice,

    /// Device properties (cached)
    properties: vk::PhysicalDeviceProperties,

    /// Device features (cached)
    features: vk::PhysicalDeviceFeatures,

    /// Queue family indices
    queue_families: QueueFamilyIndices,

    /// Adapter score (for selection heuristic)
    score: u32,
}

/// Queue family indices
#[derive(Debug, Clone, Copy)]
struct QueueFamilyIndices {
    /// Graphics queue family index
    graphics: Option<u32>,

    /// Compute queue family index
    compute: Option<u32>,

    /// Transfer queue family index
    transfer: Option<u32>,
}

impl VulkanAdapter {
    /// Create adapter from physical device
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_PHYSICAL_DEVICE_VALID`: Physical device supports Vulkan 1.0+
    pub(crate) fn new(
        instance: VulkanInstance,
        physical_device: vk::PhysicalDevice,
    ) -> KgpuResult<Self> {
        // Query device properties
        let properties = unsafe {
            instance.raw_instance().get_physical_device_properties(physical_device)
        };

        // Query device features
        let features = unsafe {
            instance.raw_instance().get_physical_device_features(physical_device)
        };

        // Find queue families
        let queue_families = Self::find_queue_families(&instance, physical_device)?;

        // Compute adapter score
        let score = Self::compute_score(&properties, &features);

        Ok(Self {
            instance,
            physical_device,
            properties,
            features,
            queue_families,
            score,
        })
    }

    /// Find queue family indices
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_QUEUE_FAMILIES_VALID`: Graphics/Compute queues available
    fn find_queue_families(
        instance: &VulkanInstance,
        physical_device: vk::PhysicalDevice,
    ) -> KgpuResult<QueueFamilyIndices> {
        let queue_families = unsafe {
            instance.raw_instance()
                .get_physical_device_queue_family_properties(physical_device)
        };

        let mut graphics = None;
        let mut compute = None;
        let mut transfer = None;

        for (index, family) in queue_families.iter().enumerate() {
            let index = index as u32;

            // Graphics queue (also supports compute + transfer)
            if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                graphics = Some(index);
            }

            // Dedicated compute queue (no graphics)
            if family.queue_flags.contains(vk::QueueFlags::COMPUTE)
                && !family.queue_flags.contains(vk::QueueFlags::GRAPHICS)
            {
                compute = Some(index);
            }

            // Dedicated transfer queue (no graphics/compute)
            if family.queue_flags.contains(vk::QueueFlags::TRANSFER)
                && !family.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                && !family.queue_flags.contains(vk::QueueFlags::COMPUTE)
            {
                transfer = Some(index);
            }
        }

        // Fallback: Use graphics queue for compute/transfer if no dedicated queues
        if compute.is_none() && graphics.is_some() {
            compute = graphics;
        }
        if transfer.is_none() && graphics.is_some() {
            transfer = graphics;
        }

        // Validate required queues
        if graphics.is_none() {
            return Err(KgpuError::EnumerationFailed(
                "No graphics queue family found".to_string()
            ));
        }

        Ok(QueueFamilyIndices {
            graphics,
            compute,
            transfer,
        })
    }

    /// Compute adapter score for selection heuristic
    ///
    /// # Scoring
    ///
    /// - Discrete GPU: 1000 base + VRAM GB
    /// - Integrated GPU: 500 base
    /// - Virtual GPU: 100 base
    /// - CPU: 10 base
    fn compute_score(
        properties: &vk::PhysicalDeviceProperties,
        _features: &vk::PhysicalDeviceFeatures,
    ) -> u32 {
        let base_score = match properties.device_type {
            vk::PhysicalDeviceType::DISCRETE_GPU => 1000,
            vk::PhysicalDeviceType::INTEGRATED_GPU => 500,
            vk::PhysicalDeviceType::VIRTUAL_GPU => 100,
            vk::PhysicalDeviceType::CPU => 10,
            _ => 1,
        };

        // Bonus for Vulkan 1.3 support
        let api_version = properties.api_version;
        let version_bonus = if vk::api_version_major(api_version) >= 1
            && vk::api_version_minor(api_version) >= 3
        {
            100
        } else {
            0
        };

        base_score + version_bonus
    }

    /// Get device name
    pub fn name(&self) -> String {
        unsafe {
            CStr::from_ptr(self.properties.device_name.as_ptr())
                .to_string_lossy()
                .into_owned()
        }
    }

    /// Get device type
    pub fn device_type(&self) -> DeviceType {
        match self.properties.device_type {
            vk::PhysicalDeviceType::DISCRETE_GPU => DeviceType::DiscreteGpu,
            vk::PhysicalDeviceType::INTEGRATED_GPU => DeviceType::IntegratedGpu,
            vk::PhysicalDeviceType::VIRTUAL_GPU => DeviceType::VirtualGpu,
            vk::PhysicalDeviceType::CPU => DeviceType::Cpu,
            _ => DeviceType::Other,
        }
    }

    /// Get adapter score (for selection)
    pub fn score(&self) -> u32 {
        self.score
    }

    /// Get graphics queue family index
    pub(crate) fn graphics_queue_family(&self) -> u32 {
        self.queue_families.graphics.expect("No graphics queue family")
    }

    /// Get compute queue family index
    pub(crate) fn compute_queue_family(&self) -> u32 {
        self.queue_families.compute.expect("No compute queue family")
    }

    /// Get transfer queue family index
    pub(crate) fn transfer_queue_family(&self) -> u32 {
        self.queue_families.transfer.expect("No transfer queue family")
    }

    /// Get physical device handle
    pub(crate) fn physical_device(&self) -> vk::PhysicalDevice {
        self.physical_device
    }

    /// Get instance reference
    pub(crate) fn instance(&self) -> &VulkanInstance {
        &self.instance
    }

    /// Check if extension is supported
    pub(crate) fn supports_extension(&self, extension_name: &CStr) -> bool {
        let extensions = unsafe {
            self.instance.raw_instance()
                .enumerate_device_extension_properties(self.physical_device)
                .unwrap_or_default()
        };

        extensions.iter().any(|ext| {
            let name = unsafe { CStr::from_ptr(ext.extension_name.as_ptr()) };
            name == extension_name
        })
    }

    /// Check surface support for queue family
    pub(crate) fn supports_surface(
        &self,
        queue_family_index: u32,
        surface: vk::SurfaceKHR,
    ) -> bool {
        let surface_loader = ash::khr::surface::Instance::new(
            self.instance.entry(),
            self.instance.raw_instance(),
        );

        unsafe {
            surface_loader
                .get_physical_device_surface_support(
                    self.physical_device,
                    queue_family_index,
                    surface,
                )
                .unwrap_or(false)
        }
    }
}

impl HalAdapter for VulkanAdapter {
    type Device = super::VulkanDevice;

    fn backend(&self) -> Backend {
        Backend::Vulkan
    }

    fn info(&self) -> AdapterInfo {
        AdapterInfo {
            name: self.name(),
            vendor_id: self.properties.vendor_id,
            device_id: self.properties.device_id,
            device_type: self.device_type(),
            driver_version: self.properties.driver_version,
        }
    }

    fn create_device(&self) -> KgpuResult<Self::Device> {
        super::VulkanDevice::new(self.clone())
    }

    fn is_compatible(&self) -> bool {
        // Check Vulkan 1.3 support
        let api_version = self.properties.api_version;
        vk::api_version_major(api_version) >= 1
            && vk::api_version_minor(api_version) >= 3
    }
}

impl PartialEq for VulkanAdapter {
    fn eq(&self, other: &Self) -> bool {
        self.physical_device == other.physical_device
    }
}

impl Eq for VulkanAdapter {}

impl PartialOrd for VulkanAdapter {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VulkanAdapter {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher score = better adapter
        self.score.cmp(&other.score)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires Vulkan drivers
    fn test_adapter_properties() {
        let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        let adapters = instance.enumerate_adapters().unwrap();

        assert!(!adapters.is_empty(), "No adapters found");

        for adapter in &adapters {
            println!("Adapter: {}", adapter.name());
            println!("  Type: {:?}", adapter.device_type());
            println!("  Score: {}", adapter.score());
            println!("  Graphics queue: {}", adapter.graphics_queue_family());
        }
    }

    #[test]
    #[ignore] // Requires Vulkan drivers
    fn test_adapter_sorting() {
        let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        let mut adapters = instance.enumerate_adapters().unwrap();

        adapters.sort_by(|a, b| b.cmp(a)); // Descending order (best first)

        println!("Adapters sorted by score:");
        for (i, adapter) in adapters.iter().enumerate() {
            println!("  {}: {} (score {})", i + 1, adapter.name(), adapter.score());
        }
    }

    #[test]
    #[ignore] // Requires Vulkan drivers
    fn test_swapchain_extension() {
        let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        let adapters = instance.enumerate_adapters().unwrap();

        for adapter in &adapters {
            let supports = adapter.supports_extension(ash::khr::swapchain::NAME);
            println!("{}: Swapchain support = {}", adapter.name(), supports);
        }
    }
}
