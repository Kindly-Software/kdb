//! Output Validator - Validate kdb output against expected values or GDB baseline
//!
//! Provides validation utilities for comparing kdb behavior against expected
//! results or GDB as a reference implementation.
//!
//! # ASSUM Safety
//!
//! - #ASSUME_TOLERANCE_VALID: Tolerance values are within reasonable bounds
//! - #ASSUME_REGISTER_SUBSET: Not all registers may be available on all platforms
//! - #ASSUME_ADDRESS_STABLE: Addresses may vary between runs (ASLR)

use super::debugger_driver::{Registers, StackFrame};
use super::error::{E2EError, E2EResult};
use super::gdb_driver::{GdbRegisters, GdbStackFrame};
use std::collections::HashSet;

/// Comparison result with details
#[derive(Debug, Clone)]
pub struct ComparisonResult {
    /// Whether the comparison passed
    pub passed: bool,
    /// List of differences found (empty if passed)
    pub differences: Vec<String>,
    /// Summary message
    pub summary: String,
}

impl ComparisonResult {
    /// Create a passing result
    pub fn pass(summary: impl Into<String>) -> Self {
        Self {
            passed: true,
            differences: Vec::new(),
            summary: summary.into(),
        }
    }

    /// Create a failing result with differences
    pub fn fail(summary: impl Into<String>, differences: Vec<String>) -> Self {
        Self {
            passed: false,
            differences,
            summary: summary.into(),
        }
    }

    /// Convert to E2EResult (Err if failed)
    pub fn into_result(self) -> E2EResult<()> {
        if self.passed {
            Ok(())
        } else {
            Err(E2EError::ValidationMismatch {
                expected: self.summary.clone(),
                actual: self.differences.join("; "),
            })
        }
    }
}

/// Configuration for validation tolerances
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Allow address differences (for ASLR)
    pub allow_address_variance: bool,
    /// Maximum allowed address offset (for relative comparisons)
    pub max_address_offset: u64,
    /// Registers to ignore in comparisons
    pub ignored_registers: HashSet<String>,
    /// Allow missing frames in stack traces
    pub allow_missing_frames: bool,
    /// Maximum allowed stack depth difference
    pub max_stack_depth_difference: usize,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            allow_address_variance: true,
            max_address_offset: 0x1000, // 4KB offset tolerance
            ignored_registers: HashSet::new(),
            allow_missing_frames: true,
            max_stack_depth_difference: 2,
        }
    }
}

impl ValidationConfig {
    /// Create a strict configuration (no tolerances)
    pub fn strict() -> Self {
        Self {
            allow_address_variance: false,
            max_address_offset: 0,
            ignored_registers: HashSet::new(),
            allow_missing_frames: false,
            max_stack_depth_difference: 0,
        }
    }

    /// Create a lenient configuration (maximum tolerance)
    pub fn lenient() -> Self {
        Self {
            allow_address_variance: true,
            max_address_offset: 0x10000, // 64KB
            ignored_registers: ["rflags", "fs", "gs", "cs", "ss", "ds", "es"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            allow_missing_frames: true,
            max_stack_depth_difference: 5,
        }
    }

    /// Add a register to ignore
    pub fn ignore_register(mut self, name: &str) -> Self {
        self.ignored_registers.insert(name.to_string());
        self
    }
}

/// Validate kdb output against expected values and GDB baselines
///
/// This validator provides comprehensive comparison utilities for E2E testing,
/// supporting both exact matching and tolerant comparisons.
///
/// # Example
///
/// ```ignore
/// let validator = OutputValidator::new();
///
/// // Compare registers
/// let result = validator.compare_registers(&kdb_regs, &gdb_regs);
/// result.into_result()?;
///
/// // Validate stack trace
/// let result = validator.validate_stack_trace(&frames, 5);
/// result.into_result()?;
/// ```
#[derive(Debug, Clone)]
pub struct OutputValidator {
    /// Validation configuration
    config: ValidationConfig,
}

impl OutputValidator {
    /// Create a new validator with default configuration
    pub fn new() -> Self {
        Self {
            config: ValidationConfig::default(),
        }
    }

    /// Create a validator with custom configuration
    pub fn with_config(config: ValidationConfig) -> Self {
        Self { config }
    }

    /// Get the current configuration
    pub fn config(&self) -> &ValidationConfig {
        &self.config
    }

    /// Set the configuration
    pub fn set_config(&mut self, config: ValidationConfig) {
        self.config = config;
    }

