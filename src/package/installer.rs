use chrono::Local;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::api::github::GithubApi;
use crate::config::Config;
use crate::database::{InstalledPackage, PackageDatabase};
use crate::downloader::Downloader;
use crate::errors::{Result, UpmError};
use crate::groq::Groq;
use crate::logger::Logger;
use crate::package::index::PackageIndex;
use crate::package::manifest::{Manifest, SourceType};
use crate::package::resolver::DependencyResolver;
use crate::rollback::RollbackManager;
use crate::verify;

pub struct Installer;

impl Installer {
    pub async fn install_package(
        name: &str,
        config: &Config,
        logger: &Logger,
        db: &mut PackageDatabase,
        github: &GithubApi,
        index: &PackageIndex,
        rollback: &mut RollbackManager,
        use_ai: bool,
    ) -> Result<()> {
        logger.header(&format!("Installing {}", name));

        if db.is_installed(name) {
            return Err(UpmError::PackageAlreadyInstalled(name.to_string()));
        }

        Self::reinstall_package(name, config, logger, db, github, index, rollback, use_ai).await
    }

    pub async fn reinstall_package(
        name: &str,
        config: &Config,
        logger: &Logger,
        db: &mut PackageDatabase,
        github: &GithubApi,
        index: &PackageIndex,
        rollback: &mut RollbackManager,
        use_ai: bool,
    ) -> Result<()> {
        let label = if db.is_installed(name) { "Updating" } else { "Installing" };
        logger.header(&format!("{} {}", label, name));

        let rp = rollback.create_point(db, &format!("{} {}", label, name))?;

        // Check for circular dependencies before resolving
        match DependencyResolver::check_circular_dependencies(index, name) {
            Ok(cycles) if !cycles.is_empty() => {
                logger.warning(&format!("Circular dependencies detected for '{}':", name));
                for cycle in &cycles {
                    logger.warning(&format!("  {} -> {}", cycle.join(" -> "), cycle[0]));
                }
            }
            _ => {}
        }

        let deps = DependencyResolver::resolve(name, index, db)?;

        for dep in &deps {
            if !db.is_installed(&dep.name) {
                logger.step(&format!("Installing dependency: {} ({})", dep.name, dep.version));
                Box::pin(Self::install_single_package(
                    &dep.name, config, logger, db, github, index, false,
                ))
                .await?;
            }
        }

        Self::install_single_package(name, config, logger, db, github, index, use_ai).await?;

        rollback.finalize_point(&rp.id, db)?;
        logger.success(&format!("Successfully {} {}", label.to_lowercase(), name));

        Ok(())
    }

    async fn install_single_package(
        name: &str,
        config: &Config,
        logger: &Logger,
        db: &mut PackageDatabase,
        github: &GithubApi,
        index: &PackageIndex,
        use_ai: bool,
    ) -> Result<()> {
        let index_pkg = index
            .find_package(name)
            .ok_or_else(|| UpmError::PackageNotFound(name.to_string()))?;

        let install_dir = config.installed_dir.join(name);
        let work_dir = config.cache_dir.join(name);

        if work_dir.exists() {
            std::fs::remove_dir_all(&work_dir)?;
        }
        std::fs::create_dir_all(&work_dir)?;

        let result = Self::install_core(name, config, logger, db, github, index, use_ai, index_pkg, &install_dir, &work_dir).await;

        if result.is_err() && work_dir.exists() {
            std::fs::remove_dir_all(&work_dir).ok();
        }

        result
    }

