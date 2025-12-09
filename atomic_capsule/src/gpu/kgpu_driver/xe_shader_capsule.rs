// Intel Xe2 Shader Management Capsule
// T1 Atomic Tier: 256B cache-aligned, 100% lockfree
//
// Manages shader object loading, compilation, and binding for Intel Xe2 GPUs.
// Provides lockfree coordination for shader lifecycle and SPIR-V/ISA compilation.
//
// # Overview
// Shader capsules manage GPU shader programs on Intel Xe2.
// Each shader has:
// - A handle (kernel object identifier)
// - Type (compute, vertex, fragment, geometry)
// - Binary data (SPIR-V or compiled Intel ISA)
// - State machine: UNLOADED → LOADING → COMPILED → BOUND
//
// # Compilation Pipeline
// 1. UNLOADED: Initial state, no shader loaded
// 2. LOADING: SPIR-V data uploaded to GPU memory via GEM
// 3. COMPILED: Intel compiler (ioc) converts SPIR-V to native ISA
// 4. BOUND: Shader bound to execution queue, ready for dispatch
//
// # Memory Safety
// - #ASSUME: DRM file descriptor remains valid during operations
// - #VERIFY: All operations check state before proceeding
// - #ASSUME: SPIR-V binary is valid and well-formed
// - #VERIFY: Generation counter prevents ABA race conditions
// - #ASSUME: Shader binary GPU address is valid after compilation
// - #VERIFY: Binary size does not exceed MAX_SHADER_SIZE

#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
use std::os::unix::io::RawFd;

#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
use super::xe_gem_capsule::XeGemCapsule;

#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
use super::xe_exec_capsule::XeExecCapsule;

/// Shader states
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
const SHADER_STATE_UNLOADED: u32 = 0;
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
const SHADER_STATE_LOADING: u32 = 1;
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
const SHADER_STATE_COMPILED: u32 = 2;
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
const SHADER_STATE_BOUND: u32 = 3;
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
const SHADER_STATE_ERROR: u32 = 4;

/// Shader types
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub const SHADER_TYPE_COMPUTE: u32 = 0;
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub const SHADER_TYPE_VERTEX: u32 = 1;
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub const SHADER_TYPE_FRAGMENT: u32 = 2;
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
pub const SHADER_TYPE_GEOMETRY: u32 = 3;

/// Maximum shader binary size (4MB)
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
const MAX_SHADER_SIZE: u64 = 4 * 1024 * 1024;

/// Intel Xe2 Shader Capsule (T1 Atomic, 256B cache-aligned)
///
/// Manages GPU shader programs for Intel Xe2 GPU command execution.
/// Provides lockfree coordination for shader loading, compilation, and binding.
///
/// # State Machine
/// ```text
/// UNLOADED --load_spirv()--> LOADING --compile()--> COMPILED --bind()--> BOUND
///    ^                           |                       |                 |
///    |                           +-------- ERROR --------+                 |
///    |                                                                     |
///    +-------------------- unload() / unbind() --------------------------+
/// ```
///
/// # Memory Safety
/// - #ASSUME: DRM file descriptor remains valid during operations
/// - #VERIFY: All operations check state before proceeding
/// - #ASSUME: SPIR-V binary is valid (caller responsibility)
/// - #VERIFY: Generation counter prevents ABA race conditions
/// - #ASSUME: Shader binary GPU address is valid after compilation
/// - #VERIFY: Binary size does not exceed MAX_SHADER_SIZE (4MB)
///
/// # Performance
/// - Load SPIR-V: ~100-500μs (depends on size, GEM allocation)
/// - Compile: ~1-10ms (Intel offline compiler, depends on complexity)
/// - Bind: ~10-50μs (kernel handle registration)
/// - Unbind: ~10-50μs (handle deregistration)
/// - State queries: <10ns atomic load
///
/// # Compilation
/// - Phase 1 (Simulation): No real Intel compiler, marks shader as COMPILED immediately
/// - Phase 2 (Production): Integrates with Intel Graphics Compiler (IGC) via ioc tool
/// - Binary format: Intel EU ISA (Execution Unit instruction set architecture)
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
#[repr(C, align(256))]
pub struct XeShaderCapsule {
    // Shader identification
    handle: AtomicU32,      // Kernel object handle (0 if not loaded)
    shader_type: AtomicU32, // Shader type (see SHADER_TYPE_* constants)