    // =========================================================================
    // Register Comparisons
    // =========================================================================

    /// Compare kdb registers against GDB registers
    ///
    /// # Arguments
    ///
    /// * `kdb_regs` - Registers from kdb
    /// * `gdb_regs` - Registers from GDB
    ///
    /// # Returns
    ///
    /// ComparisonResult with differences (if any)
    pub fn compare_registers(
        &self,
        kdb_regs: &Registers,
        gdb_regs: &GdbRegisters,
    ) -> ComparisonResult {
        let mut differences = Vec::new();

        // Compare instruction pointer
        if !self.config.ignored_registers.contains("rip") {
            if let Err(diff) = self.compare_address("rip", kdb_regs.rip, gdb_regs.rip) {
                differences.push(diff);
            }
        }

        // Compare stack pointer
        if !self.config.ignored_registers.contains("rsp") {
            if let Err(diff) = self.compare_address("rsp", kdb_regs.rsp, gdb_regs.rsp) {
                differences.push(diff);
            }
        }

        // Compare base pointer
        if !self.config.ignored_registers.contains("rbp") {
            if let Err(diff) = self.compare_address("rbp", kdb_regs.rbp, gdb_regs.rbp) {
                differences.push(diff);
            }
        }

        // Compare general purpose registers
        let gp_regs = [
            ("rax", kdb_regs.rax, gdb_regs.rax),
            ("rbx", kdb_regs.rbx, gdb_regs.values.get("rbx").copied().unwrap_or(0)),
            ("rcx", kdb_regs.rcx, gdb_regs.values.get("rcx").copied().unwrap_or(0)),
            ("rdx", kdb_regs.rdx, gdb_regs.values.get("rdx").copied().unwrap_or(0)),
            ("rsi", kdb_regs.rsi, gdb_regs.values.get("rsi").copied().unwrap_or(0)),
            ("rdi", kdb_regs.rdi, gdb_regs.values.get("rdi").copied().unwrap_or(0)),
            ("r8", kdb_regs.r8, gdb_regs.values.get("r8").copied().unwrap_or(0)),
            ("r9", kdb_regs.r9, gdb_regs.values.get("r9").copied().unwrap_or(0)),
            ("r10", kdb_regs.r10, gdb_regs.values.get("r10").copied().unwrap_or(0)),
            ("r11", kdb_regs.r11, gdb_regs.values.get("r11").copied().unwrap_or(0)),
            ("r12", kdb_regs.r12, gdb_regs.values.get("r12").copied().unwrap_or(0)),
            ("r13", kdb_regs.r13, gdb_regs.values.get("r13").copied().unwrap_or(0)),
            ("r14", kdb_regs.r14, gdb_regs.values.get("r14").copied().unwrap_or(0)),
            ("r15", kdb_regs.r15, gdb_regs.values.get("r15").copied().unwrap_or(0)),
        ];

        for (name, kdb_val, gdb_val) in gp_regs {
            if self.config.ignored_registers.contains(name) {
                continue;
            }
            if kdb_val != gdb_val {
                differences.push(format!(
                    "{}: kdb=0x{:016x}, gdb=0x{:016x}",
                    name, kdb_val, gdb_val
                ));
            }
        }

        if differences.is_empty() {
            ComparisonResult::pass("All registers match")
        } else {
            ComparisonResult::fail(
                format!("{} register differences", differences.len()),
                differences,
            )
        }
    }

    /// Compare addresses with optional tolerance
    fn compare_address(&self, name: &str, kdb_addr: u64, gdb_addr: u64) -> Result<(), String> {
        if kdb_addr == gdb_addr {
            return Ok(());
        }

        if self.config.allow_address_variance {
            let diff = if kdb_addr > gdb_addr {
                kdb_addr - gdb_addr
            } else {
                gdb_addr - kdb_addr
            };

            if diff <= self.config.max_address_offset {
                return Ok(());
            }
        }

        Err(format!(
            "{}: kdb=0x{:016x}, gdb=0x{:016x}",
            name, kdb_addr, gdb_addr
        ))
    }

