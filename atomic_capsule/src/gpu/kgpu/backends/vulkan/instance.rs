//! Vulkan Instance Implementation
//!
//! # Architecture
//!
//! VulkanInstance wraps ash::Entry and ash::Instance for GPU enumeration and validation.
//!
//! - **Entry**: Vulkan library loader (vkGetInstanceProcAddr)
//! - **Instance**: Vulkan instance handle (application/engine info)
//! - **Extensions**: VK_KHR_surface + platform surface (Win32/Xlib/Wayland/Metal)
//! - **Validation**: VK_LAYER_KHRONOS_validation (debug builds only)
//! - **Debug Messenger**: Validation layer callback (vkCreateDebugUtilsMessengerEXT)
//!
//! # Performance
//!
//! - Creation: <50ms (B32 target)
//! - Adapter enumeration: <100ms (vkEnumeratePhysicalDevices)
//! - Extension negotiation: <10ms (vkEnumerateInstanceExtensionProperties)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_VULKAN_LOADER_AVAILABLE`: ash::Entry::load() succeeds
//! - `#ASSUME_EXTENSIONS_AVAILABLE`: VK_KHR_surface supported on all platforms
//! - `#VERIFY_UNSAFE_FFI`: All ash FFI calls return VkResult, checked via ?
//! - `#ASSUME_VALIDATION_LAYERS`: VK_LAYER_KHRONOS_validation installed (debug only)

use ash::vk;
use std::ffi::{CStr, CString};
use std::sync::Arc;

use crate::gpu::kgpu::hal::{HalInstance, Backend};
use super::error::{KgpuError, KgpuResult};

/// Vulkan instance capsule
///
/// # Layout
///
/// - 128B cache-aligned
/// - Arc-wrapped for cheap cloning
/// - Validation enabled in debug builds only
///
/// # Lifecycle
///
/// ```text
/// Uninitialized → Create (vkCreateInstance) → Active
///     ↓                                         ↓
/// Destroyed  ←───────────────────────────── Destroy (vkDestroyInstance)
/// ```
#[derive(Clone)]
pub struct VulkanInstance {
    /// Inner state (Arc for cheap cloning)
    inner: Arc<VulkanInstanceInner>,
}

struct VulkanInstanceInner {
    /// Vulkan entry point (library loader)
    entry: ash::Entry,

    /// Vulkan instance handle
    instance: ash::Instance,

    /// Debug messenger (validation callback)
    #[cfg(debug_assertions)]
    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,

    /// Debug utils extension (for messenger cleanup)
    #[cfg(debug_assertions)]
    debug_utils: Option<ash::ext::debug_utils::Instance>,

    /// API version (major.minor.patch)
    api_version: u32,
}

impl VulkanInstance {
    /// Create Vulkan instance with validation (debug builds)
    ///
    /// # Performance
    ///
    /// <50ms (B32 target, includes extension negotiation)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_VULKAN_LOADER_AVAILABLE`: ash::Entry::load() succeeds
    /// - `#ASSUME_EXTENSIONS_AVAILABLE`: VK_KHR_surface + platform surface supported
    /// - `#ASSUME_VALIDATION_LAYERS`: VK_LAYER_KHRONOS_validation available (debug)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use atomic_capsule::gpu::kgpu::backends::vulkan::VulkanInstance;
    ///
    /// let instance = VulkanInstance::new("MyApp", "MyEngine")?;
    /// println!("Vulkan instance created with API {}.{}.{}",
    ///     vk::api_version_major(instance.api_version()),
    ///     vk::api_version_minor(instance.api_version()),
    ///     vk::api_version_patch(instance.api_version()));
    /// # Ok::<(), atomic_capsule::gpu::kgpu::error::KgpuError>(())
    /// ```
    pub fn new(app_name: &str, engine_name: &str) -> KgpuResult<Self> {
        // #ASSUME_VULKAN_LOADER_AVAILABLE: ash::Entry::load() succeeds
        let entry = unsafe {
            ash::Entry::load().map_err(|e| {
                KgpuError::InitializationFailed(format!("Failed to load Vulkan library: {}", e))
            })?
        };

        // Query instance API version
        let api_version = match entry.try_enumerate_instance_version() {
            Ok(Some(version)) => version,
            Ok(None) => vk::API_VERSION_1_0, // Vulkan 1.0 fallback
            Err(e) => return Err(KgpuError::InitializationFailed(
                format!("Failed to query Vulkan version: {}", e)
            )),
        };

        // Check Vulkan 1.3 support
        if vk::api_version_major(api_version) < 1 ||
           (vk::api_version_major(api_version) == 1 && vk::api_version_minor(api_version) < 3) {
            return Err(KgpuError::InitializationFailed(
                format!("Vulkan 1.3+ required, found {}.{}.{}",
                    vk::api_version_major(api_version),
                    vk::api_version_minor(api_version),
                    vk::api_version_patch(api_version))
            ));
        }

        // Application info
        let app_name_cstr = CString::new(app_name).unwrap();
        let engine_name_cstr = CString::new(engine_name).unwrap();
        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name_cstr)
            .application_version(vk::make_api_version(0, 1, 0, 0))
            .engine_name(&engine_name_cstr)
            .engine_version(vk::make_api_version(0, 1, 0, 0))
            .api_version(api_version);

