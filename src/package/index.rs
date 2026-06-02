use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::downloader::Downloader;
use crate::errors::{Result, UpmError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: String,
    pub repository: String,
    pub license: Option<String>,
    pub dependencies: Vec<String>,
    pub sha256: Option<String>,
    pub platforms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageIndex {
    pub name: String,
    pub description: String,
    pub packages: HashMap<String, IndexPackage>,
}

impl PackageIndex {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            packages: HashMap::new(),
        }
    }

    pub fn load(path: &std::path::Path) -> Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let index: PackageIndex = serde_json::from_str(&content)?;
            Ok(index)
        } else {
            Err(UpmError::General(format!("Index file not found: {}", path.display())))
        }
    }

    pub async fn load_remote(url: &str, downloader: &Downloader) -> Result<Self> {
        let data: PackageIndex = downloader.download_json(url).await?;
        Ok(data)
    }

    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn find_package(&self, name: &str) -> Option<&IndexPackage> {
        self.packages.get(name)
    }

    pub fn search(&self, query: &str) -> Vec<&IndexPackage> {
        let q = query.to_lowercase();
        self.packages
            .values()
            .filter(|p| {
                p.name.to_lowercase().contains(&q)
                    || p.description.to_lowercase().contains(&q)
            })
            .collect()
    }

    pub fn add_package(&mut self, pkg: IndexPackage) {
        self.packages.insert(pkg.name.clone(), pkg);
    }

    pub fn package_count(&self) -> usize {
        self.packages.len()
    }

    pub async fn update_from_github(
        &mut self,
        downloader: &Downloader,
        repo_url: &str,
    ) -> Result<()> {
        let data: PackageIndex = downloader.download_json(repo_url).await?;
        self.packages = data.packages;
        Ok(())
    }
}