    /// Validate that a specific register has an expected value
    pub fn validate_register(
        &self,
        regs: &Registers,
        name: &str,
        expected: u64,
    ) -> ComparisonResult {
        let actual = match name.to_lowercase().as_str() {
            "rip" => regs.rip,
            "rsp" => regs.rsp,
            "rbp" => regs.rbp,
            "rax" => regs.rax,
            "rbx" => regs.rbx,
            "rcx" => regs.rcx,
            "rdx" => regs.rdx,
            "rsi" => regs.rsi,
            "rdi" => regs.rdi,
            "r8" => regs.r8,
            "r9" => regs.r9,
            "r10" => regs.r10,
            "r11" => regs.r11,
            "r12" => regs.r12,
            "r13" => regs.r13,
            "r14" => regs.r14,
            "r15" => regs.r15,
            "rflags" => regs.rflags,
            _ => {
                return ComparisonResult::fail(
                    format!("Unknown register: {}", name),
                    vec![format!("Register '{}' not found", name)],
                )
            }
        };

        if actual == expected {
            ComparisonResult::pass(format!("{} = 0x{:x}", name, actual))
        } else {
            ComparisonResult::fail(
                format!("{} mismatch", name),
                vec![format!(
                    "{}: expected=0x{:016x}, actual=0x{:016x}",
                    name, expected, actual
                )],
            )
        }
    }

    // =========================================================================
    // Stack Trace Comparisons
    // =========================================================================

    /// Compare kdb stack trace against GDB stack trace
    pub fn compare_stack_traces(
        &self,
        kdb_frames: &[StackFrame],
        gdb_frames: &[GdbStackFrame],
    ) -> ComparisonResult {
        let mut differences = Vec::new();

        // Check frame count difference
        let count_diff = if kdb_frames.len() > gdb_frames.len() {
            kdb_frames.len() - gdb_frames.len()
        } else {
            gdb_frames.len() - kdb_frames.len()
        };

        if count_diff > self.config.max_stack_depth_difference {
            differences.push(format!(
                "Frame count: kdb={}, gdb={} (diff={})",
                kdb_frames.len(),
                gdb_frames.len(),
                count_diff
            ));
        }

        // Compare individual frames
        let min_frames = kdb_frames.len().min(gdb_frames.len());
        for i in 0..min_frames {
            let kdb = &kdb_frames[i];
            let gdb = &gdb_frames[i];

            // Compare addresses
            if let Err(diff) =
                self.compare_address(&format!("frame[{}].rip", i), kdb.rip, gdb.addr)
            {
                differences.push(diff);
            }

            // Compare function names (if available)
            if let (Some(kdb_func), Some(gdb_func)) = (&kdb.function_name, &gdb.func) {
                if kdb_func != gdb_func {
                    differences.push(format!(
                        "frame[{}].func: kdb='{}', gdb='{}'",
                        i, kdb_func, gdb_func
                    ));
                }
            }
        }

        // Note missing frames
        if self.config.allow_missing_frames {
            if kdb_frames.len() > gdb_frames.len() {
                for i in min_frames..kdb_frames.len() {
                    // Just note, don't fail
                    differences.push(format!(
                        "frame[{}]: kdb has extra frame at 0x{:x}",
                        i, kdb_frames[i].rip
                    ));
                }
            } else if gdb_frames.len() > kdb_frames.len() {
                for i in min_frames..gdb_frames.len() {
                    differences.push(format!(
                        "frame[{}]: gdb has extra frame at 0x{:x}",
                        i, gdb_frames[i].addr
                    ));
                }
            }
        }

        if differences.is_empty() {
            ComparisonResult::pass(format!("Stack traces match ({} frames)", min_frames))
        } else {
            ComparisonResult::fail(
                format!("{} stack trace differences", differences.len()),
                differences,
            )
        }
    }

    /// Validate stack trace against expected properties
    ///
    /// # Arguments
    ///
    /// * `frames` - Stack frames from kdb
    /// * `min_depth` - Minimum expected stack depth
    pub fn validate_stack_trace(&self, frames: &[StackFrame], min_depth: usize) -> ComparisonResult {
        let mut differences = Vec::new();

        // Check minimum depth
        if frames.len() < min_depth {
            differences.push(format!(
                "Stack depth {} is less than minimum {}",
                frames.len(),
                min_depth
            ));
        }

        // Validate frame indices are sequential
        for (expected_idx, frame) in frames.iter().enumerate() {
            if frame.index != expected_idx {
                differences.push(format!(
                    "Frame index mismatch: expected {}, got {}",
                    expected_idx, frame.index
                ));
            }
        }

        // Validate addresses are non-zero (except possibly frame 0 for main)
        for (i, frame) in frames.iter().enumerate() {
            if frame.rip == 0 && i > 0 {
                differences.push(format!("Frame {} has null address", i));
            }
        }

        if differences.is_empty() {
            ComparisonResult::pass(format!("Stack trace valid ({} frames)", frames.len()))
        } else {
            ComparisonResult::fail(
                format!("{} stack trace issues", differences.len()),
                differences,
            )
        }
    }

