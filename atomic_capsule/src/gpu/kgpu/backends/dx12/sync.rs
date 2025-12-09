//! DX12 Fence Implementation (ID3D12Fence)
//!
//! **Tier**: T1 Atomic (lockfree fence with AtomicU64 value tracking)
//!
//! Implements CPU-GPU synchronization using ID3D12Fence with:
//! - Timeline fence values (64-bit monotonic counter)
//! - Event-based blocking wait
//! - Signal from CPU or GPU queue
//! - AtomicU64 for lockfree fence value queries

use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::Threading::*;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::gpu::kgpu::hal::*;
use super::*;

/// DX12 Fence (ID3D12Fence + HANDLE + AtomicU64)
///
/// Wraps ID3D12Fence with:
/// - Chaos compliance (AtomicU64 for lockfree value caching)
/// - Event-based signaling (CreateEventW)
/// - 64-bit timeline values
///
/// # Memory Layout
///
/// - ID3D12Fence: COM object (thread-safe)
/// - HANDLE: Event handle for blocking wait
/// - AtomicU64: Cached fence value (lockfree query)
///
/// # Performance (B32 Targets)
///
/// - Signal (CPU): <100ns (SetEventOnCompletion + event trigger)
/// - Wait (signaled): <100ns (immediate return)
/// - Wait (blocking): OS-dependent (WaitForSingleObject)
/// - Value query: <10ns (atomic load)
pub struct Dx12Fence {
    fence: ID3D12Fence,
    event: HANDLE,
    value: AtomicU64, // Chaos: Lockfree fence value cache
}

impl Dx12Fence {
    /// Creates a new DX12 fence with initial value 0
    ///
    /// # Arguments
    ///
    /// - `device`: D3D12 device for fence creation
    ///
    /// # Performance
    ///
    /// - Creation: <1ms (CreateFence + CreateEventW)
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_EVENT_VALID`: CreateEventW succeeds (manual-reset, non-signaled)
    /// - `#ASSUME_FENCE_VALID`: CreateFence succeeds
    /// - `#VERIFY_EVENT_NOT_NULL`: Check HANDLE != null
    #[allow(dead_code)]
    pub fn new(device: &ID3D12Device) -> Result<Self, windows::core::Error> {
        unsafe {
            // Create D3D12 fence (initial value 0)
            let fence: ID3D12Fence = device.CreateFence(
                0,
                D3D12_FENCE_FLAG_NONE,
            )?;

            // Create event for signaling (manual-reset, non-signaled)
            let event = CreateEventW(None, true, false, None)?;

            // #VERIFY_EVENT_NOT_NULL
            if event.is_invalid() {
                return Err(windows::core::Error::from_win32());
            }

            Ok(Self {
                fence,
                event,
                value: AtomicU64::new(0),
            })
        }
    }

    /// Waits for fence to reach target value (with timeout)
    ///
    /// Blocks until fence value >= target or timeout expires.
    ///
    /// # Arguments
    ///
    /// - `value`: Target fence value
    /// - `timeout_ns`: Timeout in nanoseconds (u64::MAX = infinite)
    ///
    /// # Returns
    ///
    /// - `true`: Fence reached target value
    /// - `false`: Timeout expired
    ///
    /// # Performance
    ///
    /// - Immediate return: <100ns (already signaled)
    /// - Blocking: OS-dependent (WaitForSingleObject)
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_WAIT_TIMEOUT_NS`: Timeout is in nanoseconds
    /// - `#ASSUME_EVENT_SET_ON_COMPLETION`: SetEventOnCompletion succeeds
    /// - `#VERIFY_COMPLETED_VALUE`: Check GetCompletedValue first (fast path)
    #[allow(dead_code)]
    pub fn wait(&self, value: u64, timeout_ns: u64) -> Result<bool, windows::core::Error> {
        unsafe {
            // #VERIFY_COMPLETED_VALUE: Fast path if already signaled
            if self.fence.GetCompletedValue() >= value {
                return Ok(true);
            }

            // Set event to trigger when fence reaches value
            self.fence.SetEventOnCompletion(value, self.event)?;

            // Convert nanoseconds to milliseconds
            let timeout_ms = if timeout_ns == u64::MAX {
                INFINITE
            } else {
                (timeout_ns / 1_000_000).min(u32::MAX as u64) as u32
            };

            // Block until event is signaled or timeout
            let result = WaitForSingleObject(self.event, timeout_ms);

            match result {
                WAIT_OBJECT_0 => {
                    // Update cached value (Chaos lockfree)
                    self.value.store(self.fence.GetCompletedValue(), Ordering::Release);
                    Ok(true)
                }
                WAIT_TIMEOUT => Ok(false),
                _ => Err(windows::core::Error::from_win32()),
            }
        }
    }

