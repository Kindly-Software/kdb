//! Package Types and Metadata Structures
//!
//! **Tier**: T0 (Foundation) + T1 (Atomic state)
//! **Chaos Compliance**: 100% safe, cache-aligned where needed
//!
//! Core data types for package management following dpkg/apt conventions
//! with capsule architecture enhancements.

use core::fmt;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[cfg(feature = "std")]
use std::path::PathBuf;

use super::version::Version;

// ============================================================================
// Package Identifier
// ============================================================================

/// Unique package identifier (name + architecture)
///
/// # dpkg Convention
/// Package identity is `name:arch` (e.g., `libc6:amd64`)
///
/// # Size
/// 48 bytes (name String + arch u8)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageId {
    /// Package name (e.g., "nginx", "openssl")
    pub name: String,
    /// Target architecture
    pub arch: Architecture,
}

impl PackageId {
    /// Create new package identifier
    pub fn new<S: Into<String>>(name: S, arch: Architecture) -> Self {
        Self {
            name: name.into(),
            arch,
        }
    }

    /// Create package ID for current architecture
    pub fn native<S: Into<String>>(name: S) -> Self {
        Self {
            name: name.into(),
            arch: Architecture::native(),
        }
    }

    /// Create architecture-independent package ID
    pub fn all<S: Into<String>>(name: S) -> Self {
        Self {
            name: name.into(),
            arch: Architecture::All,
        }
    }

    /// Parse from "name:arch" string
    pub fn parse(s: &str) -> Option<Self> {
        if let Some((name, arch_str)) = s.split_once(':') {
            let arch = Architecture::parse(arch_str)?;
            Some(Self::new(name, arch))
        } else {
            Some(Self::native(s))
        }
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.arch == Architecture::All {
            write!(f, "{}", self.name)
        } else {
            write!(f, "{}:{}", self.name, self.arch)
        }
    }
}

// ============================================================================
// Architecture
// ============================================================================

/// Target architecture enumeration
///
/// Following dpkg architecture naming conventions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Architecture {
    /// x86-64 (amd64)
    #[default]
    Amd64 = 1,
    /// ARM 64-bit (arm64/aarch64)
    Arm64 = 2,
    /// ARM 32-bit (armhf)
    Armhf = 3,
    /// x86 32-bit (i386)
    I386 = 4,
    /// RISC-V 64-bit
    Riscv64 = 5,
    /// WebAssembly 32-bit
    Wasm32 = 6,
    /// Architecture-independent (all)
    All = 255,
}

impl Architecture {
    /// Get native architecture at compile time
    #[cfg(target_arch = "x86_64")]
    pub const fn native() -> Self {
        Architecture::Amd64
    }

    #[cfg(target_arch = "aarch64")]
    pub const fn native() -> Self {
        Architecture::Arm64
    }

    #[cfg(target_arch = "arm")]
    pub const fn native() -> Self {
        Architecture::Armhf
    }

    #[cfg(target_arch = "x86")]
    pub const fn native() -> Self {
        Architecture::I386
    }

    #[cfg(target_arch = "riscv64")]
    pub const fn native() -> Self {
        Architecture::Riscv64
    }

    #[cfg(target_arch = "wasm32")]
    pub const fn native() -> Self {
        Architecture::Wasm32
    }

    #[cfg(not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "x86",
        target_arch = "riscv64",
        target_arch = "wasm32"
    )))]
    pub const fn native() -> Self {
        Architecture::All
    }

    /// Parse architecture from string
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "amd64" | "x86_64" | "x86-64" => Some(Architecture::Amd64),
            "arm64" | "aarch64" => Some(Architecture::Arm64),
            "armhf" | "arm" | "armv7" => Some(Architecture::Armhf),
            "i386" | "i686" | "x86" => Some(Architecture::I386),
            "riscv64" => Some(Architecture::Riscv64),
            "wasm32" | "wasm" => Some(Architecture::Wasm32),
            "all" | "any" | "noarch" => Some(Architecture::All),
            _ => None,
        }
    }

    /// Check if architecture is compatible with native
    pub fn is_compatible(&self, target: Architecture) -> bool {
        if *self == Architecture::All {
            return true;
        }
        if *self == target {
            return true;
        }
        // Special cases: i386 on amd64, armhf on arm64
        matches!(
            (*self, target),
            (Architecture::I386, Architecture::Amd64) |
            (Architecture::Armhf, Architecture::Arm64)
        )
    }
}