    /// Validate that a function name appears in the stack trace
    pub fn validate_function_in_stack(
        &self,
        frames: &[StackFrame],
        function_name: &str,
    ) -> ComparisonResult {
        for frame in frames {
            if let Some(ref name) = frame.function_name {
                if name.contains(function_name) {
                    return ComparisonResult::pass(format!(
                        "Found '{}' at frame {}",
                        function_name, frame.index
                    ));
                }
            }
        }

        ComparisonResult::fail(
            format!("Function '{}' not found in stack", function_name),
            vec![format!(
                "Searched {} frames, no match for '{}'",
                frames.len(),
                function_name
            )],
        )
    }

    // =========================================================================
    // Memory Comparisons
    // =========================================================================

    /// Compare memory contents
    ///
    /// # Arguments
    ///
    /// * `kdb_mem` - Memory from kdb
    /// * `gdb_mem` - Memory from GDB
    /// * `address` - Base address (for error messages)
    pub fn compare_memory(
        &self,
        kdb_mem: &[u8],
        gdb_mem: &[u8],
        address: u64,
    ) -> ComparisonResult {
        let mut differences = Vec::new();

        // Check length
        if kdb_mem.len() != gdb_mem.len() {
            differences.push(format!(
                "Length mismatch: kdb={}, gdb={}",
                kdb_mem.len(),
                gdb_mem.len()
            ));
        }

        // Compare bytes
        let min_len = kdb_mem.len().min(gdb_mem.len());
        for i in 0..min_len {
            if kdb_mem[i] != gdb_mem[i] {
                differences.push(format!(
                    "0x{:x}: kdb=0x{:02x}, gdb=0x{:02x}",
                    address + i as u64,
                    kdb_mem[i],
                    gdb_mem[i]
                ));
            }
        }

        if differences.is_empty() {
            ComparisonResult::pass(format!("{} bytes match", min_len))
        } else {
            ComparisonResult::fail(
                format!("{} byte differences", differences.len()),
                differences,
            )
        }
    }

    /// Validate memory contains expected pattern
    pub fn validate_memory_pattern(
        &self,
        memory: &[u8],
        pattern: &[u8],
        address: u64,
    ) -> ComparisonResult {
        // Search for pattern in memory
        for i in 0..=memory.len().saturating_sub(pattern.len()) {
            if &memory[i..i + pattern.len()] == pattern {
                return ComparisonResult::pass(format!(
                    "Pattern found at offset {} (0x{:x})",
                    i,
                    address + i as u64
                ));
            }
        }

        ComparisonResult::fail(
            "Pattern not found in memory",
            vec![format!(
                "Searched {} bytes starting at 0x{:x}",
                memory.len(),
                address
            )],
        )
    }

    // =========================================================================
    // Audit Trail Validation
    // =========================================================================

    /// Validate audit trail integrity
    ///
    /// # Arguments
    ///
    /// * `is_valid` - Result of kdb's verify_audit_trail()
    /// * `root_hash` - Current root hash
    pub fn validate_audit_trail(&self, is_valid: bool, root_hash: u64) -> ComparisonResult {
        if !is_valid {
            return ComparisonResult::fail(
                "Audit trail integrity check failed",
                vec!["Hash chain verification returned false".to_string()],
            );
        }

        ComparisonResult::pass(format!("Audit trail valid (root_hash=0x{:x})", root_hash))
    }

    /// Validate snapshot count
    pub fn validate_snapshot_count(
        &self,
        actual_count: u64,
        min_count: u64,
        max_count: u64,
    ) -> ComparisonResult {
        if actual_count < min_count {
            return ComparisonResult::fail(
                format!("Too few snapshots: {} < {}", actual_count, min_count),
                vec![format!(
                    "Expected at least {} snapshots, got {}",
                    min_count, actual_count
                )],
            );
        }

        if actual_count > max_count {
            return ComparisonResult::fail(
                format!("Too many snapshots: {} > {}", actual_count, max_count),
                vec![format!(
                    "Expected at most {} snapshots, got {}",
                    max_count, actual_count
                )],
            );
        }

        ComparisonResult::pass(format!(
            "Snapshot count {} in range [{}, {}]",
            actual_count, min_count, max_count
        ))
    }

    // =========================================================================
    // Convenience Methods
    // =========================================================================

