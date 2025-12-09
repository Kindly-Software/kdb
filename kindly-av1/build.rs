//! Build script for kindly-av1
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Compiles GLSL compute shaders to SPIR-V for Vulkan backend.
//! Compiles HIP kernels for ROCm backend.
//! Embeds Ed25519 public key for offline license verification.
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q11 100% Rust implementation (shaderc-rs is Rust wrapper)
//! - **Chaos**: Zero runtime overhead (build-time compilation)
//! - **ASSUM**: Shader compilation failures caught at build time
//! - **Q34**: License key embedding for offline verification

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=kernels/motion_estimation.comp");
    println!("cargo:rerun-if-changed=kernels/motion_estimation.hip");
    println!("cargo:rerun-if-changed=kernels/sgemm.comp");
    println!("cargo:rerun-if-changed=keys/public_key.bin");

    // Embed Ed25519 public key for license verification
    embed_license_public_key();

    // Compile Vulkan shaders if enabled
    #[cfg(feature = "gpu-vulkan")]
    compile_shaders();

    // Compile HIP kernels if enabled
    #[cfg(feature = "gpu-rocm")]
    compile_hip_kernels();
}

#[cfg(feature = "gpu-vulkan")]
fn compile_shaders() {
    use shaderc::{CompileOptions, Compiler, ShaderKind};

    // Initialize shader compiler
    // #ASSUME_SHADERC_AVAILABLE: shaderc library installed on build system
    // #VERIFY_COMPILER: Compiler initialization succeeds
    let mut compiler = match Compiler::new() {
        Some(c) => c,
        None => {
            eprintln!("WARNING: shaderc not available, skipping SPIR-V compilation");
            eprintln!("Install shaderc library or use cpu-only build");
            return;
        }
    };

    let mut options = CompileOptions::new().expect("Failed to create compile options");

    // Enable Vulkan 1.2 target environment
    options.set_target_env(
        shaderc::TargetEnv::Vulkan,
        shaderc::EnvVersion::Vulkan1_2 as u32,
    );

    // Set GLSL source language version (450 = GLSL 4.50)
    options.set_source_language(shaderc::SourceLanguage::GLSL);

    // Add global macro to fix array size issues
    // GL_EXT_scalar_block_layout enables runtime-sized arrays in SSBO
    options.add_macro_definition("GL_EXT_scalar_block_layout", Some("1"));

    // Enable optimization for release builds
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    if profile == "release" {
        options.set_optimization_level(shaderc::OptimizationLevel::Performance);
    }

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");

    // Compile motion estimation compute shader
    compile_shader(
        &mut compiler,
        &options,
        "kernels/motion_estimation.comp",
        "kernels/motion_estimation.spv",
        ShaderKind::Compute,
    );

    // Compile SGEMM compute shader for benchmarks
    compile_shader(
        &mut compiler,
        &options,
        "kernels/sgemm.comp",
        &format!("{}/sgemm.spv", out_dir),
        ShaderKind::Compute,
    );

    println!("cargo:warning=SPIR-V shader compilation complete");
}

#[cfg(feature = "gpu-vulkan")]
fn compile_shader(
    compiler: &mut shaderc::Compiler,
    options: &shaderc::CompileOptions,
    source_path: &str,
    output_path: &str,
    shader_kind: shaderc::ShaderKind,
) {
    use std::io::Write;

    println!("Compiling shader: {} -> {}", source_path, output_path);

    // Read shader source
    let source = fs::read_to_string(source_path)
        .unwrap_or_else(|e| panic!("Failed to read shader {}: {}", source_path, e));

    // Compile to SPIR-V
    // #ASSUME_SHADER_VALID: GLSL shader is syntactically correct
    // #VERIFY_COMPILATION: Compilation succeeds or panics
    let binary_result =
        compiler.compile_into_spirv(&source, shader_kind, source_path, "main", Some(options));

    let binary = match binary_result {
        Ok(b) => b,
        Err(e) => {
            panic!("Shader compilation failed for {}:\n{}", source_path, e);
        }
    };

    // Check for warnings
    let num_warnings = binary.get_num_warnings();
    if num_warnings > 0 {
        println!(
            "cargo:warning=Shader {} compiled with {} warnings",
            source_path, num_warnings
        );
        println!("cargo:warning={}", binary.get_warning_messages());
    }

    // Write SPIR-V binary to file
    let spirv_bytes = binary.as_binary_u8();
    let mut file = fs::File::create(output_path)
        .unwrap_or_else(|e| panic!("Failed to create output file {}: {}", output_path, e));

    file.write_all(spirv_bytes)
        .unwrap_or_else(|e| panic!("Failed to write SPIR-V binary {}: {}", output_path, e));

    println!(
        "cargo:warning=Compiled {} to {} ({} bytes, {} words)",
        source_path,
        output_path,
        spirv_bytes.len(),
        binary.as_binary().len()
    );

    // Generate Rust const array for embedding (optional optimization)
    generate_spirv_constant(output_path, spirv_bytes);
}