impl fmt::Display for Architecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Architecture::Amd64 => "amd64",
            Architecture::Arm64 => "arm64",
            Architecture::Armhf => "armhf",
            Architecture::I386 => "i386",
            Architecture::Riscv64 => "riscv64",
            Architecture::Wasm32 => "wasm32",
            Architecture::All => "all",
        };
        write!(f, "{}", s)
    }
}

// ============================================================================
// Package State Machine
// ============================================================================

/// Package installation state (dpkg-compatible)
///
/// # State Machine
/// ```text
///                          +----------------+
///                          | NotInstalled   |
///                          +----------------+
///                                  |
///                                  v
///                          +----------------+
///                     +--->| Unpacked       |<---+
///                     |    +----------------+    |
///                     |            |             |
///                     |            v             |
///                     |    +----------------+    |
///                     |    | HalfConfigured |----+
///                     |    +----------------+    |
///                     |            |             |
///                     |            v             |
///                     |    +----------------+    |
///                     +----| Installed      |----+
///                          +----------------+
///                                  |
///                                  v
///                          +----------------+
///                          | HalfInstalled  | (error state)
///                          +----------------+
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PackageState {
    /// Package is not installed
    NotInstalled = 0,
    /// Package files unpacked but not configured
    Unpacked = 1,
    /// Configuration started but incomplete
    HalfConfigured = 2,
    /// Package fully installed and configured
    Installed = 3,
    /// Installation interrupted (broken state)
    HalfInstalled = 4,
    /// Package triggers pending
    TriggersPending = 5,
    /// Package triggers awaited
    TriggersAwaited = 6,
    /// Package marked for removal
    ConfigFiles = 7,
}

impl PackageState {
    /// Check if package is in a usable state
    pub const fn is_usable(&self) -> bool {
        matches!(self, PackageState::Installed)
    }

    /// Check if package is in an error state
    pub const fn is_broken(&self) -> bool {
        matches!(
            self,
            PackageState::HalfInstalled | PackageState::HalfConfigured
        )
    }

    /// Check if state transition is valid
    pub const fn can_transition_to(&self, target: PackageState) -> bool {
        // #ASSUME_STATE_MACHINE: These transitions are dpkg-compatible
        // #VERIFY_STATE_MACHINE: T28 tests cover all valid transitions
        match (*self, target) {
            // NotInstalled can go to Unpacked
            (PackageState::NotInstalled, PackageState::Unpacked) => true,
            // Unpacked can go to HalfConfigured or Installed or back
            (PackageState::Unpacked, PackageState::HalfConfigured) => true,
            (PackageState::Unpacked, PackageState::Installed) => true,
            (PackageState::Unpacked, PackageState::NotInstalled) => true,
            // HalfConfigured can go to Installed or back
            (PackageState::HalfConfigured, PackageState::Installed) => true,
            (PackageState::HalfConfigured, PackageState::Unpacked) => true,
            // Installed can go to ConfigFiles or NotInstalled
            (PackageState::Installed, PackageState::ConfigFiles) => true,
            (PackageState::Installed, PackageState::NotInstalled) => true,
            (PackageState::Installed, PackageState::Unpacked) => true, // reinstall
            // ConfigFiles can go to NotInstalled
            (PackageState::ConfigFiles, PackageState::NotInstalled) => true,
            // Any state can go to HalfInstalled (error)
            (_, PackageState::HalfInstalled) => true,
            // HalfInstalled can attempt recovery
            (PackageState::HalfInstalled, PackageState::NotInstalled) => true,
            (PackageState::HalfInstalled, PackageState::Unpacked) => true,
            // Same state (no-op)
            (a, b) if a as u8 == b as u8 => true,
            _ => false,
        }
    }

