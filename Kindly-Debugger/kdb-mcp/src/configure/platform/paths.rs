//! Cross-platform path resolution for KDB configuration files.
//!
//! # Overview
//! Provides XDG Base Directory (Linux), Library (macOS), and AppData (Windows)
//! compliant path resolution for configuration, data, cache, and system directories.
//!
//! # Platform-Specific Paths
//!
//! ## Linux (XDG Base Directory Specification)
//! - Config: `$XDG_CONFIG_HOME/kdb/` or `~/.config/kdb/`
//! - Data: `$XDG_DATA_HOME/kdb/` or `~/.local/share/kdb/`
//! - Cache: `$XDG_CACHE_HOME/kdb/` or `~/.cache/kdb/`
//! - System: `/etc/kdb/`
//!
//! ## macOS
//! - Config: `~/Library/Application Support/kdb/`
//! - Data: `~/Library/Application Support/kdb/`
//! - Cache: `~/Library/Caches/kdb/`
//! - System: `/etc/kdb/`
//!
//! ## Windows
//! - Config: `%APPDATA%\kdb\` (e.g., `C:\Users\User\AppData\Roaming\kdb\`)
//! - Data: `%LOCALAPPDATA%\kdb\`
//! - Cache: `%LOCALAPPDATA%\Temp\kdb\`
//! - System: `%PROGRAMDATA%\kdb\`
//!
//! # Tier Selection (UCE35 Q10)
//! This is a utility module (not a capsule) - pure functions for path resolution.
//! No atomic operations, no concurrency concerns, deterministic output.
//!
//! # Security
//! - Environment variable expansion supports `${VAR:-default}` syntax
//! - Secure permissions (chmod 600) for sensitive files on Unix
//! - Tilde expansion for home directory paths

use std::env;
use std::path::{Path, PathBuf};

// Re-export Platform from detector module for use in path functions
pub use super::detector::Platform;

/// Get the base configuration directory for the current platform (auto-detected).
///
/// This is a convenience function that detects the platform at compile time
/// and returns the appropriate configuration directory.
///
/// # Returns
/// `Some(PathBuf)` with the platform-specific configuration base directory,
/// or `None` if detection fails.
///
/// # Example
/// ```
/// use kdb_mcp::configure::platform::paths::get_config_dir;
///
/// if let Some(config_dir) = get_config_dir() {
///     println!("Config dir: {:?}", config_dir);
/// }
/// ```
#[must_use]
pub fn get_config_dir() -> Option<std::path::PathBuf> {
    let platform = {
        #[cfg(target_os = "linux")]
        { Platform::Linux }
        #[cfg(target_os = "macos")]
        { Platform::MacOS }
        #[cfg(target_os = "windows")]
        { Platform::Windows }
        #[cfg(target_os = "freebsd")]
        { Platform::FreeBSD }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows", target_os = "freebsd")))]
        { Platform::Unknown }
    };
    Some(get_config_dir_for_platform(platform))
}

