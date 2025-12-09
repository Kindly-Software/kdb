// path_setup.rs - T0 Auditable PATH Configuration Capsule
// UCE34 Q34: Auditable, reversible, idempotent PATH modification

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum PathError {
    IoError(io::Error),
    ShellNotDetected,
    UnsupportedShell(String),
}

impl From<io::Error> for PathError {
    fn from(e: io::Error) -> Self {
        PathError::IoError(e)
    }
}

/// Detect user's shell from environment or rc file existence
fn detect_shell() -> Result<String, PathError> {
    // Try $SHELL first
    if let Ok(shell) = std::env::var("SHELL") {
        let shell_name = Path::new(&shell)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        if matches!(shell_name, "bash" | "zsh" | "fish" | "sh") {
            return Ok(shell_name.to_string());
        }
    }

    // Fallback: check which rc files exist
    let home = dirs::home_dir().ok_or(PathError::ShellNotDetected)?;

    if home.join(".zshrc").exists() {
        return Ok("zsh".to_string());
    }
    if home.join(".bashrc").exists() {
        return Ok("bash".to_string());
    }
    if home.join(".config/fish/config.fish").exists() {
        return Ok("fish".to_string());
    }

    Err(PathError::ShellNotDetected)
}

/// Get appropriate rc file path for shell
fn get_rc_path(shell: &str) -> Result<PathBuf, PathError> {
    let home = dirs::home_dir().ok_or(PathError::ShellNotDetected)?;

    match shell {
        "bash" => Ok(home.join(".bashrc")),
        "zsh" => Ok(home.join(".zshrc")),
        "fish" => {
            let config_dir = home.join(".config/fish");
            fs::create_dir_all(&config_dir)?;
            Ok(config_dir.join("config.fish"))
        }
        other => Err(PathError::UnsupportedShell(other.to_string())),
    }
}

/// Format PATH export for shell
fn format_path_export(shell: &str, install_dir: &Path) -> String {
    let path_str = install_dir.to_string_lossy();

    match shell {
        "fish" => format!("set -gx PATH {} $PATH", path_str),
        _ => format!("export PATH=\"{}:$PATH\"", path_str), // bash/zsh/sh
    }
}

/// Check if PATH already contains install directory
fn path_already_added(rc_path: &Path, install_dir: &Path) -> Result<bool, PathError> {
    if !rc_path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(rc_path)?;
    let install_str = install_dir.to_string_lossy();

    Ok(content.contains(&*install_str))
}

/// Add install directory to user's PATH (Unix)
#[cfg(unix)]
pub fn add_to_path(install_dir: &Path) -> Result<(), PathError> {
    let shell = detect_shell()?;
    let rc_path = get_rc_path(&shell)?;

    // Check idempotency
    if path_already_added(&rc_path, install_dir)? {
        println!("[kindly-av1] PATH already configured for {}", shell);
        return Ok(());
    }

    // Backup rc file (T0 Auditable: reversible modifications)
    if rc_path.exists() {
        let backup_path = rc_path.with_extension("backup");
        fs::copy(&rc_path, &backup_path)?;
    }

    // Append PATH export
    let export_line = format_path_export(&shell, install_dir);
    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&rc_path)?;

    writeln!(file, "\n# kindly-av1 installer")?;
    writeln!(file, "{}", export_line)?;

    println!("[kindly-av1] Added kindly-av1 to your PATH ({})", shell);
    println!("[kindly-av1] Please restart your terminal for changes to take effect");

    // Also update ~/.bash_profile for macOS login shells
    #[cfg(target_os = "macos")]
    if shell == "bash" {
        let home = dirs::home_dir().ok_or(PathError::ShellNotDetected)?;
        let profile_path = home.join(".bash_profile");

        if !path_already_added(&profile_path, install_dir)? {
            let mut profile = fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&profile_path)?;

            writeln!(profile, "\n# kindly-av1 installer")?;
            writeln!(profile, "{}", export_line)?;
        }
    }

    Ok(())
}

/// Add install directory to user's PATH (Windows)
#[cfg(windows)]
pub fn add_to_path(install_dir: &Path) -> Result<(), PathError> {
    use std::process::Command;

    let install_str = install_dir.to_string_lossy();

    // Use setx to modify User PATH (no admin required)
    let output = Command::new("setx")
        .arg("PATH")
        .arg(format!("%PATH%;{}", install_str))
        .output()?;

    if !output.status.success() {
        return Err(PathError::IoError(io::Error::new(
            io::ErrorKind::Other,
            "setx failed to modify PATH",
        )));
    }

    println!("[kindly-av1] Added kindly-av1 to your PATH");
    println!("[kindly-av1] Please restart your terminal for changes to take effect");

    Ok(())
}

/// Display post-installation message
pub fn print_success_message() {
    println!("\n[kindly-av1] Installation complete!");
    println!("[kindly-av1] kindly-av1 is ready! Try: kindly-av1 --help");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_shell() {
        // Should not panic
        let _ = detect_shell();
    }

    #[test]
    fn test_format_path_export() {
        let install_dir = Path::new("/home/user/.local/bin");

        let bash_export = format_path_export("bash", install_dir);
        assert!(bash_export.contains("export"));
        assert!(bash_export.contains("/home/user/.local/bin"));

        let fish_export = format_path_export("fish", install_dir);
        assert!(fish_export.contains("set -gx PATH"));
        assert!(fish_export.contains("/home/user/.local/bin"));
    }
}
