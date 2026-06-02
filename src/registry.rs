use std::path::PathBuf;

use crate::config::Config;
use crate::errors::{Result, UpmError};
use crate::logger::Logger;
use crate::package::index::{IndexPackage, PackageIndex};

pub struct Registry;

impl Registry {
    pub fn add(
        name: &str,
        repo_url: &str,
        version: &str,
        description: &str,
        license: &str,
        config: &Config,
        logger: &Logger,
        assume_yes: bool,
    ) -> Result<()> {
        logger.header(&format!("Register: {}", name));

        let index_path = Self::local_index_path(config);
        let mut index = if index_path.exists() {
            PackageIndex::load(&index_path)?
        } else {
            PackageIndex::new("UPM Official Index", "Official package index for Universal Package Manager")
        };

        if index.find_package(name).is_some() {
            return Err(UpmError::General(format!("Package '{}' is already registered in the index", name)));
        }

        let pkg = IndexPackage {
            name: name.to_string(),
            version: version.to_string(),
            description: description.to_string(),
            source: repo_url.to_string(),
            repository: repo_url.to_string(),
            license: Some(license.to_string()),
            dependencies: Vec::new(),
            sha256: None,
            platforms: vec!["linux".into(), "macos".into(), "windows".into(), "bsd".into()],
        };

        index.add_package(pkg);
        index.save(&index_path)?;
        logger.success(&format!("{} v{} added to local index", name, version));

        let repo_root = Self::find_repo_root()?;
        let repo_index = repo_root.join("index").join("official.json");
        if repo_index.exists() {
            let mut repo_idx = PackageIndex::load(&repo_index)?;
            let pkg = index.find_package(name)
                .ok_or_else(|| UpmError::General(format!("Package '{}' not found after adding", name)))?
                .clone();
            repo_idx.add_package(pkg);
            repo_idx.save(&repo_index)?;
            logger.success(&format!("{} v{} added to repo index", name, version));

            if Self::git_available() && Self::is_git_repo(&repo_root) {
                if assume_yes {
                    Self::git_commit_and_push(&repo_root, name, logger)?;
                } else {
                    logger.step("Commit and push to GitHub?");
                    logger.info("Use --yes to auto-push, or do it manually:");
                    logger.info(&format!("  cd {} && git add . && git commit -m \"Add package {}\" && git push", repo_root.display(), name));
                }
            }
        }

        Ok(())
    }

    fn local_index_path(config: &Config) -> PathBuf {
        config.config_dir.join("official.json")
    }

    fn find_repo_root() -> Result<PathBuf> {
        let cwd = std::env::current_dir().map_err(|e| UpmError::General(format!("Cannot get cwd: {e}")))?;
        let mut dir = Some(cwd.as_path());
        while let Some(d) = dir {
            if d.join("index").join("official.json").exists() {
                return Ok(d.to_path_buf());
            }
            dir = d.parent();
        }
        Err(UpmError::General("Not inside the UPM repository (index/official.json not found)".into()))
    }

    fn git_available() -> bool {
        std::process::Command::new("git")
            .arg("--version")
            .output()
            .is_ok()
    }

    fn is_git_repo(path: &std::path::Path) -> bool {
        path.join(".git").exists()
    }

    fn git_commit_and_push(repo_root: &std::path::Path, name: &str, logger: &Logger) -> Result<()> {
        logger.step("Committing and pushing to GitHub...");

        let status = std::process::Command::new("git")
            .args(["add", "index/official.json"])
            .current_dir(repo_root)
            .status()
            .map_err(|e| UpmError::General(format!("git add failed: {e}")))?;

        if !status.success() {
            return Err(UpmError::General("git add failed".into()));
        }

        let output = std::process::Command::new("git")
            .args(["commit", "-m", &format!("Add package {}", name)])
            .current_dir(repo_root)
            .output()
            .map_err(|e| UpmError::General(format!("git commit failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("nothing to commit") || stderr.contains("nothing added") {
                logger.warning("Nothing to commit (already up to date)");
            } else {
                logger.warning(&format!("git commit warning: {}", stderr.trim()));
            }
            return Ok(());
        }

        let status = std::process::Command::new("git")
            .args(["push"])
            .current_dir(repo_root)
            .status()
            .map_err(|e| UpmError::General(format!("git push failed: {e}")))?;

        if status.success() {
            logger.success("Committed and pushed to GitHub");
        } else {
            logger.warning("Commit succeeded but push failed. Push manually.");
        }

        Ok(())
    }
}