/// Get the user's home directory.
///
/// # Returns
/// - Unix: `$HOME` environment variable
/// - Windows: `%USERPROFILE%` environment variable
/// - Fallback: Current directory "."
fn get_home_dir() -> PathBuf {
    env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// Get the base configuration directory for the specified platform.
///
/// # Arguments
/// * `platform` - Target platform for path resolution
///
/// # Returns
/// Platform-specific configuration base directory:
/// - Linux/FreeBSD: `$XDG_CONFIG_HOME` or `~/.config`
/// - macOS: `~/Library/Application Support`
/// - Windows: `%APPDATA%`
/// - Unknown: Falls back to `~/.config`
///
/// # Example
/// ```
/// use kdb_mcp::configure::platform::paths::{get_config_dir_for_platform, Platform};
///
/// let config_dir = get_config_dir_for_platform(Platform::Linux);
/// // Returns ~/.config or $XDG_CONFIG_HOME
/// ```
#[must_use]
pub fn get_config_dir_for_platform(platform: Platform) -> PathBuf {
    match platform {
        Platform::Linux | Platform::FreeBSD | Platform::Unknown => {
            env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| get_home_dir().join(".config"))
        }
        Platform::MacOS => get_home_dir().join("Library").join("Application Support"),
        Platform::Windows => {
            env::var("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|_| get_home_dir().join("AppData").join("Roaming"))
        }
    }
}

/// Get the base data directory for the platform.
///
/// # Arguments
/// * `platform` - Target platform for path resolution
///
/// # Returns
/// Platform-specific data base directory:
/// - Linux: `$XDG_DATA_HOME` or `~/.local/share`
/// - macOS: `~/Library/Application Support`
/// - Windows: `%LOCALAPPDATA%`
/// - Unknown: Falls back to `~/.local/share`
#[must_use]
pub fn get_data_dir(platform: Platform) -> PathBuf {
    match platform {
        Platform::Linux | Platform::FreeBSD | Platform::Unknown => {
            env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| get_home_dir().join(".local").join("share"))
        }
        Platform::MacOS => get_home_dir().join("Library").join("Application Support"),
        Platform::Windows => {
            env::var("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|_| get_home_dir().join("AppData").join("Local"))
        }
    }
}

/// Get the base cache directory for the platform.
///
/// # Arguments
/// * `platform` - Target platform for path resolution
///
/// # Returns
/// Platform-specific cache base directory:
/// - Linux: `$XDG_CACHE_HOME` or `~/.cache`
/// - macOS: `~/Library/Caches`
/// - Windows: `%LOCALAPPDATA%\Temp`
/// - Unknown: Falls back to `~/.cache`
#[must_use]
pub fn get_cache_dir(platform: Platform) -> PathBuf {
    match platform {
        Platform::Linux | Platform::FreeBSD | Platform::Unknown => {
            env::var_os("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| get_home_dir().join(".cache"))
        }
        Platform::MacOS => get_home_dir().join("Library").join("Caches"),
        Platform::Windows => {
            env::var("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|_| get_home_dir().join("AppData").join("Local"))
                .join("Temp")
        }
    }
}

/// Get the system-wide configuration directory for the platform.
///
/// # Arguments
/// * `platform` - Target platform for path resolution
///
/// # Returns
/// Platform-specific system configuration directory:
/// - Unix (Linux/macOS): `/etc/kdb`
/// - Windows: `%PROGRAMDATA%\kdb`
/// - Unknown: `/etc/kdb`
#[must_use]
pub fn get_system_config_dir(platform: Platform) -> PathBuf {
    match platform {
        Platform::Windows => {
            env::var("PROGRAMDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("C:\\ProgramData"))
                .join("kdb")
        }
        _ => PathBuf::from("/etc/kdb"),
    }
}

/// Get the KDB-specific configuration directory.
///
/// # Arguments
/// * `platform` - Target platform for path resolution
///
/// # Returns
/// `{config_dir}/kdb/` for the given platform.
///
/// # Example
/// ```
/// use kdb_mcp::configure::platform::paths::{get_kdb_config_dir, Platform};
///
/// let kdb_config = get_kdb_config_dir(Platform::Linux);
/// // Returns ~/.config/kdb or $XDG_CONFIG_HOME/kdb
/// ```
#[must_use]
pub fn get_kdb_config_dir(platform: Platform) -> PathBuf {
    get_config_dir_for_platform(platform).join("kdb")
}

/// Get the KDB-specific data directory.
///
/// # Arguments
/// * `platform` - Target platform for path resolution
///
/// # Returns
/// `{data_dir}/kdb/` for the given platform.
#[must_use]
pub fn get_kdb_data_dir(platform: Platform) -> PathBuf {
    get_data_dir(platform).join("kdb")
}

/// Get the KDB-specific cache directory.
///
/// # Arguments
/// * `platform` - Target platform for path resolution
///
/// # Returns
/// `{cache_dir}/kdb/` for the given platform.
#[must_use]
pub fn get_kdb_cache_dir(platform: Platform) -> PathBuf {
    get_cache_dir(platform).join("kdb")
}

/// Get the path to the KDB user `.env` file.
///
/// # Arguments
/// * `platform` - Target platform for path resolution
///
/// # Returns
/// `{kdb_config_dir}/.env` for the given platform.
///
/// # Example
/// ```
/// use kdb_mcp::configure::platform::paths::{get_kdb_env_path, Platform};
///
/// let env_path = get_kdb_env_path(Platform::Linux);
/// // Returns ~/.config/kdb/.env
/// ```
#[must_use]
pub fn get_kdb_env_path(platform: Platform) -> PathBuf {
    get_kdb_config_dir(platform).join(".env")
}

/// Get the path to the KDB license file.
///
/// # Arguments
/// * `platform` - Target platform for path resolution
///
/// # Returns
/// `{kdb_config_dir}/license.key` for the given platform.
#[must_use]
pub fn get_kdb_license_path(platform: Platform) -> PathBuf {
    get_kdb_config_dir(platform).join("license.key")
}

/// Expand tilde (`~`) and environment variables in a path string.
///
/// # Arguments
/// * `path` - Path string potentially containing `~` or environment variables
/// * `platform` - Target platform for environment variable syntax
///
/// # Returns
/// Expanded `PathBuf` with all variables resolved.
///
/// # Environment Variable Syntax
/// - Unix-style: `${VAR}` or `${VAR:-default}`
/// - Windows-style: `%VAR%` (only on Windows platform)
///
/// # Example
/// ```
/// use kdb_mcp::configure::platform::paths::{expand_path, Platform};
///
/// let path = expand_path("~/config", Platform::Linux);
/// // Returns /home/user/config
///
/// let path = expand_path("${HOME}/config", Platform::Linux);
/// // Returns /home/user/config
///
/// let path = expand_path("${MISSING:-/default}/config", Platform::Linux);
/// // Returns /default/config
/// ```
#[must_use]
pub fn expand_path(path: &str, platform: Platform) -> PathBuf {
    // First, expand tilde
    let expanded = if path.starts_with("~/") {
        let home = get_home_dir();
        home.join(&path[2..])
    } else if path == "~" {
        get_home_dir()
    } else {
        PathBuf::from(path)
    };

    // Then expand environment variables
    let path_str = expanded.to_string_lossy().to_string();
    expand_env_vars(&path_str, platform)
}

/// Expand environment variables in a string.
///
/// # Arguments
/// * `s` - String potentially containing environment variables
/// * `platform` - Target platform for environment variable syntax
///
/// # Returns
/// Expanded `PathBuf` with all environment variables resolved.
///
/// # Supported Syntax
/// - `${VAR}` - Replace with value of VAR, empty string if unset
/// - `${VAR:-default}` - Replace with value of VAR, or "default" if unset
/// - `%VAR%` - Windows-style (only on Windows platform)
///
/// # Example
/// ```
/// use kdb_mcp::configure::platform::paths::{expand_env_vars, Platform};
///
/// let result = expand_env_vars("${HOME}/config", Platform::Linux);
/// // Returns /home/user/config
///
/// let result = expand_env_vars("${MISSING:-fallback}/config", Platform::Linux);
/// // Returns fallback/config
/// ```
#[must_use]
pub fn expand_env_vars(s: &str, platform: Platform) -> PathBuf {
    let mut result = s.to_string();

    // Unix-style ${VAR} and ${VAR:-default}
    while let Some(start) = result.find("${") {
        if let Some(end_offset) = result[start..].find('}') {
            let end = start + end_offset;
            let var_expr = &result[start + 2..end];

            // Handle ${VAR:-default}
            let (var_name, default) = if let Some(colon_pos) = var_expr.find(":-") {
                (&var_expr[..colon_pos], Some(&var_expr[colon_pos + 2..]))
            } else {
                (var_expr, None)
            };

            let value = env::var(var_name)
                .ok()
                .or_else(|| default.map(|d| d.to_string()))
                .unwrap_or_default();

            result.replace_range(start..=end, &value);
        } else {
            // Malformed variable reference, stop processing
            break;
        }
    }

    // Windows-style %VAR% (only on Windows platform)
    if matches!(platform, Platform::Windows) {
        while let Some(start) = result.find('%') {
            if let Some(end_offset) = result[start + 1..].find('%') {
                let end = start + 1 + end_offset;
                let var_name = &result[start + 1..end];

                // Skip empty %% sequences
                if var_name.is_empty() {
                    break;
                }

                let value = env::var(var_name).unwrap_or_default();
                result.replace_range(start..=end, &value);
            } else {
                // No closing %, stop processing
                break;
            }
        }
    }

    PathBuf::from(result)
}

/// Set secure permissions on a file (chmod 600 on Unix).
///
/// # Arguments
/// * `path` - Path to the file to secure
///
/// # Returns
/// `Ok(())` on success, `Err` on failure.
///
/// # Permissions
/// - Unix: `rw-------` (0o600) - owner read/write only
/// - Windows: No-op (file permissions are handled differently via ACLs)
///
/// # Example
/// ```no_run
/// use kdb_mcp::configure::platform::paths::set_secure_permissions;
/// use std::path::Path;
///
/// let path = Path::new("/home/user/.config/kdb/.env");
/// set_secure_permissions(path).expect("Failed to set permissions");
/// ```
#[cfg(unix)]
pub fn set_secure_permissions(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = std::fs::metadata(path)?;
    let mut perms = metadata.permissions();
    perms.set_mode(0o600); // rw-------
    std::fs::set_permissions(path, perms)
}

/// Set secure permissions on a file (no-op on non-Unix platforms).
///
/// # Arguments
/// * `_path` - Path to the file (unused on Windows)
///
/// # Returns
/// Always `Ok(())` on Windows.
///
/// # Note
/// Windows file permissions are handled via ACLs which require more complex
/// setup. For production use, consider using `icacls` or Windows ACL APIs.
#[cfg(not(unix))]
pub fn set_secure_permissions(_path: &Path) -> std::io::Result<()> {
    // Windows: No-op for now
    // Production: Use icacls or SetSecurityInfo for ACL-based permissions
    Ok(())
}

/// Ensure a directory exists with secure permissions.
///
/// # Arguments
/// * `path` - Path to the directory to create
///
/// # Returns
/// `Ok(())` on success, `Err` on failure.
///
/// # Behavior
/// - Creates directory and all parent directories if they don't exist
/// - On Unix: Sets directory permissions to 0o700 (rwx------)
pub fn ensure_secure_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o700); // rwx------
        std::fs::set_permissions(path, perms)?;
    }

    Ok(())
}

// =============================================================================
// UNIT TESTS (T28 Q1-Q7)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    // -------------------------------------------------------------------------
    // Test 1: get_config_dir_linux
    // -------------------------------------------------------------------------
    #[test]
    fn test_get_config_dir_linux() {
        // Save current XDG_CONFIG_HOME
        let original = env::var("XDG_CONFIG_HOME").ok();

        // Test with XDG_CONFIG_HOME set
        env::set_var("XDG_CONFIG_HOME", "/custom/config");
        let config_dir = get_config_dir_for_platform(Platform::Linux);
        assert_eq!(config_dir, PathBuf::from("/custom/config"));

        // Test without XDG_CONFIG_HOME (falls back to ~/.config)
        env::remove_var("XDG_CONFIG_HOME");
        let config_dir = get_config_dir_for_platform(Platform::Linux);
        let expected = get_home_dir().join(".config");
        assert_eq!(config_dir, expected);

        // Restore original
        if let Some(val) = original {
            env::set_var("XDG_CONFIG_HOME", val);
        }
    }

    // -------------------------------------------------------------------------
    // Test 2: get_config_dir_macos
    // -------------------------------------------------------------------------
    #[test]
    fn test_get_config_dir_macos() {
        let config_dir = get_config_dir_for_platform(Platform::MacOS);
        let expected = get_home_dir().join("Library").join("Application Support");
        assert_eq!(config_dir, expected);
    }

    // -------------------------------------------------------------------------
    // Test 3: get_config_dir_windows
    // -------------------------------------------------------------------------
    #[test]
    fn test_get_config_dir_windows() {
        // Save current APPDATA
        let original = env::var("APPDATA").ok();

        // Test with APPDATA set
        env::set_var("APPDATA", "C:\\Users\\Test\\AppData\\Roaming");
        let config_dir = get_config_dir_for_platform(Platform::Windows);
        assert_eq!(
            config_dir,
            PathBuf::from("C:\\Users\\Test\\AppData\\Roaming")
        );

        // Restore original
        if let Some(val) = original {
            env::set_var("APPDATA", val);
        } else {
            env::remove_var("APPDATA");
        }
    }

    // -------------------------------------------------------------------------
    // Test 4: expand_tilde
    // -------------------------------------------------------------------------
    #[test]
    fn test_expand_tilde() {
        let home = get_home_dir();

        // Test ~/path expansion
        let expanded = expand_path("~/config/kdb", Platform::Linux);
        assert_eq!(expanded, home.join("config").join("kdb"));

        // Test ~ alone
        let expanded = expand_path("~", Platform::Linux);
        assert_eq!(expanded, home);

        // Test path without tilde (should remain unchanged)
        let expanded = expand_path("/etc/kdb", Platform::Linux);
        assert_eq!(expanded, PathBuf::from("/etc/kdb"));

        // Test tilde in middle of path (should not expand)
        let expanded = expand_path("/path/~user/config", Platform::Linux);
        assert_eq!(expanded, PathBuf::from("/path/~user/config"));
    }

    // -------------------------------------------------------------------------
    // Test 5: expand_env_vars_unix
    // -------------------------------------------------------------------------
    #[test]
    fn test_expand_env_vars_unix() {
        // Save and set test variable
        let original = env::var("TEST_KDB_VAR").ok();
        env::set_var("TEST_KDB_VAR", "/test/value");

        // Test ${VAR} expansion
        let expanded = expand_env_vars("${TEST_KDB_VAR}/config", Platform::Linux);
        assert_eq!(expanded, PathBuf::from("/test/value/config"));

        // Test multiple variables
        env::set_var("TEST_KDB_VAR2", "subdir");
        let expanded =
            expand_env_vars("${TEST_KDB_VAR}/${TEST_KDB_VAR2}/file", Platform::Linux);
        assert_eq!(expanded, PathBuf::from("/test/value/subdir/file"));

        // Cleanup
        env::remove_var("TEST_KDB_VAR2");
        if let Some(val) = original {
            env::set_var("TEST_KDB_VAR", val);
        } else {
            env::remove_var("TEST_KDB_VAR");
        }
    }

    // -------------------------------------------------------------------------
    // Test 6: expand_env_vars_with_default
    // -------------------------------------------------------------------------
    #[test]
    fn test_expand_env_vars_with_default() {
        // Ensure variable is not set
        env::remove_var("TEST_MISSING_VAR_12345");

        // Test ${VAR:-default} expansion when VAR is not set
        let expanded = expand_env_vars(
            "${TEST_MISSING_VAR_12345:-/default/path}/config",
            Platform::Linux,
        );
        assert_eq!(expanded, PathBuf::from("/default/path/config"));

        // Test ${VAR:-default} expansion when VAR is set
        env::set_var("TEST_MISSING_VAR_12345", "/actual/value");
        let expanded = expand_env_vars(
            "${TEST_MISSING_VAR_12345:-/default/path}/config",
            Platform::Linux,
        );
        assert_eq!(expanded, PathBuf::from("/actual/value/config"));

        // Cleanup
        env::remove_var("TEST_MISSING_VAR_12345");
    }

    // -------------------------------------------------------------------------
    // Test 7: expand_env_vars_windows
    // -------------------------------------------------------------------------
    #[test]
    fn test_expand_env_vars_windows() {
        // Save and set test variable
        let original = env::var("TEST_WIN_VAR").ok();
        env::set_var("TEST_WIN_VAR", "C:\\TestPath");

        // Test %VAR% expansion on Windows platform
        let expanded = expand_env_vars("%TEST_WIN_VAR%\\config", Platform::Windows);
        assert_eq!(expanded, PathBuf::from("C:\\TestPath\\config"));

        // Test %VAR% NOT expanded on Linux platform
        let expanded = expand_env_vars("%TEST_WIN_VAR%\\config", Platform::Linux);
        assert_eq!(expanded, PathBuf::from("%TEST_WIN_VAR%\\config"));

        // Cleanup
        if let Some(val) = original {
            env::set_var("TEST_WIN_VAR", val);
        } else {
            env::remove_var("TEST_WIN_VAR");
        }
    }

    // -------------------------------------------------------------------------
    // Test 8: secure_permissions (Unix only)
    // -------------------------------------------------------------------------
    #[test]
    #[cfg(unix)]
    fn test_secure_permissions() {
        use std::os::unix::fs::PermissionsExt;

        // Create a temporary file
        let temp_dir = env::temp_dir();
        let test_file = temp_dir.join("kdb_test_permissions_file");

        // Create the file
        std::fs::write(&test_file, "test content").expect("Failed to create test file");

        // Set secure permissions
        set_secure_permissions(&test_file).expect("Failed to set permissions");

        // Verify permissions
        let metadata = std::fs::metadata(&test_file).expect("Failed to get metadata");
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "Expected mode 0o600, got 0o{:o}", mode);

        // Cleanup
        std::fs::remove_file(&test_file).ok();
    }

    // -------------------------------------------------------------------------
    // Additional tests for completeness
    // -------------------------------------------------------------------------

    #[test]
    fn test_platform_detect() {
        // Test that we can detect the current platform at compile time
        #[cfg(target_os = "linux")]
        {
            let platform = Platform::Linux;
            assert_eq!(platform.as_str(), "linux");
        }
        #[cfg(target_os = "macos")]
        {
            let platform = Platform::MacOS;
            assert_eq!(platform.as_str(), "macos");
        }
        #[cfg(target_os = "windows")]
        {
            let platform = Platform::Windows;
            assert_eq!(platform.as_str(), "windows");
        }
    }

    #[test]
    fn test_get_kdb_config_dir() {
        let kdb_dir = get_kdb_config_dir(Platform::Linux);
        assert!(kdb_dir.ends_with("kdb"));
    }

    #[test]
    fn test_get_kdb_env_path() {
        let env_path = get_kdb_env_path(Platform::Linux);
        assert!(env_path.ends_with(".env"));
        assert!(env_path.parent().unwrap().ends_with("kdb"));
    }

    #[test]
    fn test_get_system_config_dir() {
        let system_dir = get_system_config_dir(Platform::Linux);
        assert_eq!(system_dir, PathBuf::from("/etc/kdb"));

        // Save and set PROGRAMDATA for Windows test
        let original = env::var("PROGRAMDATA").ok();
        env::set_var("PROGRAMDATA", "C:\\ProgramData");
        let system_dir = get_system_config_dir(Platform::Windows);
        assert_eq!(system_dir, PathBuf::from("C:\\ProgramData\\kdb"));

        // Restore
        if let Some(val) = original {
            env::set_var("PROGRAMDATA", val);
        } else {
            env::remove_var("PROGRAMDATA");
        }
    }

    #[test]
    fn test_get_data_dir() {
        let data_dir = get_data_dir(Platform::Linux);
        assert!(
            data_dir.to_string_lossy().contains(".local/share")
                || env::var("XDG_DATA_HOME").is_ok()
        );

        let data_dir = get_data_dir(Platform::MacOS);
        assert!(data_dir.to_string_lossy().contains("Library/Application Support"));
    }

    #[test]
    fn test_get_cache_dir() {
        let cache_dir = get_cache_dir(Platform::Linux);
        assert!(
            cache_dir.to_string_lossy().contains(".cache")
                || env::var("XDG_CACHE_HOME").is_ok()
        );

        let cache_dir = get_cache_dir(Platform::MacOS);
        assert!(cache_dir.to_string_lossy().contains("Library/Caches"));
    }

    #[test]
    fn test_platform_supports_xdg() {
        // Test using detector::Platform's supports_xdg() method
        assert!(Platform::Linux.supports_xdg());
        assert!(Platform::FreeBSD.supports_xdg());
        assert!(!Platform::MacOS.supports_xdg());
        assert!(!Platform::Windows.supports_xdg());
        assert!(!Platform::Unknown.supports_xdg());
    }

    #[test]
    fn test_platform_uses_backslash() {
        // Test using detector::Platform's uses_backslash() method
        assert!(Platform::Windows.uses_backslash());
        assert!(!Platform::Linux.uses_backslash());
        assert!(!Platform::MacOS.uses_backslash());
        assert!(!Platform::FreeBSD.uses_backslash());
        assert!(!Platform::Unknown.uses_backslash());
    }

    #[test]
    fn test_expand_env_vars_empty_variable() {
        // Test with unset variable (no default)
        env::remove_var("TEST_EMPTY_VAR_99999");
        let expanded = expand_env_vars("prefix/${TEST_EMPTY_VAR_99999}/suffix", Platform::Linux);
        assert_eq!(expanded, PathBuf::from("prefix//suffix"));
    }

    #[test]
    fn test_expand_path_combined() {
        // Save and set test variable
        let original = env::var("TEST_COMBINED_VAR").ok();
        env::set_var("TEST_COMBINED_VAR", "subdir");

        // Test tilde + env var expansion
        let expanded = expand_path("~/${TEST_COMBINED_VAR}/config", Platform::Linux);
        let expected = get_home_dir().join("subdir").join("config");
        assert_eq!(expanded, expected);

        // Cleanup
        if let Some(val) = original {
            env::set_var("TEST_COMBINED_VAR", val);
        } else {
            env::remove_var("TEST_COMBINED_VAR");
        }
    }

    #[test]
    fn test_malformed_variable_reference() {
        // Test unclosed ${
        let expanded = expand_env_vars("prefix/${UNCLOSED", Platform::Linux);
        assert_eq!(expanded, PathBuf::from("prefix/${UNCLOSED"));

        // Test single % on Windows (no closing %)
        let expanded = expand_env_vars("prefix/%UNCLOSED", Platform::Windows);
        assert_eq!(expanded, PathBuf::from("prefix/%UNCLOSED"));
    }
}