    /// Assert that a comparison passed (panic if not)
    pub fn assert_pass(&self, result: ComparisonResult) {
        if !result.passed {
            panic!(
                "Validation failed: {}\nDifferences:\n  {}",
                result.summary,
                result.differences.join("\n  ")
            );
        }
    }

    /// Assert that registers match (panic if not)
    pub fn assert_registers_match(&self, kdb_regs: &Registers, gdb_regs: &GdbRegisters) {
        self.assert_pass(self.compare_registers(kdb_regs, gdb_regs));
    }

    /// Assert that stack traces match (panic if not)
    pub fn assert_stack_traces_match(
        &self,
        kdb_frames: &[StackFrame],
        gdb_frames: &[GdbStackFrame],
    ) {
        self.assert_pass(self.compare_stack_traces(kdb_frames, gdb_frames));
    }
}

impl Default for OutputValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comparison_result_pass() {
        let result = ComparisonResult::pass("All good");
        assert!(result.passed);
        assert!(result.differences.is_empty());
        assert!(result.into_result().is_ok());
    }

    #[test]
    fn test_comparison_result_fail() {
        let result = ComparisonResult::fail("Failed", vec!["diff1".to_string()]);
        assert!(!result.passed);
        assert_eq!(result.differences.len(), 1);
        assert!(result.into_result().is_err());
    }

    #[test]
    fn test_validation_config_default() {
        let config = ValidationConfig::default();
        assert!(config.allow_address_variance);
        assert!(config.allow_missing_frames);
    }

    #[test]
    fn test_validation_config_strict() {
        let config = ValidationConfig::strict();
        assert!(!config.allow_address_variance);
        assert!(!config.allow_missing_frames);
    }

    #[test]
    fn test_validate_register() {
        let validator = OutputValidator::new();
        let mut regs = Registers::default();
        regs.rax = 0x1234;

        let result = validator.validate_register(&regs, "rax", 0x1234);
        assert!(result.passed);

        let result = validator.validate_register(&regs, "rax", 0x5678);
        assert!(!result.passed);
    }

    #[test]
    fn test_validate_stack_trace() {
        let validator = OutputValidator::new();

        let frames = vec![
            StackFrame {
                index: 0,
                rip: 0x400000,
                rsp: 0x7fff0000,
                rbp: 0x7fff0010,
                function_name: Some("main".to_string()),
                source_location: None,
            },
            StackFrame {
                index: 1,
                rip: 0x400100,
                rsp: 0x7fff0020,
                rbp: 0x7fff0030,
                function_name: Some("_start".to_string()),
                source_location: None,
            },
        ];

        let result = validator.validate_stack_trace(&frames, 2);
        assert!(result.passed);

        let result = validator.validate_stack_trace(&frames, 5);
        assert!(!result.passed);
    }

    #[test]
    fn test_validate_function_in_stack() {
        let validator = OutputValidator::new();

        let frames = vec![StackFrame {
            index: 0,
            rip: 0x400000,
            rsp: 0x7fff0000,
            rbp: 0x7fff0010,
            function_name: Some("process_data".to_string()),
            source_location: None,
        }];

        let result = validator.validate_function_in_stack(&frames, "process");
        assert!(result.passed);

        let result = validator.validate_function_in_stack(&frames, "nonexistent");
        assert!(!result.passed);
    }

    #[test]
    fn test_validate_memory_pattern() {
        let validator = OutputValidator::new();

        let memory = vec![0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
        let pattern = vec![0x22, 0x33];

        let result = validator.validate_memory_pattern(&memory, &pattern, 0x1000);
        assert!(result.passed);

        let pattern = vec![0xAA, 0xBB];
        let result = validator.validate_memory_pattern(&memory, &pattern, 0x1000);
        assert!(!result.passed);
    }

    #[test]
    fn test_validate_audit_trail() {
        let validator = OutputValidator::new();

        let result = validator.validate_audit_trail(true, 0xDEADBEEF);
        assert!(result.passed);

        let result = validator.validate_audit_trail(false, 0);
        assert!(!result.passed);
    }

    #[test]
    fn test_validate_snapshot_count() {
        let validator = OutputValidator::new();

        let result = validator.validate_snapshot_count(50, 10, 100);
        assert!(result.passed);

        let result = validator.validate_snapshot_count(5, 10, 100);
        assert!(!result.passed);

        let result = validator.validate_snapshot_count(150, 10, 100);
        assert!(!result.passed);
    }
}
