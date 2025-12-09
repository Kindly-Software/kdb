// install.rs - T9 Persistent Install Capsule
// UCE34 Q10 T9 Persistent tier - Extract archive, verify binary, set permissions

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Installation errors
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("Cannot write to install directory.")]
    PermissionDenied(#[source] io::Error),

    #[error("Archive extraction failed: {0}")]
    ExtractionFailed(String),

    #[error("Binary not found in archive")]
    BinaryNotFound,

    #[error("Not enough disk space.")]
    DiskFull(#[source] io::Error),
}

/// Install capsule state (T9 Persistent)
pub struct InstallCapsule {
    install_dir: PathBuf,
}

impl InstallCapsule {
    /// Create new install capsule with default install directory
    pub fn new() -> Result<Self, InstallError> {
        let install_dir = Self::default_install_dir()?;
        Ok(Self { install_dir })
    }

    /// Create install capsule with custom directory
    pub fn with_dir(install_dir: PathBuf) -> Self {
        Self { install_dir }
    }

    /// Get default install directory by platform
    fn default_install_dir() -> Result<PathBuf, InstallError> {
        #[cfg(unix)]
        {
            let home = dirs::home_dir()
                .ok_or_else(|| InstallError::PermissionDenied(
                    io::Error::new(io::ErrorKind::NotFound, "HOME not set")
                ))?;
            Ok(home.join(".local/bin"))
        }

        #[cfg(windows)]
        {
            let local_app_data = dirs::data_local_dir()
                .ok_or_else(|| InstallError::PermissionDenied(
                    io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA not set")
                ))?;
            Ok(local_app_data.join("kindly-av1"))
        }
    }

    /// Extract and install from archive
    pub fn extract_and_install(&self, archive_path: &Path) -> Result<PathBuf, InstallError> {
        // Create install directory
        fs::create_dir_all(&self.install_dir)
            .map_err(Self::classify_io_error)?;

        // Determine archive type and extract
        let extension = archive_path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        match extension {
            "gz" => self.extract_tar_gz(archive_path)?,
            #[cfg(windows)]
            "zip" => self.extract_zip(archive_path)?,
            _ => return Err(InstallError::ExtractionFailed(
                format!("Unsupported archive format: {}", extension)
            )),
        }

        // Verify binary exists
        let binary_path = self.get_binary_path();
        if !binary_path.exists() {
            return Err(InstallError::BinaryNotFound);
        }

        // Set executable permissions on Unix
        #[cfg(unix)]
        self.set_executable(&binary_path)?;

        println!("[kindly-av1] Installation complete: {}", binary_path.display());
        Ok(binary_path)
    }

    /// Extract .tar.gz archive
    fn extract_tar_gz(&self, archive_path: &Path) -> Result<(), InstallError> {
        let file = File::open(archive_path)
            .map_err(Self::classify_io_error)?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);

        archive.unpack(&self.install_dir)
            .map_err(|e| InstallError::ExtractionFailed(e.to_string()))?;

        Ok(())
    }

    /// Extract .zip archive (Windows only)
    #[cfg(windows)]
    fn extract_zip(&self, archive_path: &Path) -> Result<(), InstallError> {
        let file = File::open(archive_path)
            .map_err(Self::classify_io_error)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| InstallError::ExtractionFailed(e.to_string()))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .map_err(|e| InstallError::ExtractionFailed(e.to_string()))?;
            let outpath = self.install_dir.join(file.mangled_name());

            if file.is_dir() {
                fs::create_dir_all(&outpath)
                    .map_err(Self::classify_io_error)?;
            } else {
                if let Some(p) = outpath.parent() {
                    fs::create_dir_all(p)
                        .map_err(Self::classify_io_error)?;
                }
                let mut outfile = File::create(&outpath)
                    .map_err(Self::classify_io_error)?;
                io::copy(&mut file, &mut outfile)
                    .map_err(Self::classify_io_error)?;
            }
        }

        Ok(())
    }

    /// Get expected binary path
    fn get_binary_path(&self) -> PathBuf {
        #[cfg(windows)]
        return self.install_dir.join("kindly-av1.exe");

        #[cfg(unix)]
        return self.install_dir.join("kindly-av1");
    }

    /// Set executable permissions on Unix
    #[cfg(unix)]
    fn set_executable(&self, path: &Path) -> Result<(), InstallError> {
        let mut perms = fs::metadata(path)
            .map_err(Self::classify_io_error)?
            .permissions();
        perms.set_mode(0o755); // rwxr-xr-x
        fs::set_permissions(path, perms)
            .map_err(Self::classify_io_error)?;
        Ok(())
    }

    /// Classify I/O errors
    fn classify_io_error(err: io::Error) -> InstallError {
        match err.kind() {
            io::ErrorKind::PermissionDenied => InstallError::PermissionDenied(err),
            io::ErrorKind::OutOfMemory => InstallError::DiskFull(err),
            _ => {
                #[cfg(unix)]
                if err.raw_os_error() == Some(28) { // ENOSPC
                    return InstallError::DiskFull(err);
                }

                InstallError::PermissionDenied(err)
            }
        }
    }

    /// Get install directory
    pub fn install_dir(&self) -> &Path {
        &self.install_dir
    }
}

impl Default for InstallCapsule {
    fn default() -> Self {
        Self::new().expect("Failed to determine install directory")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_capsule_creation() {
        let capsule = InstallCapsule::new().unwrap();
        assert!(capsule.install_dir.is_absolute());
    }

    #[test]
    fn test_binary_path() {
        let capsule = InstallCapsule::new().unwrap();
        let binary_path = capsule.get_binary_path();

        #[cfg(windows)]
        assert!(binary_path.to_str().unwrap().ends_with("kindly-av1.exe"));

        #[cfg(unix)]
        assert!(binary_path.to_str().unwrap().ends_with("kindly-av1"));
    }
}