    /// Get human-readable description
    pub const fn description(&self) -> &'static str {
        match self {
            PackageState::NotInstalled => "not installed",
            PackageState::Unpacked => "unpacked",
            PackageState::HalfConfigured => "half-configured",
            PackageState::Installed => "installed",
            PackageState::HalfInstalled => "half-installed (broken)",
            PackageState::TriggersPending => "triggers-pending",
            PackageState::TriggersAwaited => "triggers-awaited",
            PackageState::ConfigFiles => "config-files",
        }
    }

    /// Convert from raw u8
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(PackageState::NotInstalled),
            1 => Some(PackageState::Unpacked),
            2 => Some(PackageState::HalfConfigured),
            3 => Some(PackageState::Installed),
            4 => Some(PackageState::HalfInstalled),
            5 => Some(PackageState::TriggersPending),
            6 => Some(PackageState::TriggersAwaited),
            7 => Some(PackageState::ConfigFiles),
            _ => None,
        }
    }
}

impl fmt::Display for PackageState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

// ============================================================================
// Priority
// ============================================================================

/// Package priority level (dpkg-compatible)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u8)]
pub enum Priority {
    /// Required for proper system functioning
    Required = 5,
    /// Important packages installed by default
    Important = 4,
    /// Standard packages on standard install
    #[default]
    Standard = 3,
    /// Optional packages
    Optional = 2,
    /// Extra/specialty packages
    Extra = 1,
}

impl Priority {
    /// Parse from string
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "required" => Some(Priority::Required),
            "important" => Some(Priority::Important),
            "standard" => Some(Priority::Standard),
            "optional" => Some(Priority::Optional),
            "extra" => Some(Priority::Extra),
            _ => None,
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Priority::Required => "required",
            Priority::Important => "important",
            Priority::Standard => "standard",
            Priority::Optional => "optional",
            Priority::Extra => "extra",
        };
        write!(f, "{}", s)
    }
}

// ============================================================================
// Package Metadata
// ============================================================================

/// Complete package metadata
///
/// # Size
/// ~512 bytes (variable due to strings)
///
/// # dpkg Fields Mapping
/// - Package → name
/// - Version → version
/// - Architecture → arch
/// - Priority → priority
/// - Section → section
/// - Depends → dependencies
/// - Maintainer → maintainer
/// - Description → description
#[derive(Debug, Clone)]
pub struct PackageMetadata {
    /// Package name
    pub name: String,
    /// Package version
    pub version: Version,
    /// Target architecture
    pub arch: Architecture,
    /// Priority level
    pub priority: Priority,
    /// Section/category (e.g., "web", "libs", "devel")
    pub section: String,
    /// Package maintainer
    pub maintainer: String,
    /// Short description
    pub description: String,
    /// Long description (optional)
    pub long_description: Option<String>,
    /// Homepage URL
    pub homepage: Option<String>,
    /// Installed size in bytes
    pub installed_size: u64,
    /// Download size in bytes
    pub download_size: u64,
    /// SHA256 checksum (hex)
    pub sha256: String,
    /// Dependencies
    pub dependencies: Vec<Dependency>,
    /// Conflicts
    pub conflicts: Vec<Dependency>,
    /// Provides (virtual packages)
    pub provides: Vec<String>,
    /// Replaces
    pub replaces: Vec<Dependency>,
    /// Source package name
    pub source: Option<String>,
}

impl PackageMetadata {
    /// Create minimal metadata
    pub fn new<S: Into<String>>(name: S, version: Version) -> Self {
        Self {
            name: name.into(),
            version,
            arch: Architecture::native(),
            priority: Priority::Optional,
            section: String::new(),
            maintainer: String::new(),
            description: String::new(),
            long_description: None,
            homepage: None,
            installed_size: 0,
            download_size: 0,
            sha256: String::new(),
            dependencies: Vec::new(),
            conflicts: Vec::new(),
            provides: Vec::new(),
            replaces: Vec::new(),
            source: None,
        }
    }

    /// Get package identifier
    pub fn id(&self) -> PackageId {
        PackageId::new(&self.name, self.arch)
    }
}

// ============================================================================
// Dependency
// ============================================================================

/// Package dependency specification
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    /// Package name
    pub name: String,
    /// Version constraint (optional)
    pub constraint: Option<super::version::VersionConstraint>,
    /// Dependency kind
    pub kind: DependencyKind,
    /// Architecture restriction (optional)
    pub arch: Option<Architecture>,
}

impl Dependency {
    /// Create simple dependency
    pub fn new<S: Into<String>>(name: S) -> Self {
        Self {
            name: name.into(),
            constraint: None,
            kind: DependencyKind::Depends,
            arch: None,
        }
    }

