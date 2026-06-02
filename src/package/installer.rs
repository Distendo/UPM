use chrono::Local;
use std::path::{Path, PathBuf};

use crate::api::github::GithubApi;
use crate::config::Config;
use crate::database::{InstalledPackage, PackageDatabase};
use crate::downloader::Downloader;
use crate::errors::{Result, UpmError};
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
    ) -> Result<()> {
        logger.header(&format!("Installing {}", name));

        if db.is_installed(name) {
            return Err(UpmError::PackageAlreadyInstalled(name.to_string()));
        }

        let rp = rollback.create_point(db, &format!("Install {}", name))?;

        let deps = DependencyResolver::resolve(name, index, db)?;

        for dep in &deps {
            if !db.is_installed(&dep.name) {
                logger.step(&format!("Installing dependency: {} ({})", dep.name, dep.version));
                Box::pin(Self::install_single_package(
                    &dep.name,
                    config,
                    logger,
                    db,
                    github,
                    index,
                ))
                .await?;
            }
        }

        Self::install_single_package(name, config, logger, db, github, index).await?;

        rollback.finalize_point(&rp.id, db)?;
        logger.success(&format!("Successfully installed {}", name));

        Ok(())
    }

    async fn install_single_package(
        name: &str,
        config: &Config,
        logger: &Logger,
        db: &mut PackageDatabase,
        github: &GithubApi,
        index: &PackageIndex,
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

        let manifest = Self::fetch_manifest(name, github, index_pkg).await?;

        logger.step("Downloading package source...");
        let source_info = Self::download_source(name, &manifest, github, &work_dir, config).await?;

        logger.step("Verifying package integrity...");
        if let Some(ref expected_sha) = manifest.sha256 {
            verify::verify_checksum(&source_info.path, expected_sha)?;
            logger.success("Checksum verification passed");
        }

        logger.step("Installing package files...");
        let files = Self::install_files(name, &work_dir, &install_dir, &manifest, config)?;

        logger.step("Registering package in database...");
        let checksum = verify::sha256_file(&source_info.path).unwrap_or_default();
        let files_list: Vec<String> = files.iter().map(|f| f.to_string_lossy().to_string()).collect();

        let installed_pkg = InstalledPackage {
            name: name.to_string(),
            version: manifest.version.clone(),
            source: index_pkg.repository.clone(),
            install_path: install_dir,
            installed_at: Local::now().to_rfc3339(),
            files: files_list,
            checksum,
            manifest: manifest.clone(),
            dependencies: index_pkg.dependencies.clone(),
        };

        db.add_package(installed_pkg)?;

        let cache_file = config.cache_dir.join(format!("{}-{}.tgz", name, manifest.version));
        if cache_file.exists() {
            std::fs::remove_file(&cache_file).ok();
        }

        logger.success(&format!("{} {} installed successfully", name, manifest.version));

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
                    install: Some(vec!["install -d {{prefix}}/bin".to_string(), "cp files/* {{prefix}}/bin/".to_string()]),
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
                Err(UpmError::General("Git source type not yet implemented".into()))
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

        if dest.join("manifest.upm").exists() {
            return Ok(());
        }

        {
            let files_dir = dest.join("files");
            if files_dir.is_dir() {
                return Ok(());
            }
        }

        Ok(())
    }

    fn install_files(
        _name: &str,
        work_dir: &Path,
        install_dir: &Path,
        _manifest: &Manifest,
        _config: &Config,
    ) -> Result<Vec<PathBuf>> {
        std::fs::create_dir_all(install_dir)?;
        let mut installed_files = Vec::new();

        let files_dir = work_dir.join("files");
        if files_dir.exists() && files_dir.is_dir() {
            Self::copy_dir_recursive(&files_dir, install_dir, &mut installed_files)?;
        } else {
            Self::copy_dir_recursive(work_dir, install_dir, &mut installed_files)?;
        }

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

        let installed_dir = install_dir.parent().unwrap_or(install_dir);
        let symlink_dir = installed_dir.join("bin");
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

        Ok(installed_files)
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