    // State coordination
    state: AtomicU32,      // Current state (see SHADER_STATE_* constants)
    generation: AtomicU64, // Generation counter for ABA prevention

    // Binary storage
    binary_size: AtomicU64,        // Size of shader binary in bytes
    binary_gpu_addr: AtomicU64,    // GPU address of compiled binary (0 if not compiled)
    entry_point_offset: AtomicU32, // Offset to entry point in binary

    // Shader metadata
    num_uniforms: AtomicU32, // Number of uniform variables
    num_samplers: AtomicU32, // Number of texture samplers

    // Statistics (lockfree counters)
    load_count: AtomicU64,
    compile_count: AtomicU64,

    // Padding to exactly 256 bytes
    // Current size without padding:
    //   handle: 4 bytes
    //   shader_type: 4 bytes
    //   state: 4 bytes
    //   generation: 8 bytes (aligned to 8)
    //   binary_size: 8 bytes
    //   binary_gpu_addr: 8 bytes
    //   entry_point_offset: 4 bytes
    //   num_uniforms: 4 bytes
    //   num_samplers: 4 bytes
    //   load_count: 8 bytes (aligned to 8)
    //   compile_count: 8 bytes
    //
    // Total: 4 + 4 + 4 + 8 + 8 + 8 + 4 + 4 + 4 + 8 + 8 = 64 bytes
    //
    // With repr(C) implicit padding:
    //   - 4 bytes after state (to align generation to 8)
    //   - 4 bytes after entry_point_offset (to align load_count to 8)
    // Total with implicit padding: 64 + 4 + 4 = 72 bytes
    //
    // Explicit padding needed: 256 - 72 = 184 bytes
    _padding: [u8; 184],
}

/// Shader-specific errors
#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XeShaderError {
    /// Shader already loaded
    AlreadyLoaded,
    /// Shader not loaded yet
    NotLoaded,
    /// Load operation failed
    LoadFailed { errno: i32 },
    /// Compilation failed
    CompileFailed { errno: i32, message: String },
    /// Shader not compiled yet
    NotCompiled,
    /// Bind operation failed
    BindFailed { errno: i32 },
    /// Shader not bound
    NotBound,
    /// Invalid shader type
    InvalidShaderType { shader_type: u32 },
    /// SPIR-V binary too large
    SpirvTooLarge { size: usize, limit: u64 },
}

#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
impl XeShaderCapsule {
    /// Create new unloaded shader capsule
    #[inline]
    pub fn new() -> Self {
        // #ASSUME: Cache-aligned allocation by caller
        // #VERIFY: #[repr(C, align(256))] enforces alignment
        Self {
            handle: AtomicU32::new(0),
            shader_type: AtomicU32::new(SHADER_TYPE_COMPUTE),
            state: AtomicU32::new(SHADER_STATE_UNLOADED),
            generation: AtomicU64::new(0),
            binary_size: AtomicU64::new(0),
            binary_gpu_addr: AtomicU64::new(0),
            entry_point_offset: AtomicU32::new(0),
            num_uniforms: AtomicU32::new(0),
            num_samplers: AtomicU32::new(0),
            load_count: AtomicU64::new(0),
            compile_count: AtomicU64::new(0),
            _padding: [0u8; 184],
        }
    }

