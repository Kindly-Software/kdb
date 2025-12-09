//! Platform Detection Capsule (T0 Auditable)
//!
//! Detects operating system and architecture to determine the correct
//! GitHub release asset to download.

use std::fmt;

/// Operating system family
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux = 0,
    MacOS = 1,
    Windows = 2,
}

/// CPU architecture
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86_64 = 0,
    Aarch64 = 1,
}

/// Platform detection capsule - immutable, zero-cost abstraction
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PlatformCapsule {
    /// Operating system
    os: Os,
    /// CPU architecture
    arch: Arch,
}

impl PlatformCapsule {
    /// Detect current platform at runtime
    pub fn detect() -> Self {
        let os = if cfg!(target_os = "linux") {
            Os::Linux
        } else if cfg!(target_os = "macos") {
            Os::MacOS
        } else if cfg!(target_os = "windows") {
            Os::Windows
        } else {
            panic!("Unsupported operating system");
        };

        let arch = if cfg!(target_arch = "x86_64") {
            Arch::X86_64
        } else if cfg!(target_arch = "aarch64") {
            Arch::Aarch64
        } else {
            panic!("Unsupported architecture");
        };

        Self { os, arch }
    }

    /// Get GitHub release asset filename for this platform
    pub fn asset_name(&self) -> &'static str {
        match (self.os, self.arch) {
            (Os::Linux, Arch::X86_64) => "kindly-av1-x86_64-unknown-linux-musl.tar.gz",
            (Os::Linux, Arch::Aarch64) => "kindly-av1-aarch64-unknown-linux-musl.tar.gz",
            (Os::MacOS, Arch::X86_64) => "kindly-av1-x86_64-apple-darwin.tar.gz",
            (Os::MacOS, Arch::Aarch64) => "kindly-av1-aarch64-apple-darwin.tar.gz",
            (Os::Windows, Arch::X86_64) => "kindly-av1-x86_64-pc-windows-msvc.zip",
            (Os::Windows, Arch::Aarch64) => "kindly-av1-aarch64-pc-windows-msvc.zip",
        }
    }

    /// Check if this platform uses ZIP archives (Windows)
    pub fn is_zip(&self) -> bool {
        matches!(self.os, Os::Windows)
    }

    /// Get installation directory for this platform
    pub fn install_dir(&self) -> Option<std::path::PathBuf> {
        match self.os {
            Os::Linux | Os::MacOS => {
                dirs::home_dir().map(|h| h.join(".local").join("bin"))
            }
            Os::Windows => {
                dirs::data_local_dir().map(|d| d.join("kindly-av1"))
            }
        }
    }

    /// Get shell configuration file for PATH updates
    pub fn shell_config(&self) -> Option<&'static str> {
        match self.os {
            Os::Linux | Os::MacOS => Some(".bashrc"),
            Os::Windows => None, // Windows uses registry
        }
    }
}

impl fmt::Display for PlatformCapsule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let os_str = match self.os {
            Os::Linux => "Linux",
            Os::MacOS => "macOS",
            Os::Windows => "Windows",
        };
        let arch_str = match self.arch {
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
        };
        write!(f, "{} ({})", os_str, arch_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        let platform = PlatformCapsule::detect();

        // Should always detect something
        assert!(matches!(platform.os, Os::Linux | Os::MacOS | Os::Windows));
        assert!(matches!(platform.arch, Arch::X86_64 | Arch::Aarch64));

        // Asset name should not be empty
        assert!(!platform.asset_name().is_empty());
    }

    #[test]
    fn test_asset_naming() {
        let platform = PlatformCapsule::detect();
        let asset = platform.asset_name();

        // Should contain architecture
        assert!(asset.contains("x86_64") || asset.contains("aarch64"));

        // Should have correct extension
        if platform.is_zip() {
            assert!(asset.ends_with(".zip"));
        } else {
            assert!(asset.ends_with(".tar.gz"));
        }
    }

    #[test]
    fn test_install_dir() {
        let platform = PlatformCapsule::detect();
        let dir = platform.install_dir();

        // Should always return a directory
        assert!(dir.is_some());
    }
}