    /// Create dependency with version constraint
    pub fn versioned<S: Into<String>>(
        name: S,
        constraint: super::version::VersionConstraint,
    ) -> Self {
        Self {
            name: name.into(),
            constraint: Some(constraint),
            kind: DependencyKind::Depends,
            arch: None,
        }
    }

    /// Set dependency kind
    pub fn with_kind(mut self, kind: DependencyKind) -> Self {
        self.kind = kind;
        self
    }
}

impl fmt::Display for Dependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(ref constraint) = self.constraint {
            write!(f, " ({})", constraint)?;
        }
        if let Some(arch) = self.arch {
            write!(f, " [{}]", arch)?;
        }
        Ok(())
    }
}

/// Dependency relationship type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum DependencyKind {
    /// Hard dependency (Depends)
    #[default]
    Depends = 0,
    /// Recommendation (Recommends)
    Recommends = 1,
    /// Suggestion (Suggests)
    Suggests = 2,
    /// Pre-dependency (Pre-Depends)
    PreDepends = 3,
    /// Enhancement (Enhances)
    Enhances = 4,
    /// Build dependency (Build-Depends)
    BuildDepends = 5,
}

impl DependencyKind {
    /// Check if dependency is mandatory
    pub const fn is_mandatory(&self) -> bool {
        matches!(self, DependencyKind::Depends | DependencyKind::PreDepends)
    }
}

// ============================================================================
// Package (Full Package Entry)
// ============================================================================

/// Full package entry with metadata and state
///
/// # Size
/// ~640 bytes (metadata + state + timestamps)
#[derive(Debug, Clone)]
pub struct Package {
    /// Package metadata
    pub metadata: PackageMetadata,
    /// Current installation state
    pub state: PackageState,
    /// Installation timestamp (Unix epoch)
    pub installed_at: Option<u64>,
    /// Last update timestamp
    pub updated_at: Option<u64>,
    /// Configuration files (paths)
    #[cfg(feature = "std")]
    pub config_files: Vec<PathBuf>,
    #[cfg(not(feature = "std"))]
    pub config_files: Vec<String>,
    /// Files owned by package
    #[cfg(feature = "std")]
    pub files: Vec<FileEntry>,
}

impl Package {
    /// Create new package entry
    pub fn new(metadata: PackageMetadata) -> Self {
        Self {
            metadata,
            state: PackageState::NotInstalled,
            installed_at: None,
            updated_at: None,
            config_files: Vec::new(),
            #[cfg(feature = "std")]
            files: Vec::new(),
        }
    }
}

// ============================================================================
// File Entry
// ============================================================================

/// File entry in package
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// File path relative to root
    pub path: PathBuf,
    /// File mode (permissions)
    pub mode: u32,
    /// File size in bytes
    pub size: u64,
    /// MD5 checksum (for conffiles)
    pub md5: Option<String>,
}

#[cfg(feature = "std")]
impl FileEntry {
    /// Create new file entry
    pub fn new(path: PathBuf, mode: u32, size: u64) -> Self {
        Self {
            path,
            mode,
            size,
            md5: None,
        }
    }

    /// Check if file is a configuration file
    pub fn is_conffile(&self) -> bool {
        self.md5.is_some()
    }
}

// ============================================================================
// Script Types
// ============================================================================

/// Maintainer script kind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ScriptKind {
    /// Pre-installation script
    PreInst = 0,
    /// Post-installation script
    PostInst = 1,
    /// Pre-removal script
    PreRm = 2,
    /// Post-removal script
    PostRm = 3,
    /// Configuration script
    Config = 4,
}

impl ScriptKind {
    /// Get script filename
    pub const fn filename(&self) -> &'static str {
        match self {
            ScriptKind::PreInst => "preinst",
            ScriptKind::PostInst => "postinst",
            ScriptKind::PreRm => "prerm",
            ScriptKind::PostRm => "postrm",
            ScriptKind::Config => "config",
        }
    }
}

/// Maintainer script
#[derive(Debug, Clone)]
pub struct Script {
    /// Script kind
    pub kind: ScriptKind,
    /// Script content (shell script)
    pub content: String,
    /// Interpreter (default: /bin/sh)
    pub interpreter: String,
}