#[cfg(feature = "gpu-vulkan")]
fn generate_spirv_constant(spirv_path: &str, spirv_bytes: &[u8]) {
    use std::io::Write;

    // Generate Rust source file with embedded SPIR-V
    let rs_path = spirv_path.replace(".spv", "_spirv.rs");
    let const_name = Path::new(spirv_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .expect("Invalid SPIR-V filename")
        .to_uppercase()
        .replace('-', "_");

    let mut rs_file = fs::File::create(&rs_path)
        .unwrap_or_else(|e| panic!("Failed to create Rust constant file {}: {}", rs_path, e));

    writeln!(
        rs_file,
        "//! Auto-generated SPIR-V shader constant\n\
         //!\n\
         //! Generated from: {}\n\
         //! DO NOT EDIT MANUALLY\n\
         \n\
         /// SPIR-V bytecode for {} shader ({} bytes)\n\
         #[allow(dead_code)]\n\
         pub const {}_SPIRV: &[u8] = &[",
        spirv_path,
        spirv_path,
        spirv_bytes.len(),
        const_name
    )
    .expect("Failed to write header");

    // Write bytes in rows of 16 for readability
    for (i, byte) in spirv_bytes.iter().enumerate() {
        if i % 16 == 0 {
            write!(rs_file, "\n    ").expect("Failed to write indent");
        }
        write!(rs_file, "0x{:02x}, ", byte).expect("Failed to write byte");
    }

    writeln!(rs_file, "\n];\n").expect("Failed to write footer");

    println!("cargo:warning=Generated Rust constant: {}", rs_path);
}

// =============================================================================
// HIP Kernel Compilation (ROCm Backend)
// =============================================================================

#[cfg(feature = "gpu-rocm")]
fn compile_hip_kernels() {
    // Emit linker search path for ROCm libraries
    // #ASSUME_ROCM_INSTALLED: ROCm toolkit installed at /opt/rocm
    // #VERIFY_LIB_PATH: libamdhip64.so exists in /opt/rocm/lib
    println!("cargo:rustc-link-search=native=/opt/rocm/lib");

    // Attempt to find hipcc compiler
    let hipcc_path = match find_hipcc() {
        Some(path) => path,
        None => {
            println!("cargo:warning=hipcc not found, skipping HIP kernel compilation");
            println!("cargo:warning=Install ROCm toolkit or use cpu-only/gpu-vulkan build");
            return;
        }
    };

    println!("cargo:warning=Found hipcc: {}", hipcc_path.display());

    // Compile motion estimation kernel
    match compile_hip_kernel(
        &hipcc_path,
        "kernels/motion_estimation.hip",
        "motion_estimation",
    ) {
        Ok(kernel_path) => {
            println!("cargo:warning=HIP kernel compilation successful");
            println!("cargo:rustc-env=HIP_KERNEL_PATH={}", kernel_path);

            // Generate Rust constant file for kernel path
            generate_hip_kernel_path_constant(&kernel_path);
        }
        Err(e) => {
            println!("cargo:warning=HIP kernel compilation failed: {}", e);
            println!("cargo:warning=Encoder will fall back to CPU implementation");
        }
    }
}

#[cfg(feature = "gpu-rocm")]
fn find_hipcc() -> Option<PathBuf> {
    // Check standard ROCm installation paths
    let rocm_paths = [
        "/opt/rocm/bin/hipcc",
        "/opt/rocm-6.0.0/bin/hipcc",
        "/opt/rocm-5.7.0/bin/hipcc",
        "/usr/bin/hipcc",
    ];

    for path_str in &rocm_paths {
        let path = PathBuf::from(path_str);
        if path.exists() && path.is_file() {
            return Some(path);
        }
    }

    // Check PATH environment variable
    if let Ok(path_env) = env::var("PATH") {
        for dir in path_env.split(':') {
            let hipcc_path = PathBuf::from(dir).join("hipcc");
            if hipcc_path.exists() && hipcc_path.is_file() {
                return Some(hipcc_path);
            }
        }
    }

    None
}

#[cfg(feature = "gpu-rocm")]
fn compile_hip_kernel(hipcc: &Path, kernel_src: &str, kernel_name: &str) -> Result<String, String> {
    use std::io::Write;

    println!("Compiling HIP kernel: {}", kernel_src);

    // Get output directory
    let out_dir = env::var("OUT_DIR").map_err(|e| format!("Failed to get OUT_DIR: {}", e))?;

    let kernel_out = format!("{}/{}.co", out_dir, kernel_name);

    // Determine optimization level based on profile
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let opt_level = if profile == "release" { "-O3" } else { "-O0" };

    // Build hipcc command with multiple GPU targets
    // #ASSUME_AMDGPU_TARGETS: Targeting common AMD GPUs
    // #VERIFY_COMPILATION: hipcc exits with status 0
    let mut cmd = Command::new(hipcc);
    cmd.args(&[
        "--genco",                 // Generate code object
        opt_level,                 // Optimization level
        "--amdgpu-target=gfx1035", // AMD 680M (Ryzen 6000 series integrated)
        "--amdgpu-target=gfx1030", // RX 6800/6900 XT
        "--amdgpu-target=gfx1100", // RX 7900 XTX
        "--amdgpu-target=gfx906",  // Radeon VII, MI50
        "-o",
        &kernel_out,
        kernel_src,
    ]);

    println!("Running: {:?}", cmd);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute hipcc: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("hipcc compilation failed:\n{}", stderr));
    }

    // Check for warnings
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        println!("cargo:warning=hipcc warnings:\n{}", stderr);
    }
    if !stdout.is_empty() {
        println!("cargo:warning=hipcc output:\n{}", stdout);
    }

    // Verify output file was created
    let kernel_path = Path::new(&kernel_out);
    if !kernel_path.exists() {
        return Err(format!("Kernel code object not created: {}", kernel_out));
    }

    let file_size = kernel_path.metadata().map(|m| m.len()).unwrap_or(0);

    println!(
        "cargo:warning=Compiled {} to {} ({} bytes)",
        kernel_src, kernel_out, file_size
    );

    Ok(kernel_out)
}

