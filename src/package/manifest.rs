use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::errors::{Result, UpmError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Github,
    Git,
    Direct,
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceType::Github => write!(f, "github"),
            SourceType::Git => write!(f, "git"),
            SourceType::Direct => write!(f, "direct"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub package: String,
    pub version: String,
    pub description: Option<String>,
    pub license: Option<String>,
    pub platforms: Vec<String>,
    pub source: PackageSource,
    pub dependencies: Vec<Dependency>,
    pub build: Option<Vec<String>>,
    pub install: Option<Vec<String>>,
    pub environment: Option<HashMap<String, String>>,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSource {
    pub url: String,
    pub source_type: SourceType,
    pub branch: Option<String>,
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version: Option<String>,
    pub optional: Option<bool>,
}

impl Manifest {
    pub fn parse(content: &str) -> Result<Self> {
        let parsed: Manifest = serde_json::from_str(content)?;
        parsed.validate()?;
        Ok(parsed)
    }

    pub fn parse_file(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| UpmError::ManifestParseError(format!("Cannot read manifest: {e}")))?;
        Self::parse(&content)
    }

    fn validate(&self) -> Result<()> {
        if self.package.is_empty() {
            return Err(UpmError::ManifestParseError("Package name is empty".into()));
        }
        if self.version.is_empty() {
            return Err(UpmError::ManifestParseError("Package version is empty".into()));
        }
        let current_os = std::env::consts::OS;
        let os_name = match current_os {
            "linux" => "linux",
            "windows" => "windows",
            "macos" => "macos",
            "freebsd" | "netbsd" | "openbsd" => "bsd",
            _ => "unknown",
        };

        if !self.platforms.iter().any(|p| p.to_lowercase() == os_name || p.to_lowercase() == "all") {
            return Err(UpmError::UnsupportedPlatform(format!(
                "Package '{}' does not support platform '{}'. Supported: {}",
                self.package,
                current_os,
                self.platforms.join(", ")
            )));
        }

        Ok(())
    }

    pub fn serialize(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn supports_current_platform(&self) -> bool {
        let os = std::env::consts::OS;
        let os_name = match os {
            "linux" => "linux",
            "windows" => "windows",
            "macos" => "macos",
            _ => "bsd",
        };
        self.platforms.iter().any(|p| p.to_lowercase() == os_name || p.to_lowercase() == "all")
    }
}