impl Script {
    /// Create new script
    pub fn new(kind: ScriptKind, content: String) -> Self {
        Self {
            kind,
            content,
            interpreter: "/bin/sh".to_string(),
        }
    }
}

// ============================================================================
// Package Specification (for requests)
// ============================================================================

/// Package installation/query specification
#[derive(Debug, Clone)]
pub struct PackageSpec {
    /// Package name
    pub name: String,
    /// Version constraint
    pub constraint: super::version::VersionConstraint,
    /// Target architecture (optional)
    pub arch: Option<Architecture>,
}

impl PackageSpec {
    /// Create specification with version constraint
    pub fn new<S: Into<String>>(name: S, constraint: super::version::VersionConstraint) -> Self {
        Self {
            name: name.into(),
            constraint,
            arch: None,
        }
    }

    /// Create specification for latest version
    pub fn latest<S: Into<String>>(name: S) -> Self {
        Self {
            name: name.into(),
            constraint: super::version::VersionConstraint::Any,
            arch: None,
        }
    }

    /// Create specification for exact version
    pub fn exact<S: Into<String>>(name: S, version: Version) -> Self {
        Self {
            name: name.into(),
            constraint: super::version::VersionConstraint::Exact(version),
            arch: None,
        }
    }

    /// Set target architecture
    pub fn with_arch(mut self, arch: Architecture) -> Self {
        self.arch = Some(arch);
        self
    }
}

// ============================================================================
// Repository Types
// ============================================================================

/// Repository configuration
#[derive(Debug, Clone)]
pub struct Repository {
    /// Repository identifier
    pub id: String,
    /// Repository URL
    pub url: String,
    /// Distribution (e.g., "stable", "testing")
    pub distribution: String,
    /// Components (e.g., ["main", "contrib"])
    pub components: Vec<String>,
    /// Architectures supported
    pub architectures: Vec<Architecture>,
    /// Signing key fingerprint
    pub key_fingerprint: Option<String>,
    /// Priority (for package selection)
    pub priority: i32,
    /// Enabled flag
    pub enabled: bool,
}

impl Repository {
    /// Create new repository configuration
    pub fn new<S: Into<String>>(id: S, url: S, distribution: S) -> Self {
        Self {
            id: id.into(),
            url: url.into(),
            distribution: distribution.into(),
            components: vec!["main".to_string()],
            architectures: vec![Architecture::native()],
            key_fingerprint: None,
            priority: 500,
            enabled: true,
        }
    }
}

/// Repository package entry (from Packages file)
#[derive(Debug, Clone)]
pub struct RepositoryEntry {
    /// Package metadata
    pub metadata: PackageMetadata,
    /// Download filename
    pub filename: String,
    /// Repository source
    pub repository_id: String,
}

// ============================================================================
// Atomic Package State (for PackageDbCapsule)
// ============================================================================

/// Atomic package state for lockfree database operations
///
/// # Layout (64 bytes, cache-aligned)
/// - state: u8 (PackageState)
/// - flags: u8 (hold, auto-installed, etc.)
/// - arch: u8 (Architecture)
/// - priority: u8 (Priority)
/// - version_hash: u32 (FNV-1a hash of version string)
/// - installed_at: u64 (Unix timestamp)
/// - updated_at: u64 (Unix timestamp)
/// - generation: u64 (for ABA prevention)
/// - name_hash: u64 (FNV-1a hash of name)
/// - padding: [u8; 24]
#[repr(C, align(64))]
pub struct AtomicPackageState {
    /// Packed state (state | flags | arch | priority)
    packed_state: AtomicU32,
    /// Version hash
    version_hash: AtomicU32,
    /// Installation timestamp
    installed_at: AtomicU64,
    /// Update timestamp
    updated_at: AtomicU64,
    /// Generation counter (ABA prevention)
    generation: AtomicU64,
    /// Name hash for quick lookup
    name_hash: AtomicU64,
    /// Padding to 64 bytes
    _padding: [u8; 16],
}

impl AtomicPackageState {
    /// Create new atomic package state
    pub const fn new() -> Self {
        Self {
            packed_state: AtomicU32::new(0),
            version_hash: AtomicU32::new(0),
            installed_at: AtomicU64::new(0),
            updated_at: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            name_hash: AtomicU64::new(0),
            _padding: [0; 16],
        }
    }

