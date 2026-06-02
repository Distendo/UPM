use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::errors::{Result, UpmError};
use crate::package::manifest::Manifest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    pub source: String,
    pub install_path: PathBuf,
    pub installed_at: String,
    pub files: Vec<String>,
    pub checksum: String,
    pub manifest: Manifest,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageDatabase {
    pub packages: HashMap<String, InstalledPackage>,
    pub db_path: PathBuf,
}

impl PackageDatabase {
    pub fn new(installed_dir: PathBuf) -> Self {
        Self {
            packages: HashMap::new(),
            db_path: installed_dir.join("db.json"),
        }
    }

    pub fn load(installed_dir: PathBuf) -> Result<Self> {
        let db_path = installed_dir.join("db.json");
        if db_path.exists() {
            let content = std::fs::read_to_string(&db_path)?;
            let packages: HashMap<String, InstalledPackage> = serde_json::from_str(&content)?;
            Ok(Self { packages, db_path })
        } else {
            std::fs::create_dir_all(&installed_dir)?;
            Ok(Self {
                packages: HashMap::new(),
                db_path,
            })
        }
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.packages)?;
        std::fs::write(&self.db_path, content)?;
        Ok(())
    }

    pub fn add_package(&mut self, pkg: InstalledPackage) -> Result<()> {
        let name = pkg.name.clone();
        if self.packages.contains_key(&name) {
            return Err(UpmError::PackageAlreadyInstalled(name));
        }
        self.packages.insert(name, pkg);
        self.save()?;
        Ok(())
    }

    pub fn remove_package(&mut self, name: &str) -> Result<InstalledPackage> {
        self.packages
            .remove(name)
            .ok_or_else(|| UpmError::PackageNotInstalled(name.to_string()))
    }

    pub fn get_package(&self, name: &str) -> Option<&InstalledPackage> {
        self.packages.get(name)
    }

    pub fn list_packages(&self) -> Vec<&InstalledPackage> {
        let mut pkgs: Vec<_> = self.packages.values().collect();
        pkgs.sort_by(|a, b| a.name.cmp(&b.name));
        pkgs
    }

    pub fn is_installed(&self, name: &str) -> bool {
        self.packages.contains_key(name)
    }

    pub fn update_package(&mut self, pkg: InstalledPackage) -> Result<()> {
        let name = pkg.name.clone();
        self.packages.insert(name, pkg);
        self.save()?;
        Ok(())
    }

    pub fn count(&self) -> usize {
        self.packages.len()
    }
}