#[cfg(feature = "gpu-rocm")]
fn generate_hip_kernel_path_constant(kernel_path: &str) {
    use std::io::Write;

    // Get output directory
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");

    let rs_path = format!("{}/hip_kernel_path.rs", out_dir);

    let mut rs_file = fs::File::create(&rs_path)
        .unwrap_or_else(|e| panic!("Failed to create Rust constant file {}: {}", rs_path, e));

    writeln!(
        rs_file,
        "//! Auto-generated HIP kernel path constant\n\
         //!\n\
         //! Generated from HIP kernel compilation\n\
         //! DO NOT EDIT MANUALLY\n\
         \n\
         /// Absolute path to compiled HIP kernel code object (.co file)\n\
         ///\n\
         /// This path is set at build time by build.rs and points to the\n\
         /// compiled HIP kernel in the OUT_DIR.\n\
         ///\n\
         /// # Framework Compliance\n\
         ///\n\
         /// - **UCE34**: Q11 Build-time kernel compilation\n\
         /// - **Chaos**: Zero runtime overhead (compile-time constant)\n\
         /// - **ASSUM**: Path validity verified at build time\n\
         #[allow(dead_code)]\n\
         pub const HIP_KERNEL_PATH: &str = {:?};\n",
        kernel_path
    )
    .expect("Failed to write HIP kernel path constant");

    println!(
        "cargo:warning=Generated HIP kernel path constant: {}",
        rs_path
    );
}

// =============================================================================
// Ed25519 License Public Key Embedding
// =============================================================================