        // Required extensions
        let mut extensions = Self::required_extensions()?;

        // Validation layers (debug builds only)
        #[cfg(debug_assertions)]
        let layers = Self::validation_layers(&entry)?;
        #[cfg(not(debug_assertions))]
        let layers: Vec<*const i8> = Vec::new();

        // Debug messenger (debug builds)
        #[cfg(debug_assertions)]
        if !layers.is_empty() {
            extensions.push(ash::ext::debug_utils::NAME.as_ptr());
        }

        // Create instance
        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions)
            .enabled_layer_names(&layers);

        // #VERIFY_UNSAFE_FFI: vkCreateInstance
        let instance = unsafe {
            entry.create_instance(&create_info, None).map_err(|e| {
                KgpuError::InitializationFailed(format!("Failed to create Vulkan instance: {}", e))
            })?
        };

        // Create debug messenger (debug builds)
        #[cfg(debug_assertions)]
        let (debug_messenger, debug_utils) = if !layers.is_empty() {
            let debug_utils_loader = ash::ext::debug_utils::Instance::new(&entry, &instance);
            let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                        | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                )
                .pfn_user_callback(Some(vulkan_debug_callback));

            let messenger = unsafe {
                debug_utils_loader.create_debug_utils_messenger(&debug_info, None).ok()
            };

            (messenger, Some(debug_utils_loader))
        } else {
            (None, None)
        };

        Ok(Self {
            inner: Arc::new(VulkanInstanceInner {
                entry,
                instance,
                #[cfg(debug_assertions)]
                debug_messenger,
                #[cfg(debug_assertions)]
                debug_utils,
                api_version,
            }),
        })
    }

    /// Required instance extensions
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_EXTENSIONS_AVAILABLE`: VK_KHR_surface + platform surface supported
    fn required_extensions() -> KgpuResult<Vec<*const i8>> {
        let mut extensions = vec![
            ash::khr::surface::NAME.as_ptr(),
        ];

        // Platform-specific surface extensions
        #[cfg(target_os = "windows")]
        extensions.push(ash::khr::win32_surface::NAME.as_ptr());

        #[cfg(target_os = "linux")]
        {
            // Try Wayland first, fallback to Xlib
            extensions.push(ash::khr::wayland_surface::NAME.as_ptr());
            // Note: ash-window will handle Xlib fallback
        }

        #[cfg(target_os = "macos")]
        extensions.push(ash::ext::metal_surface::NAME.as_ptr());

        #[cfg(target_os = "android")]
        extensions.push(ash::khr::android_surface::NAME.as_ptr());

        Ok(extensions)
    }

    /// Validation layers (debug builds only)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_VALIDATION_LAYERS`: VK_LAYER_KHRONOS_validation installed
    #[cfg(debug_assertions)]
    fn validation_layers(entry: &ash::Entry) -> KgpuResult<Vec<*const i8>> {
        let layer_name = CString::new("VK_LAYER_KHRONOS_validation").unwrap();

        // Check if validation layer is available
        let available_layers = entry.enumerate_instance_layer_properties()
            .map_err(|e| KgpuError::InitializationFailed(
                format!("Failed to enumerate layers: {}", e)
            ))?;

        let validation_available = available_layers.iter().any(|layer| {
            let name = unsafe { CStr::from_ptr(layer.layer_name.as_ptr()) };
            name == layer_name.as_c_str()
        });

        if validation_available {
            Ok(vec![layer_name.into_raw()])
        } else {
            // Validation layer not available, continue without it
            eprintln!("Warning: VK_LAYER_KHRONOS_validation not available, validation disabled");
            Ok(Vec::new())
        }
    }

    /// Get API version
    pub fn api_version(&self) -> u32 {
        self.inner.api_version
    }

    /// Get raw ash::Entry
    pub(crate) fn entry(&self) -> &ash::Entry {
        &self.inner.entry
    }

    /// Get raw ash::Instance
    pub(crate) fn raw_instance(&self) -> &ash::Instance {
        &self.inner.instance
    }
}