    async fn install_core(
        name: &str,
        config: &Config,
        logger: &Logger,
        db: &mut PackageDatabase,
        github: &GithubApi,
        _index: &PackageIndex,
        use_ai: bool,
        index_pkg: &crate::package::index::IndexPackage,
        install_dir: &Path,
        work_dir: &Path,
    ) -> Result<()> {

        let manifest = Self::fetch_manifest(name, github, index_pkg).await?;

        logger.step("Downloading package source...");
        let source_info = Self::download_source(name, &manifest, github, &work_dir, config).await?;

        logger.step("Verifying package integrity...");
        if let Some(ref expected_sha) = manifest.sha256 {
            verify::verify_checksum(&source_info.path, expected_sha)?;
            logger.success("Checksum verification passed");
        }

        let groq = Groq::from_env();
        let has_manifest_commands = manifest.build.is_some() || manifest.install.is_some();
        let should_use_ai = use_ai || (groq.is_some() && !has_manifest_commands);

        if should_use_ai {
            if let Some(ref groq_client) = groq {
                logger.step("AI: Analyzing repository structure...");
                let file_list = Groq::list_directory_tree(&work_dir, 3)?;

                logger.step("AI: Generating custom build/install plan...");
                let plan = groq_client
                    .generate_build_plan(
                        name,
                        &manifest.source.url,
                        &file_list,
                        std::env::consts::OS,
                        &[],
                    )
                    .await?;

                logger.info(&format!("AI analysis: {}", plan.explanation));

                if let Some(deps) = plan.dependencies.first() {
                    if !deps.is_empty() {
                        logger.info(&format!("AI suggests system dependencies: {}", plan.dependencies.join(", ")));
                    }
                }

                if !plan.build.is_empty() {
                    logger.step("AI: Running build commands...");
                    Self::run_commands(&plan.build, &work_dir, config, logger)?;
                }

                if !plan.install.is_empty() {
                    logger.step("AI: Running install commands...");
                    std::fs::create_dir_all(&install_dir)?;
                    std::fs::create_dir_all(&install_dir.join("bin"))?;
                    let expanded: Vec<String> = plan
                        .install
                        .iter()
                        .map(|cmd| cmd.replace("{prefix}", &install_dir.to_string_lossy()))
                        .collect();
                    Self::run_commands(&expanded, &work_dir, config, logger)?;
                } else {
                    logger.step("AI: No install commands — copying files...");
                    Self::copy_to_install_dir(&work_dir, &install_dir, logger)?;
                }
            } else {
                logger.warning("--ai flag used but no GROQ_API_KEY set. Falling back to manifest commands.");
            }
        }

        if !should_use_ai || groq.is_none() {
            if let Some(ref build_cmds) = manifest.build {
                logger.step("Running build commands...");
                Self::run_commands(build_cmds, &work_dir, config, logger)?;
            }

            if let Some(ref install_cmds) = manifest.install {
                logger.step("Running install commands...");
                std::fs::create_dir_all(&install_dir)?;
                std::fs::create_dir_all(&install_dir.join("bin"))?;
                let expanded: Vec<String> = install_cmds
                    .iter()
                    .map(|cmd| cmd.replace("{prefix}", &install_dir.to_string_lossy()))
                    .collect();
                Self::run_commands(&expanded, &work_dir, config, logger)?;
            }
        }

        if !should_use_ai || groq.is_none() {
            if manifest.install.is_none() {
                logger.step("Installing package files...");
                Self::copy_to_install_dir(&work_dir, &install_dir, logger)?;
            }
        }

        logger.step("Registering package in database...");
        let checksum = verify::sha256_file(&source_info.path).unwrap_or_default();
        let files_list: Vec<String> = Self::collect_installed_files(&install_dir)?;

        let installed_pkg = InstalledPackage {
            name: name.to_string(),
            version: manifest.version.clone(),
            source: index_pkg.repository.clone(),
            install_path: install_dir.to_path_buf(),
            installed_at: Local::now().to_rfc3339(),
            files: files_list,
            checksum,
            manifest: manifest.clone(),
            dependencies: index_pkg.dependencies.clone(),
        };

        if db.is_installed(name) {
            db.remove_package(name)?;
        }
        db.add_package(installed_pkg)?;

        let cache_file = config.cache_dir.join(format!("{}-{}.tgz", name, manifest.version));
        if cache_file.exists() {
            std::fs::remove_file(&cache_file).ok();
        }

        logger.success(&format!("{} {} installed successfully", name, manifest.version));

        Ok(())
    }