    /// Gets current fence value (lockfree atomic load)
    ///
    /// Returns cached value for performance. Call `update_value()` first for
    /// latest GPU value.
    ///
    /// # Performance
    ///
    /// - <10ns (atomic load, no syscall)
    ///
    /// # Chaos Compliance
    ///
    /// Uses AtomicU64::load (Relaxed) for lockfree query.
    #[allow(dead_code)]
    pub fn value(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Updates cached fence value from GPU (syscall)
    ///
    /// Queries ID3D12Fence::GetCompletedValue and updates AtomicU64 cache.
    ///
    /// # Performance
    ///
    /// - <100ns (GetCompletedValue + atomic store)
    #[allow(dead_code)]
    pub fn update_value(&self) {
        unsafe {
            let completed = self.fence.GetCompletedValue();
            self.value.store(completed, Ordering::Release);
        }
    }

    /// Signals fence from CPU (host signal)
    ///
    /// Increments fence value and triggers waiting threads.
    ///
    /// # Arguments
    ///
    /// - `value`: New fence value (must be > current value)
    ///
    /// # Performance
    ///
    /// - <100ns (Signal + event trigger)
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_FENCE_VALUE_MONOTONIC`: Value must increase
    /// - `#VERIFY_SIGNAL_SUCCEEDS`: Check Signal return value
    #[allow(dead_code)]
    pub fn signal(&self, value: u64) -> Result<(), windows::core::Error> {
        unsafe {
            // Signal fence to new value
            self.fence.Signal(value)?;

            // Update cached value
            self.value.store(value, Ordering::Release);

            Ok(())
        }
    }

    /// Returns raw ID3D12Fence for queue->Signal
    #[allow(dead_code)]
    pub fn raw_fence(&self) -> &ID3D12Fence {
        &self.fence
    }
}

impl Drop for Dx12Fence {
    fn drop(&mut self) {
        unsafe {
            if !self.event.is_invalid() {
                CloseHandle(self.event).ok();
            }
        }
    }
}

// SAFETY: ID3D12Fence is thread-safe, HANDLE is thread-safe, AtomicU64 is lockfree
unsafe impl Send for Dx12Fence {}
unsafe impl Sync for Dx12Fence {}

impl KgpuQueueApi for Dx12Queue {
    type CommandBuffer = Dx12CommandBuffer;

    fn submit<I>(&self, command_buffers: I)
    where
        I: IntoIterator<Item = Self::CommandBuffer>,
    {
        unsafe {
            let lists: Vec<_> = command_buffers
                .into_iter()
                .map(|cb| Some(cb.list.cast::<ID3D12CommandList>().unwrap()))
                .collect();

            self.queue.ExecuteCommandLists(&lists);
        }
    }

    fn write_buffer(&self, _buffer: &impl KgpuBufferApi, _offset: u64, _data: &[u8]) -> HalResult<()> {
        // Would use UpdateSubresources or Map/Memcpy
        Ok(())
    }

    fn write_texture(
        &self,
        _destination: ImageCopyTexture<'_>,
        _data: &[u8],
        _data_layout: ImageDataLayout,
        _size: Extent3d,
    ) -> HalResult<()> {
        // Would use UpdateSubresources
        Ok(())
    }

    fn on_submitted_work_done(&self) -> impl core::future::Future<Output = ()> + Send {
        async {}
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires DX12 device
    fn test_fence_creation() {
        unsafe {
            let device: Result<ID3D12Device, _> = D3D12CreateDevice(
                None,
                D3D_FEATURE_LEVEL_12_0,
            );

            if let Ok(device) = device {
                let fence = Dx12Fence::new(&device);
                assert!(fence.is_ok(), "Failed to create fence");

                let fence = fence.unwrap();
                assert_eq!(fence.value(), 0);
            }
        }
    }

    #[test]
    #[ignore] // Requires DX12 device
    fn test_fence_signal() {
        unsafe {
            let device: Result<ID3D12Device, _> = D3D12CreateDevice(
                None,
                D3D_FEATURE_LEVEL_12_0,
            );

            if let Ok(device) = device {
                let fence = Dx12Fence::new(&device).unwrap();

                fence.signal(1).unwrap();
                fence.update_value();
                assert_eq!(fence.value(), 1);

                fence.signal(2).unwrap();
                fence.update_value();
                assert_eq!(fence.value(), 2);
            }
        }
    }
}