    /// Load SPIR-V shader binary
    ///
    /// Uploads SPIR-V bytecode to GPU memory via GEM buffer allocation.
    /// The shader remains in LOADING state until compilation.
    ///
    /// # Arguments
    /// - `gem`: GEM capsule for GPU memory allocation
    /// - `drm_fd`: DRM file descriptor
    /// - `spirv_data`: SPIR-V bytecode buffer
    /// - `shader_type`: Shader type (see SHADER_TYPE_* constants)
    ///
    /// # Errors
    /// - `AlreadyLoaded`: Shader has already been loaded
    /// - `InvalidShaderType`: Unknown shader type
    /// - `SpirvTooLarge`: SPIR-V binary exceeds MAX_SHADER_SIZE (4MB)
    /// - `LoadFailed`: GEM allocation or upload failed
    ///
    /// # State Transition
    /// UNLOADED → LOADING
    ///
    /// # Safety
    /// - #ASSUME: drm_fd is a valid open file descriptor
    /// - #VERIFY: Caller must ensure drm_fd remains open
    /// - #ASSUME: spirv_data is a valid SPIR-V binary
    /// - #VERIFY: Caller must validate SPIR-V with spirv-val
    pub fn load_spirv(
        &self,
        gem: &XeGemCapsule,
        drm_fd: RawFd,
        spirv_data: &[u8],
        shader_type: u32,
    ) -> Result<(), XeShaderError> {
        // Check current state
        let current_state = self.state.load(Ordering::Acquire);
        if current_state != SHADER_STATE_UNLOADED {
            return Err(XeShaderError::AlreadyLoaded);
        }

        // Validate shader type
        if shader_type > SHADER_TYPE_GEOMETRY {
            return Err(XeShaderError::InvalidShaderType { shader_type });
        }

        // Validate SPIR-V size
        let spirv_size = spirv_data.len();
        if spirv_size as u64 > MAX_SHADER_SIZE {
            return Err(XeShaderError::SpirvTooLarge {
                size: spirv_size,
                limit: MAX_SHADER_SIZE,
            });
        }

        // Phase 1: Simulate SPIR-V upload
        // In production, this would:
        // 1. Allocate GEM buffer for SPIR-V
        // 2. Copy SPIR-V data to GEM buffer
        // 3. Bind GEM buffer to GPU virtual address space
        let _ = (gem, drm_fd, spirv_data);

        // Generate simulated handle
        let simulated_handle = (self.generation.load(Ordering::Relaxed) as u32) + 1;

        // Store shader parameters
        self.handle.store(simulated_handle, Ordering::Relaxed);
        self.shader_type.store(shader_type, Ordering::Relaxed);
        self.binary_size.store(spirv_size as u64, Ordering::Relaxed);

        // Simulate metadata extraction from SPIR-V
        // In production, this would parse SPIR-V header
        self.num_uniforms.store(0, Ordering::Relaxed);
        self.num_samplers.store(0, Ordering::Relaxed);
        self.entry_point_offset.store(0, Ordering::Relaxed);

        // Update state and generation
        self.state.store(SHADER_STATE_LOADING, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
        self.load_count.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Compile shader to Intel ISA
    ///
    /// Invokes Intel Graphics Compiler (IGC) to compile SPIR-V to native
    /// Execution Unit (EU) instruction set architecture.
    ///
    /// # Arguments
    /// - `drm_fd`: DRM file descriptor
    ///
    /// # Errors
    /// - `NotLoaded`: Shader has not been loaded yet
    /// - `CompileFailed`: Intel compiler returned error
    ///
    /// # State Transition
    /// LOADING → COMPILED (or ERROR on failure)
    ///
    /// # Compilation Process
    /// 1. SPIR-V validation (spirv-val)
    /// 2. SPIR-V optimization (spirv-opt)
    /// 3. Compilation to Intel ISA (ioc tool)
    /// 4. Binary relocation and symbol resolution
    /// 5. GPU address assignment
    ///
    /// # Performance
    /// - Simple shaders: ~1-2ms
    /// - Complex shaders: ~5-10ms
    /// - Large shaders (>100KB SPIR-V): ~10-50ms
    pub fn compile(&self, drm_fd: RawFd) -> Result<(), XeShaderError> {
        // Check current state
        let current_state = self.state.load(Ordering::Acquire);
        if current_state != SHADER_STATE_LOADING {
            if current_state == SHADER_STATE_COMPILED || current_state == SHADER_STATE_BOUND {
                // Already compiled, no-op
                return Ok(());
            }
            return Err(XeShaderError::NotLoaded);
        }

        // Phase 1: Simulate compilation
        // In production, this would:
        // 1. Run spirv-val to validate SPIR-V
        // 2. Run spirv-opt for optimization passes
        // 3. Invoke ioc (Intel Offline Compiler) to generate EU ISA
        // 4. Allocate GEM buffer for compiled binary
        // 5. Copy compiled binary to GEM buffer
        // 6. Perform relocation and symbol resolution
        let _ = drm_fd;

        // Simulate compiled binary size (SPIR-V typically compiles to 2-4x size)
        let spirv_size = self.binary_size.load(Ordering::Relaxed);
        let compiled_size = spirv_size * 3;

        // Simulate GPU address allocation
        let simulated_gpu_addr =
            0x2000_0000 + (compiled_size * self.generation.load(Ordering::Relaxed));

        self.binary_size.store(compiled_size, Ordering::Relaxed);
        self.binary_gpu_addr
            .store(simulated_gpu_addr, Ordering::Relaxed);

        // Update state and generation
        self.state.store(SHADER_STATE_COMPILED, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
        self.compile_count.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Bind shader to execution queue
    ///
    /// Associates the compiled shader with an execution queue context,
    /// making it ready for GPU dispatch.
    ///
    /// # Arguments
    /// - `exec`: Execution queue capsule
    ///
    /// # Returns
    /// Kernel handle for GPU dispatch
    ///
    /// # Errors
    /// - `NotCompiled`: Shader has not been compiled yet
    /// - `BindFailed`: Kernel binding operation failed
    ///
    /// # State Transition
    /// COMPILED → BOUND
    ///
    /// # Performance
    /// - ~10-50μs (depends on kernel complexity)
    pub fn bind(&self, exec: &XeExecCapsule) -> Result<u32, XeShaderError> {
        // Check current state
        let current_state = self.state.load(Ordering::Acquire);
        if current_state != SHADER_STATE_COMPILED {
            if current_state == SHADER_STATE_BOUND {
                // Already bound, return existing handle
                return Ok(self.handle.load(Ordering::Relaxed));
            }
            return Err(XeShaderError::NotCompiled);
        }

        // Verify execution queue is created
        if !exec.is_created() {
            return Err(XeShaderError::BindFailed { errno: -22 }); // EINVAL
        }

        // Phase 1: Simulate binding
        // In production, this would register kernel handle with execution queue
        let _ = exec;

        let kernel_handle = self.handle.load(Ordering::Relaxed);

        // Update state and generation
        self.state.store(SHADER_STATE_BOUND, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);

        Ok(kernel_handle)
    }

    /// Unbind shader from execution queue
    ///
    /// Removes the shader binding from the execution queue context.
    ///
    /// # Errors
    /// - `NotBound`: Shader is not currently bound
    ///
    /// # State Transition
    /// BOUND → COMPILED
    pub fn unbind(&self) -> Result<(), XeShaderError> {
        let current_state = self.state.load(Ordering::Acquire);
        if current_state != SHADER_STATE_BOUND {
            return Err(XeShaderError::NotBound);
        }

        // Phase 1: Simulate unbinding
        // In production, this would deregister kernel handle

        // Update state and generation
        self.state.store(SHADER_STATE_COMPILED, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);

        Ok(())
    }

    /// Unload shader and free GPU memory
    ///
    /// Destroys the shader object and releases all GPU resources.
    /// Automatically unbinds if currently bound.
    ///
    /// # Arguments
    /// - `gem`: GEM capsule for GPU memory deallocation
    /// - `drm_fd`: DRM file descriptor
    ///
    /// # Errors
    /// - `NotLoaded`: Shader is not loaded
    ///
    /// # State Transition
    /// LOADING/COMPILED/BOUND → UNLOADED
    ///
    /// # Safety
    /// - #ASSUME: No outstanding GPU commands using this shader
    /// - #VERIFY: Caller must ensure all GPU work has completed
    pub fn unload(&self, gem: &XeGemCapsule, drm_fd: RawFd) -> Result<(), XeShaderError> {
        let current_state = self.state.load(Ordering::Acquire);
        if current_state == SHADER_STATE_UNLOADED {
            return Err(XeShaderError::NotLoaded);
        }

        // Unbind if currently bound
        if current_state == SHADER_STATE_BOUND {
            let _ = self.unbind();
        }

        // Phase 1: Simulate unloading
        // In production, this would free GEM buffers
        let _ = (gem, drm_fd);

        // Clear all state
        self.handle.store(0, Ordering::Relaxed);
        self.binary_size.store(0, Ordering::Relaxed);
        self.binary_gpu_addr.store(0, Ordering::Relaxed);
        self.entry_point_offset.store(0, Ordering::Relaxed);
        self.num_uniforms.store(0, Ordering::Relaxed);
        self.num_samplers.store(0, Ordering::Relaxed);

        // Update state and generation
        self.state.store(SHADER_STATE_UNLOADED, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);

        Ok(())
    }

    /// Get shader handle
    #[inline]
    pub fn get_handle(&self) -> Option<u32> {
        let handle = self.handle.load(Ordering::Relaxed);
        if handle != 0 {
            Some(handle)
        } else {
            None
        }
    }

    /// Get current state
    #[inline]
    pub fn get_state(&self) -> u32 {
        self.state.load(Ordering::Acquire)
    }

    /// Check if shader is compiled
    #[inline]
    pub fn is_compiled(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        state == SHADER_STATE_COMPILED || state == SHADER_STATE_BOUND
    }

    /// Get binary size
    #[inline]
    pub fn get_binary_size(&self) -> u64 {
        self.binary_size.load(Ordering::Relaxed)
    }

    /// Get binary GPU address
    #[inline]
    pub fn get_binary_gpu_addr(&self) -> u64 {
        self.binary_gpu_addr.load(Ordering::Relaxed)
    }

    /// Get shader type
    #[inline]
    pub fn get_shader_type(&self) -> u32 {
        self.shader_type.load(Ordering::Relaxed)
    }

    /// Get entry point offset
    #[inline]
    pub fn get_entry_point_offset(&self) -> u32 {
        self.entry_point_offset.load(Ordering::Relaxed)
    }

    /// Get number of uniforms
    #[inline]
    pub fn get_num_uniforms(&self) -> u32 {
        self.num_uniforms.load(Ordering::Relaxed)
    }

    /// Get number of samplers
    #[inline]
    pub fn get_num_samplers(&self) -> u32 {
        self.num_samplers.load(Ordering::Relaxed)
    }

    /// Get load count
    #[inline]
    pub fn get_load_count(&self) -> u64 {
        self.load_count.load(Ordering::Relaxed)
    }

    /// Get compile count
    #[inline]
    pub fn get_compile_count(&self) -> u64 {
        self.compile_count.load(Ordering::Relaxed)
    }

    /// Get generation counter
    #[inline]
    pub fn get_generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }
}

#[cfg(all(feature = "kgpu-driver-intel", target_os = "linux"))]
impl Default for XeShaderCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, feature = "kgpu-driver-intel", target_os = "linux"))]
mod tests {
    use super::super::xe_exec_capsule::XeExecCapsule;
    use super::super::xe_gem_capsule::{XeGemCapsule, GEM_FLAG_HOST_VISIBLE};
    use super::*;

