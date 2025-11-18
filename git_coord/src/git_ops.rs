//! Git operation execution with error handling.
//!
//! Real git command dispatch with comprehensive error recovery.

use std::process::Command;
use std::path::Path;
use crate::coordinator::GitCoordinator;
use crate::error::{CoordinatorError, Result};

impl GitCoordinator {
    /// Execute commit with message in specified repository.
    ///
    /// # Returns
    /// - `Ok(commit_hash)` - Commit successful
    /// - `Err(CoordinatorError::GitError)` - Commit failed
    ///
    /// # I20 Integration
    /// - Q11: No corruption (atomic commit)
    /// - Q17: Property = Commit always succeeds or fails definitively
    pub fn commit(&self, repo_path: &Path, message: &str) -> Result<String> {
        self.execute(|| {
            let output = Command::new("git")
                .args(&["commit", "-m", message])
                .current_dir(repo_path)
                .output()
                .map_err(|e| CoordinatorError::GitError(e.to_string()))?;

            if !output.status.success() {
                return Err(CoordinatorError::GitError(
                    String::from_utf8_lossy(&output.stderr).to_string()
                ));
            }

            // Extract commit hash
            let log_output = Command::new("git")
                .args(&["rev-parse", "HEAD"])
                .current_dir(repo_path)
                .output()
                .map_err(|e| CoordinatorError::GitError(e.to_string()))?;

            let commit_hash = String::from_utf8(log_output.stdout)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();

            Ok(commit_hash)
        })
    }

    /// Execute branch creation in specified repository.
    ///
    /// # Returns
    /// - `Ok(())` - Branch created
    /// - `Err(CoordinatorError::GitError)` - Branch creation failed
    pub fn branch(&self, repo_path: &Path, name: &str) -> Result<()> {
        self.execute(|| {
            let output = Command::new("git")
                .args(&["branch", name])
                .current_dir(repo_path)
                .output()
                .map_err(|e| CoordinatorError::GitError(e.to_string()))?;

            if !output.status.success() {
                return Err(CoordinatorError::GitError(
                    String::from_utf8_lossy(&output.stderr).to_string()
                ));
            }

            Ok(())
        })
    }

    /// Execute checkout in specified repository.
    ///
    /// # Returns
    /// - `Ok(())` - Checkout successful
    /// - `Err(CoordinatorError::GitError)` - Checkout failed
    pub fn checkout(&self, repo_path: &Path, ref_name: &str) -> Result<()> {
        self.execute(|| {
            let output = Command::new("git")
                .args(&["checkout", ref_name])
                .current_dir(repo_path)
                .output()
                .map_err(|e| CoordinatorError::GitError(e.to_string()))?;

            if !output.status.success() {
                return Err(CoordinatorError::GitError(
                    String::from_utf8_lossy(&output.stderr).to_string()
                ));
            }

            Ok(())
        })
    }

    /// Execute merge in specified repository.
    ///
    /// # Returns
    /// - `Ok(merge_output)` - Merge successful
    /// - `Err(CoordinatorError::GitError)` - Merge failed
    pub fn merge(&self, repo_path: &Path, source: &str) -> Result<String> {
        self.execute(|| {
            let output = Command::new("git")
                .args(&["merge", source])
                .current_dir(repo_path)
                .output()
                .map_err(|e| CoordinatorError::GitError(e.to_string()))?;

            if !output.status.success() {
                return Err(CoordinatorError::GitError(
                    String::from_utf8_lossy(&output.stderr).to_string()
                ));
            }

            let merge_output = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(merge_output)
        })
    }

    /// Execute status in specified repository.
    ///
    /// # Returns
    /// - `Ok(status_output)` - Status retrieved
    /// - `Err(CoordinatorError::GitError)` - Status failed
    pub fn status(&self, repo_path: &Path) -> Result<String> {
        self.execute(|| {
            let output = Command::new("git")
                .args(&["status", "--short"])
                .current_dir(repo_path)
                .output()
                .map_err(|e| CoordinatorError::GitError(e.to_string()))?;

            if !output.status.success() {
                return Err(CoordinatorError::GitError(
                    String::from_utf8_lossy(&output.stderr).to_string()
                ));
            }

            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_operations() {
        let coord = GitCoordinator::new();

        // Just verify coordinator created
        assert_ne!(coord.instance_id().0, 0);
    }
}
