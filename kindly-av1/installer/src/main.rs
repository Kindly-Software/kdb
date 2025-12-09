//! kindly-av1 Smart Installer
//!
//! One-command installation for non-technical users.
//! Target: "A 12-year-old YouTuber can install without help"
//!
//! Architecture: T0 Auditable capsules (platform, download, install, path_setup)
//! Framework: UCE34 Q1-Q7 (simple correctness), Chaos compliant

mod download;
mod install;
mod path_setup;
mod platform;

use platform::PlatformCapsule;
use download::DownloadCapsule;
use install::InstallCapsule;

const GITHUB_OWNER: &str = "kindly-team";
const GITHUB_REPO: &str = "kindly-av1";
const RELEASE_TAG: &str = "v1.0.0";
const BINARY_NAME: &str = "kindly-av1";

fn main() {
    // Print friendly banner
    println!("╔════════════════════════════════════════╗");
    println!("║   kindly-av1 Smart Installer v1.0.0   ║");
    println!("║   GPU-Accelerated AV1 Encoder          ║");
    println!("╚════════════════════════════════════════╝\n");

    // Parse optional license key from args
    let license_key = std::env::args().nth(1);

    if let Some(ref key) = license_key {
        println!("📝 License key: {}", mask_license_key(key));
    }

    // Step 1: Detect platform
    println!("🔍 Step 1/4: Detecting platform...");
    let platform = PlatformCapsule::detect();
    println!("   Platform: {}", platform);
    println!("   Asset: {}\n", platform.asset_name());

    // Step 2: Download release asset
    println!("📥 Step 2/4: Downloading kindly-av1...");

    // GitHub releases URL format
    let github_url = format!(
        "https://github.com/{}/{}/releases/download/{}",
        GITHUB_OWNER, GITHUB_REPO, RELEASE_TAG
    );

    let downloader = DownloadCapsule::new(&github_url);
    let temp_archive = std::env::temp_dir().join(platform.asset_name());

    if let Err(e) = downloader.download_with_progress(platform.asset_name(), &temp_archive) {
        eprintln!("\n❌ Download failed: {}", e);
        eprintln!("   Please check your internet connection and try again.");
        std::process::exit(1);
    }

    // Step 3: Extract and install
    println!("\n📦 Step 3/4: Installing kindly-av1...");

    let installer = match platform.install_dir() {
        Some(dir) => InstallCapsule::with_dir(dir),
        None => {
            eprintln!("❌ Cannot determine installation directory for this platform");
            std::process::exit(1);
        }
    };

    let binary_path = match installer.extract_and_install(&temp_archive) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("\n❌ Installation failed: {}", e);
            std::process::exit(1);
        }
    };

    // Clean up temp archive
    let _ = std::fs::remove_file(&temp_archive);

    // Step 4: Configure PATH
    println!("\n🔧 Step 4/4: Configuring PATH...");
    let install_parent = binary_path.parent().expect("Binary has parent directory");
    if let Err(e) = path_setup::add_to_path(install_parent) {
        eprintln!("⚠ Warning: Could not automatically configure PATH: {:?}", e);
        eprintln!("   You can manually add this to your PATH:");
        eprintln!("   {}", install_parent.display());
    }

    // Success message
    println!("\n╔════════════════════════════════════════╗");
    println!("║    ✓ Installation Complete!            ║");
    println!("╚════════════════════════════════════════╝\n");

    println!("Binary installed at:");
    println!("  {}\n", binary_path.display());

    println!("Next steps:");
    if license_key.is_some() {
        println!("  1. Restart your terminal (or run: source ~/.bashrc)");
        println!("  2. Verify installation: kindly-av1 --version");
        println!("  3. Your license key is already configured!");
        println!("  4. Start encoding: kindly-av1 encode input.mp4 -o output.av1\n");
    } else {
        println!("  1. Restart your terminal (or run: source ~/.bashrc)");
        println!("  2. Activate your license: kindly-av1 license activate <KEY>");
        println!("  3. Start encoding: kindly-av1 encode input.mp4 -o output.av1\n");
    }

    println!("Get help:");
    println!("  Documentation: https://docs.kindly.dev/kindly-av1");
    println!("  Support: support@kindly.dev\n");

    println!("Thank you for choosing kindly-av1! 💜");
}

/// Mask license key for display (show first 4 and last 4 characters)
fn mask_license_key(key: &str) -> String {
    if key.len() <= 8 {
        return "*".repeat(key.len());
    }

    let first = &key[..4];
    let last = &key[key.len() - 4..];
    let middle = "*".repeat(key.len() - 8);

    format!("{}{}{}", first, middle, last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_license_key() {
        assert_eq!(
            mask_license_key("ABC123DEF456GHI789"),
            "ABC1**********I789"
        );

        assert_eq!(mask_license_key("SHORT"), "*****");

        assert_eq!(mask_license_key("12345678"), "********");
    }
}