/// Embed Ed25519 public key for offline license verification
///
/// # Key Sources (in priority order)
///
/// 1. `keys/public_key.bin` - Generated by keygen tool
/// 2. Development fallback - Zero key with warning (debug only)
///
/// # Release Mode Enforcement
///
/// In release mode, build FAILS if public_key.bin is missing.
/// This prevents shipping binaries without license verification.
///
/// # Framework Compliance
///
/// - **UCE34 Q34**: Audit-compliant license verification
/// - **Chaos**: Compile-time embedding (zero runtime overhead)
/// - **ASSUM**: Key validity verified at build time
fn embed_license_public_key() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());

    // Look for public key file
    let key_path = PathBuf::from(&manifest_dir).join("keys/public_key.bin");

    let public_key_bytes: [u8; 32] = if key_path.exists() {
        // Read key from file
        let bytes = fs::read(&key_path)
            .unwrap_or_else(|e| panic!("Failed to read public key from {}: {}", key_path.display(), e));

        if bytes.len() != 32 {
            panic!(
                "Invalid public key size: expected 32 bytes, got {} bytes in {}",
                bytes.len(),
                key_path.display()
            );
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);

        println!("cargo:warning=Embedded Ed25519 public key from {}", key_path.display());
        key
    } else if profile == "release" {
        // Release mode: FAIL if key missing
        panic!(
            "RELEASE BUILD ERROR: Ed25519 public key not found!\n\
             \n\
             License verification requires a public key.\n\
             \n\
             To generate the keypair:\n\
             \n\
             1. cd tools/keygen\n\
             2. cargo run --release\n\
             \n\
             This creates:\n\
             - keys/signing_key.bin (PRIVATE - server only)\n\
             - keys/public_key.bin (embedded in binary)\n\
             \n\
             Expected location: {}\n",
            key_path.display()
        );
    } else {
        // Debug mode: Use development placeholder with warning
        println!("cargo:warning=================================");
        println!("cargo:warning=  DEVELOPMENT MODE: Using placeholder license key");
        println!("cargo:warning=  Generate real key with: cd tools/keygen && cargo run");
        println!("cargo:warning=================================");

        // Development key (all zeros - will only work with dev-signed licenses)
        [0u8; 32]
    };

    // Generate Rust source file
    let rs_path = PathBuf::from(&out_dir).join("license_public_key.rs");
    let mut file = fs::File::create(&rs_path)
        .unwrap_or_else(|e| panic!("Failed to create {}: {}", rs_path.display(), e));

    // Note: Using regular comments (//) instead of doc comments (//!) because
    // this file is included via include!() macro, not as a module
    writeln!(file, "// Auto-generated Ed25519 public key for license verification").unwrap();
    writeln!(file, "//").unwrap();
    writeln!(file, "// Generated by build.rs from keys/public_key.bin").unwrap();
    writeln!(file, "// DO NOT EDIT MANUALLY").unwrap();
    writeln!(file, "//").unwrap();
    writeln!(file, "// Security:").unwrap();
    writeln!(file, "// This key is used to verify Ed25519 signatures on license files.").unwrap();
    writeln!(file, "// The corresponding private key is held by the activation server.").unwrap();
    writeln!(file, "//").unwrap();
    writeln!(file, "// Framework Compliance:").unwrap();
    writeln!(file, "// - UCE34 Q34: Audit-compliant license verification").unwrap();
    writeln!(file, "// - Chaos: Compile-time constant (0ns runtime overhead)").unwrap();
    writeln!(file).unwrap();
    writeln!(file, "/// Ed25519 public key for offline license verification (32 bytes)").unwrap();
    writeln!(file, "///").unwrap();
    writeln!(file, "/// Used by `GumroadLicenseCapsule` to verify license signatures.").unwrap();
    writeln!(file, "/// Invalid signatures indicate license tampering or file corruption.").unwrap();
    writeln!(file, "pub const ED25519_PUBLIC_KEY: [u8; 32] = [").unwrap();

    // Write bytes in rows of 8 for readability
    for (i, byte) in public_key_bytes.iter().enumerate() {
        if i % 8 == 0 {
            write!(file, "    ").unwrap();
        }
        write!(file, "0x{:02x}, ", byte).unwrap();
        if i % 8 == 7 {
            writeln!(file).unwrap();
        }
    }

    writeln!(file, "];").unwrap();

    // Also generate a flag indicating if this is a development key
    let is_dev_key = public_key_bytes == [0u8; 32];
    writeln!(file).unwrap();
    writeln!(file, "/// True if using development placeholder key (debug builds only)").unwrap();
    writeln!(file, "pub const IS_DEVELOPMENT_KEY: bool = {};", is_dev_key).unwrap();

    file.sync_all().unwrap();

    println!("cargo:warning=Generated license public key constant: {}", rs_path.display());
}