    fn run_commands(commands: &[String], cwd: &Path, _config: &Config, logger: &Logger) -> Result<()> {
        for cmd in commands {
            let trimmed = cmd.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            logger.debug(&format!("$ {}", trimmed));

            let status = if cfg!(target_os = "windows") {
                Command::new("cmd")
                    .args(["/C", trimmed])
                    .current_dir(cwd)
                    .status()
                    .map_err(|e| UpmError::General(format!("Command failed: {trimmed}: {e}")))?
            } else {
                Command::new("sh")
                    .args(["-c", trimmed])
                    .current_dir(cwd)
                    .status()
                    .map_err(|e| UpmError::General(format!("Command failed: {trimmed}: {e}")))?
            };

            if !status.success() {
                return Err(UpmError::General(format!(
                    "Command exited with {}: {}",
                    status.code().unwrap_or(-1),
                    trimmed
                )));
            }
        }
        Ok(())
    }

    fn copy_to_install_dir(work_dir: &Path, install_dir: &Path, _logger: &Logger) -> Result<()> {
        std::fs::create_dir_all(install_dir)?;

        let files_dir = work_dir.join("files");
        let source = if files_dir.exists() && files_dir.is_dir() {
            files_dir
        } else {
            work_dir.to_path_buf()
        };

        let mut installed = Vec::new();
        Self::copy_dir_recursive(&source, install_dir, &mut installed)?;

        let bin_dir = install_dir.join("bin");
        if bin_dir.exists() {
            #[cfg(unix)]
            {
                if let Ok(entries) = std::fs::read_dir(&bin_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            use std::os::unix::fs::PermissionsExt;
                            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
                        }
                    }
                }
            }
        }

        let symlink_dir = install_dir.parent().unwrap_or(install_dir).join("bin");
        std::fs::create_dir_all(&symlink_dir).ok();
        if symlink_dir.exists() && bin_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&bin_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        let target = symlink_dir.join(entry.file_name());
                        if !target.exists() {
                            std::os::unix::fs::symlink(&path, &target).ok();
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn collect_installed_files(dir: &Path) -> Result<Vec<String>> {
        let mut files = Vec::new();
        if dir.exists() {
            Self::walk_files(dir, dir, &mut files)?;
        }
        Ok(files)
    }

    fn walk_files(base: &Path, dir: &Path, files: &mut Vec<String>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let relative = path.strip_prefix(base).unwrap_or(&path).to_string_lossy().to_string();
            if entry.file_type()?.is_dir() {
                Self::walk_files(base, &path, files)?;
            } else {
                files.push(relative);
            }
        }
        Ok(())
    }

    async fn fetch_manifest(
        name: &str,
        github: &GithubApi,
        index_pkg: &crate::package::index::IndexPackage,
    ) -> Result<Manifest> {
        let manifest_url = format!(
            "{}/raw/{}/manifest.upm",
            index_pkg.repository.trim_end_matches('/'),
            index_pkg.version
        );

        match github.client().download_bytes(&manifest_url).await {
            Ok(data) => {
                let content = String::from_utf8_lossy(&data);
                Manifest::parse(&content)
            }
            Err(_) => {
                let manifest = Manifest {
                    package: name.to_string(),
                    version: index_pkg.version.clone(),
                    description: Some(index_pkg.description.clone()),
                    license: index_pkg.license.clone(),
                    platforms: index_pkg.platforms.clone(),
                    source: crate::package::manifest::PackageSource {
                        url: index_pkg.repository.clone(),
                        source_type: SourceType::Github,
                        branch: None,
                        tag: Some(index_pkg.version.clone()),
                    },
                    dependencies: index_pkg
                        .dependencies
                        .iter()
                        .map(|d| crate::package::manifest::Dependency {
                            name: d.clone(),
                            version: None,
                            optional: None,
                        })
                        .collect(),
                    build: None,
                    install: None,
                    environment: None,
                    sha256: None,
                };
                Ok(manifest)
            }
        }
    }

    async fn download_source(
        name: &str,
        manifest: &Manifest,
        github: &GithubApi,
        work_dir: &Path,
        config: &Config,
    ) -> Result<SourceInfo> {
        use crate::package::manifest::SourceType;

        match manifest.source.source_type {
            SourceType::Github => {
                let parts: Vec<&str> = manifest.source.url.trim_end_matches('/').split('/').collect();
                let repo = parts.last().unwrap_or(&name);
                let owner = parts.get(parts.len().wrapping_sub(2)).unwrap_or(&"unknown");

                let tag = manifest.source.tag.as_deref().unwrap_or("latest");
                let dest = config.cache_dir.join(format!("{}-{}.tgz", name, manifest.version));

                let tarball_url = format!(
                    "https://api.github.com/repos/{}/{}/tarball/{}",
                    owner, repo, tag
                );

                let _data = github.client().download_file(&tarball_url, &dest).await?;
                Self::extract_tarball(&dest, work_dir)?;

                Ok(SourceInfo {
                    path: dest,
                    extracted_to: work_dir.to_path_buf(),
                })
            }
            SourceType::Git => {
                let url = &manifest.source.url;
                let branch = manifest.source.branch.as_deref().unwrap_or("master");

                // Remove work_dir since git clone needs target not to exist
                if work_dir.exists() {
                    std::fs::remove_dir_all(work_dir)?;
                }

                let status = Command::new("git")
                    .args(["clone", "--depth", "1", "--branch", branch, url, work_dir.to_str().unwrap_or("")])
                    .status()
                    .map_err(|e| UpmError::General(format!("git clone failed: {e}")))?;

                if !status.success() {
                    return Err(UpmError::General(format!("git clone failed for {}", url)));
                }

                // Remove .git to save space
                let git_dir = work_dir.join(".git");
                if git_dir.exists() {
                    std::fs::remove_dir_all(git_dir).ok();
                }

                Ok(SourceInfo {
                    path: work_dir.to_path_buf(),
                    extracted_to: work_dir.to_path_buf(),
                })
            }
            SourceType::Direct => {
                let downloader = Downloader::new(4);
                let dest = config.cache_dir.join(format!("{}-{}", name, manifest.version));
                downloader.download_file(&manifest.source.url, &dest).await?;
                Ok(SourceInfo {
                    path: dest,
                    extracted_to: work_dir.to_path_buf(),
                })
            }
        }
    }

    fn extract_tarball(tarball: &Path, dest: &Path) -> Result<()> {
        let file = std::fs::File::open(tarball)?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);

        archive.unpack(dest)?;

        let entries = std::fs::read_dir(dest)?;
        let subdirs: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .collect();

        if subdirs.len() == 1 {
            let subdir = subdirs[0].path();
            let files: Vec<_> = std::fs::read_dir(&subdir)?
                .filter_map(|e| e.ok())
                .collect();
            for entry in files {
                let file_name = entry.file_name();
                let target = dest.join(&file_name);
                std::fs::rename(entry.path(), &target)?;
            }
            std::fs::remove_dir_all(&subdir)?;
        }

        Ok(())
    }

    fn copy_dir_recursive(
        src: &Path,
        dst: &Path,
        installed_files: &mut Vec<PathBuf>,
    ) -> Result<()> {
        if !src.exists() {
            return Ok(());
        }

        std::fs::create_dir_all(dst)?;

        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let file_name = entry.file_name();
            let src_path = entry.path();
            let dst_path = dst.join(&file_name);

            if file_type.is_dir() {
                Self::copy_dir_recursive(&src_path, &dst_path, installed_files)?;
            } else {
                std::fs::copy(&src_path, &dst_path)?;
                installed_files.push(dst_path);
            }
        }

        Ok(())
    }
}

struct SourceInfo {
    path: PathBuf,
    extracted_to: PathBuf,
}