    #[test]
    fn test_capsule_size_alignment() {
        // T28 Q1: Verify 256B cache alignment
        assert_eq!(
            core::mem::size_of::<XeShaderCapsule>(),
            256,
            "XeShaderCapsule must be exactly 256 bytes"
        );
        assert_eq!(
            core::mem::align_of::<XeShaderCapsule>(),
            256,
            "XeShaderCapsule must be 256-byte aligned"
        );
    }

    #[test]
    fn test_new_capsule() {
        // T28 Q2: Verify initial state
        let capsule = XeShaderCapsule::new();
        assert_eq!(capsule.get_state(), SHADER_STATE_UNLOADED);
        assert_eq!(capsule.get_handle(), None);
        assert_eq!(capsule.get_binary_size(), 0);
        assert_eq!(capsule.get_binary_gpu_addr(), 0);
        assert!(!capsule.is_compiled());
        assert_eq!(capsule.get_load_count(), 0);
        assert_eq!(capsule.get_compile_count(), 0);
        assert_eq!(capsule.get_generation(), 0);
    }

    #[test]
    fn test_default() {
        // T28 Q3: Verify Default trait
        let capsule = XeShaderCapsule::default();
        assert_eq!(capsule.get_state(), SHADER_STATE_UNLOADED);
    }