    /// Load current state
    pub fn load_state(&self) -> PackageState {
        let packed = self.packed_state.load(Ordering::Acquire);
        PackageState::from_raw((packed & 0xFF) as u8).unwrap_or(PackageState::NotInstalled)
    }

    /// Store state with generation bump
    pub fn store_state(&self, state: PackageState) {
        let current = self.packed_state.load(Ordering::Relaxed);
        let new = (current & !0xFF) | (state as u32);
        self.packed_state.store(new, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get generation counter
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Check if package is installed
    pub fn is_installed(&self) -> bool {
        self.load_state() == PackageState::Installed
    }

    /// Set installed timestamp
    pub fn set_installed_at(&self, timestamp: u64) {
        self.installed_at.store(timestamp, Ordering::Release);
    }

    /// Get installed timestamp
    pub fn installed_at(&self) -> Option<u64> {
        let ts = self.installed_at.load(Ordering::Acquire);
        if ts > 0 { Some(ts) } else { None }
    }
}

impl Default for AtomicPackageState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Constants
// ============================================================================

/// Maximum package name length
pub const MAX_PACKAGE_NAME_LEN: usize = 128;

/// Maximum version string length
pub const MAX_VERSION_LEN: usize = 64;

/// Maximum number of dependencies per package
pub const MAX_DEPENDENCIES: usize = 256;

/// Maximum number of packages in database
pub const MAX_PACKAGES: usize = 65536;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_id_parse() {
        let id = PackageId::parse("nginx:amd64").unwrap();
        assert_eq!(id.name, "nginx");
        assert_eq!(id.arch, Architecture::Amd64);
    }

    #[test]
    fn test_package_id_native() {
        let id = PackageId::parse("openssl").unwrap();
        assert_eq!(id.name, "openssl");
        assert_eq!(id.arch, Architecture::native());
    }

    #[test]
    fn test_architecture_parse() {
        assert_eq!(Architecture::parse("amd64"), Some(Architecture::Amd64));
        assert_eq!(Architecture::parse("arm64"), Some(Architecture::Arm64));
        assert_eq!(Architecture::parse("all"), Some(Architecture::All));
    }

    #[test]
    fn test_architecture_compatibility() {
        assert!(Architecture::All.is_compatible(Architecture::Amd64));
        assert!(Architecture::Amd64.is_compatible(Architecture::Amd64));
        assert!(Architecture::I386.is_compatible(Architecture::Amd64));
        assert!(!Architecture::Arm64.is_compatible(Architecture::Amd64));
    }

    #[test]
    fn test_package_state_transitions() {
        assert!(PackageState::NotInstalled.can_transition_to(PackageState::Unpacked));
        assert!(PackageState::Unpacked.can_transition_to(PackageState::Installed));
        assert!(!PackageState::NotInstalled.can_transition_to(PackageState::Installed));
    }

    #[test]
    fn test_package_state_error() {
        // Any state can transition to HalfInstalled (error)
        assert!(PackageState::Installed.can_transition_to(PackageState::HalfInstalled));
        assert!(PackageState::Unpacked.can_transition_to(PackageState::HalfInstalled));
    }

    #[test]
    fn test_atomic_package_state() {
        let state = AtomicPackageState::new();
        assert_eq!(state.load_state(), PackageState::NotInstalled);

        state.store_state(PackageState::Installed);
        assert_eq!(state.load_state(), PackageState::Installed);
        assert!(state.is_installed());
        assert!(state.generation() > 0);
    }

    #[test]
    fn test_atomic_package_state_alignment() {
        assert_eq!(core::mem::align_of::<AtomicPackageState>(), 64);
        assert_eq!(core::mem::size_of::<AtomicPackageState>(), 64);
    }

    #[test]
    fn test_dependency_kind() {
        assert!(DependencyKind::Depends.is_mandatory());
        assert!(DependencyKind::PreDepends.is_mandatory());
        assert!(!DependencyKind::Recommends.is_mandatory());
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Required > Priority::Important);
        assert!(Priority::Important > Priority::Standard);
        assert!(Priority::Standard > Priority::Optional);
        assert!(Priority::Optional > Priority::Extra);
    }
}