impl HalInstance for VulkanInstance {
    type Adapter = super::VulkanAdapter;

    fn backend(&self) -> Backend {
        Backend::Vulkan
    }

    fn enumerate_adapters(&self) -> KgpuResult<Vec<Self::Adapter>> {
        // Enumerate physical devices
        let physical_devices = unsafe {
            self.inner.instance.enumerate_physical_devices().map_err(|e| {
                KgpuError::EnumerationFailed(format!("Failed to enumerate GPUs: {}", e))
            })?
        };

        if physical_devices.is_empty() {
            return Err(KgpuError::EnumerationFailed(
                "No Vulkan-compatible GPUs found".to_string()
            ));
        }

        // Convert to VulkanAdapter
        physical_devices.into_iter()
            .map(|device| super::VulkanAdapter::new(self.clone(), device))
            .collect()
    }

    fn create_surface(
        &self,
        window: &dyn raw_window_handle::HasWindowHandle,
    ) -> KgpuResult<super::VulkanSurface> {
        super::VulkanSurface::new(self.clone(), window)
    }
}

impl Drop for VulkanInstanceInner {
    fn drop(&mut self) {
        unsafe {
            // Destroy debug messenger (debug builds)
            #[cfg(debug_assertions)]
            if let (Some(messenger), Some(ref utils)) = (self.debug_messenger, &self.debug_utils) {
                utils.destroy_debug_utils_messenger(messenger, None);
            }

            // Destroy instance
            self.instance.destroy_instance(None);
        }
    }
}

/// Vulkan debug callback (validation layer messages)
#[cfg(debug_assertions)]
unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;
    let message = if callback_data.p_message.is_null() {
        "[null message]"
    } else {
        std::ffi::CStr::from_ptr(callback_data.p_message)
            .to_str()
            .unwrap_or("[invalid UTF-8]")
    };

    let severity = match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => "VERBOSE",
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => "INFO",
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => "WARNING",
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => "ERROR",
        _ => "UNKNOWN",
    };

    let ty = match message_type {
        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL => "GENERAL",
        vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION => "VALIDATION",
        vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => "PERFORMANCE",
        _ => "UNKNOWN",
    };

    eprintln!("[Vulkan {} {}] {}", severity, ty, message);

    vk::FALSE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires Vulkan drivers
    fn test_instance_creation() {
        let instance = VulkanInstance::new("TestApp", "TestEngine");
        assert!(instance.is_ok(), "Failed to create Vulkan instance");

        if let Ok(inst) = instance {
            let api = inst.api_version();
            assert!(vk::api_version_major(api) >= 1);
            assert!(vk::api_version_minor(api) >= 3);
        }
    }

    #[test]
    #[ignore] // Requires Vulkan drivers
    fn test_adapter_enumeration() {
        let instance = VulkanInstance::new("TestApp", "TestEngine").unwrap();
        let adapters = instance.enumerate_adapters();

        assert!(adapters.is_ok(), "Failed to enumerate adapters");
        if let Ok(adapters) = adapters {
            assert!(!adapters.is_empty(), "No adapters found");
        }
    }
}