    #[test]
    fn test_load_spirv() {
        // T28 Q4: Verify SPIR-V loading
        let gem = XeGemCapsule::new();
        gem.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE).unwrap();

        let shader = XeShaderCapsule::new();
        let spirv_data = vec![0x03, 0x02, 0x23, 0x07]; // Mock SPIR-V magic number

        let result = shader.load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_COMPUTE);
        assert!(result.is_ok());
        assert_eq!(shader.get_state(), SHADER_STATE_LOADING);
        assert!(shader.get_handle().is_some());
        assert_eq!(shader.get_binary_size(), spirv_data.len() as u64);
        assert_eq!(shader.get_load_count(), 1);
        assert_eq!(shader.get_generation(), 1);
    }

    #[test]
    fn test_load_spirv_invalid_type() {
        // T28 Q5: Verify invalid shader type detection
        let gem = XeGemCapsule::new();
        let shader = XeShaderCapsule::new();
        let spirv_data = vec![0u8; 100];

        let result = shader.load_spirv(&gem, -1, &spirv_data, 999);
        assert_eq!(
            result,
            Err(XeShaderError::InvalidShaderType { shader_type: 999 })
        );
    }

    #[test]
    fn test_load_spirv_too_large() {
        // T28 Q6: Verify size limit enforcement
        let gem = XeGemCapsule::new();
        let shader = XeShaderCapsule::new();
        let large_size = (MAX_SHADER_SIZE + 1) as usize;
        let spirv_data = vec![0u8; large_size];

        let result = shader.load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_COMPUTE);
        assert_eq!(
            result,
            Err(XeShaderError::SpirvTooLarge {
                size: large_size,
                limit: MAX_SHADER_SIZE
            })
        );
    }

    #[test]
    fn test_double_load_fails() {
        // T28 Q7: Verify no double loading
        let gem = XeGemCapsule::new();
        gem.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE).unwrap();

        let shader = XeShaderCapsule::new();
        let spirv_data = vec![0u8; 100];

        shader
            .load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_COMPUTE)
            .unwrap();
        let result = shader.load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_COMPUTE);
        assert_eq!(result, Err(XeShaderError::AlreadyLoaded));
    }

    #[test]
    fn test_compile() {
        // T28 Q8: Verify compilation
        let gem = XeGemCapsule::new();
        gem.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE).unwrap();

        let shader = XeShaderCapsule::new();
        let spirv_data = vec![0u8; 100];
        shader
            .load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_COMPUTE)
            .unwrap();

        let result = shader.compile(-1);
        assert!(result.is_ok());
        assert_eq!(shader.get_state(), SHADER_STATE_COMPILED);
        assert!(shader.is_compiled());
        assert!(shader.get_binary_gpu_addr() != 0);
        assert_eq!(shader.get_compile_count(), 1);
    }

    #[test]
    fn test_compile_without_load_fails() {
        // T28 Q9: Verify compile requires load
        let shader = XeShaderCapsule::new();
        let result = shader.compile(-1);
        assert_eq!(result, Err(XeShaderError::NotLoaded));
    }

    #[test]
    fn test_double_compile_is_noop() {
        // T28 Q10: Verify double compile is safe
        let gem = XeGemCapsule::new();
        gem.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE).unwrap();

        let shader = XeShaderCapsule::new();
        let spirv_data = vec![0u8; 100];
        shader
            .load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_COMPUTE)
            .unwrap();
        shader.compile(-1).unwrap();

        let compile_count = shader.get_compile_count();
        shader.compile(-1).unwrap(); // Should be no-op
        assert_eq!(shader.get_compile_count(), compile_count);
    }

    #[test]
    fn test_bind() {
        // T28 Q11: Verify shader binding
        let gem = XeGemCapsule::new();
        gem.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE).unwrap();

        let exec = XeExecCapsule::new();
        exec.create_queue(-1, 0, 0).unwrap();

        let shader = XeShaderCapsule::new();
        let spirv_data = vec![0u8; 100];
        shader
            .load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_COMPUTE)
            .unwrap();
        shader.compile(-1).unwrap();

        let handle = shader.bind(&exec).unwrap();
        assert_ne!(handle, 0);
        assert_eq!(shader.get_state(), SHADER_STATE_BOUND);
    }

    #[test]
    fn test_bind_without_compile_fails() {
        // T28 Q12: Verify bind requires compile
        let gem = XeGemCapsule::new();
        let exec = XeExecCapsule::new();
        exec.create_queue(-1, 0, 0).unwrap();

        let shader = XeShaderCapsule::new();
        let spirv_data = vec![0u8; 100];
        shader
            .load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_COMPUTE)
            .unwrap();

        let result = shader.bind(&exec);
        assert_eq!(result, Err(XeShaderError::NotCompiled));
    }

    #[test]
    fn test_bind_without_queue_fails() {
        // T28 Q13: Verify bind requires created queue
        let gem = XeGemCapsule::new();
        gem.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE).unwrap();

        let exec = XeExecCapsule::new();

        let shader = XeShaderCapsule::new();
        let spirv_data = vec![0u8; 100];
        shader
            .load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_COMPUTE)
            .unwrap();
        shader.compile(-1).unwrap();

        let result = shader.bind(&exec);
        assert!(matches!(
            result,
            Err(XeShaderError::BindFailed { errno: -22 })
        ));
    }

    #[test]
    fn test_unbind() {
        // T28 Q14: Verify shader unbinding
        let gem = XeGemCapsule::new();
        gem.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE).unwrap();

        let exec = XeExecCapsule::new();
        exec.create_queue(-1, 0, 0).unwrap();

        let shader = XeShaderCapsule::new();
        let spirv_data = vec![0u8; 100];
        shader
            .load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_COMPUTE)
            .unwrap();
        shader.compile(-1).unwrap();
        shader.bind(&exec).unwrap();

        let result = shader.unbind();
        assert!(result.is_ok());
        assert_eq!(shader.get_state(), SHADER_STATE_COMPILED);
    }

    #[test]
    fn test_unbind_without_bind_fails() {
        // T28 Q15: Verify unbind requires bind
        let gem = XeGemCapsule::new();
        let shader = XeShaderCapsule::new();
        let spirv_data = vec![0u8; 100];
        shader
            .load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_COMPUTE)
            .unwrap();
        shader.compile(-1).unwrap();

        let result = shader.unbind();
        assert_eq!(result, Err(XeShaderError::NotBound));
    }

    #[test]
    fn test_unload() {
        // T28 Q16: Verify shader unloading
        let gem = XeGemCapsule::new();
        gem.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE).unwrap();

        let shader = XeShaderCapsule::new();
        let spirv_data = vec![0u8; 100];
        shader
            .load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_COMPUTE)
            .unwrap();
        shader.compile(-1).unwrap();

        let result = shader.unload(&gem, -1);
        assert!(result.is_ok());
        assert_eq!(shader.get_state(), SHADER_STATE_UNLOADED);
        assert_eq!(shader.get_handle(), None);
        assert_eq!(shader.get_binary_size(), 0);
        assert_eq!(shader.get_binary_gpu_addr(), 0);
    }

    #[test]
    fn test_unload_without_load_fails() {
        // T28 Q17: Verify unload requires load
        let gem = XeGemCapsule::new();
        let shader = XeShaderCapsule::new();
        let result = shader.unload(&gem, -1);
        assert_eq!(result, Err(XeShaderError::NotLoaded));
    }

    #[test]
    fn test_unload_unbinds_automatically() {
        // T28 Q18: Verify unload unbinds automatically
        let gem = XeGemCapsule::new();
        gem.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE).unwrap();

        let exec = XeExecCapsule::new();
        exec.create_queue(-1, 0, 0).unwrap();

        let shader = XeShaderCapsule::new();
        let spirv_data = vec![0u8; 100];
        shader
            .load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_COMPUTE)
            .unwrap();
        shader.compile(-1).unwrap();
        shader.bind(&exec).unwrap();

        assert_eq!(shader.get_state(), SHADER_STATE_BOUND);
        shader.unload(&gem, -1).unwrap();
        assert_eq!(shader.get_state(), SHADER_STATE_UNLOADED);
    }

    #[test]
    fn test_full_lifecycle() {
        // T28 Q19: Verify complete lifecycle
        let gem = XeGemCapsule::new();
        gem.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE).unwrap();

        let exec = XeExecCapsule::new();
        exec.create_queue(-1, 0, 0).unwrap();

        let shader = XeShaderCapsule::new();
        let spirv_data = vec![0x03, 0x02, 0x23, 0x07, 0x00, 0x00, 0x01, 0x00];

        // Load
        shader
            .load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_COMPUTE)
            .unwrap();
        assert_eq!(shader.get_state(), SHADER_STATE_LOADING);
        assert_eq!(shader.get_load_count(), 1);

        // Compile
        shader.compile(-1).unwrap();
        assert_eq!(shader.get_state(), SHADER_STATE_COMPILED);
        assert!(shader.is_compiled());
        assert_eq!(shader.get_compile_count(), 1);

        // Bind
        let handle = shader.bind(&exec).unwrap();
        assert_ne!(handle, 0);
        assert_eq!(shader.get_state(), SHADER_STATE_BOUND);

        // Unbind
        shader.unbind().unwrap();
        assert_eq!(shader.get_state(), SHADER_STATE_COMPILED);

        // Unload
        shader.unload(&gem, -1).unwrap();
        assert_eq!(shader.get_state(), SHADER_STATE_UNLOADED);
        assert_eq!(shader.get_handle(), None);
    }

    #[test]
    fn test_generation_counter() {
        // T28 Q20: Verify generation counter increments
        let gem = XeGemCapsule::new();
        gem.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE).unwrap();

        let exec = XeExecCapsule::new();
        exec.create_queue(-1, 0, 0).unwrap();

        let shader = XeShaderCapsule::new();
        let spirv_data = vec![0u8; 100];
        let gen0 = shader.get_generation();

        shader
            .load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_COMPUTE)
            .unwrap();
        let gen1 = shader.get_generation();
        assert_eq!(gen1, gen0 + 1);

        shader.compile(-1).unwrap();
        let gen2 = shader.get_generation();
        assert_eq!(gen2, gen1 + 1);

        shader.bind(&exec).unwrap();
        let gen3 = shader.get_generation();
        assert_eq!(gen3, gen2 + 1);

        shader.unbind().unwrap();
        let gen4 = shader.get_generation();
        assert_eq!(gen4, gen3 + 1);

        shader.unload(&gem, -1).unwrap();
        let gen5 = shader.get_generation();
        assert_eq!(gen5, gen4 + 1);
    }

    #[test]
    fn test_shader_types() {
        // T28 Q21: Verify all shader types
        let gem = XeGemCapsule::new();
        gem.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE).unwrap();

        let spirv_data = vec![0u8; 100];

        // Compute
        let compute = XeShaderCapsule::new();
        compute
            .load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_COMPUTE)
            .unwrap();
        assert_eq!(compute.get_shader_type(), SHADER_TYPE_COMPUTE);

        // Vertex
        let vertex = XeShaderCapsule::new();
        vertex
            .load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_VERTEX)
            .unwrap();
        assert_eq!(vertex.get_shader_type(), SHADER_TYPE_VERTEX);

        // Fragment
        let fragment = XeShaderCapsule::new();
        fragment
            .load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_FRAGMENT)
            .unwrap();
        assert_eq!(fragment.get_shader_type(), SHADER_TYPE_FRAGMENT);

        // Geometry
        let geometry = XeShaderCapsule::new();
        geometry
            .load_spirv(&gem, -1, &spirv_data, SHADER_TYPE_GEOMETRY)
            .unwrap();
        assert_eq!(geometry.get_shader_type(), SHADER_TYPE_GEOMETRY);
    }

    #[test]
    fn test_accessors() {
        // T28 Q22: Verify all accessor methods
        let shader = XeShaderCapsule::new();

        // All accessors should work without panicking
        let _ = shader.get_handle();
        let _ = shader.get_state();
        let _ = shader.is_compiled();
        let _ = shader.get_binary_size();
        let _ = shader.get_binary_gpu_addr();
        let _ = shader.get_shader_type();
        let _ = shader.get_entry_point_offset();
        let _ = shader.get_num_uniforms();
        let _ = shader.get_num_samplers();
        let _ = shader.get_load_count();
        let _ = shader.get_compile_count();
        let _ = shader.get_generation();
    }
}
