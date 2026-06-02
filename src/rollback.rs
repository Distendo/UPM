use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::errors::Result;
use crate::database::PackageDatabase;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackPoint {
    pub id: String,
    pub timestamp: String,
    pub packages_before: HashMap<String, String>,
    pub packages_after: HashMap<String, String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackManager {
    pub rollbacks: Vec<RollbackPoint>,
    pub db_dir: PathBuf,
}

impl RollbackManager {
    pub fn new(upm_dir: PathBuf) -> Self {
        Self {
            rollbacks: Vec::new(),
            db_dir: upm_dir.join("rollbacks"),
        }
    }

    pub fn load(upm_dir: PathBuf) -> Result<Self> {
        let db_dir = upm_dir.join("rollbacks");
        let path = db_dir.join("rollback.json");
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let rollbacks: Vec<RollbackPoint> = serde_json::from_str(&content)?;
            Ok(Self { rollbacks, db_dir })
        } else {
            std::fs::create_dir_all(&db_dir)?;
            Ok(Self {
                rollbacks: Vec::new(),
                db_dir,
            })
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = self.db_dir.join("rollback.json");
        std::fs::create_dir_all(&self.db_dir)?;
        let content = serde_json::to_string_pretty(&self.rollbacks)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn create_point(
        &mut self,
        db: &PackageDatabase,
        description: &str,
    ) -> Result<RollbackPoint> {
        let before: HashMap<String, String> = db
            .list_packages()
            .iter()
            .map(|p| (p.name.clone(), p.version.clone()))
            .collect();

        let point = RollbackPoint {
            id: format!("rp_{}_{:04}", Local::now().format("%Y%m%d_%H%M%S"), self.rollbacks.len()),
            timestamp: Local::now().to_rfc3339(),
            packages_before: before,
            packages_after: HashMap::new(),
            description: description.to_string(),
        };

        self.rollbacks.push(point.clone());
        self.save()?;
        Ok(point)
    }

    pub fn finalize_point(&mut self, point_id: &str, db: &PackageDatabase) -> Result<()> {
        if let Some(point) = self.rollbacks.iter_mut().rev().find(|p| p.id == point_id) {
            let after: HashMap<String, String> = db
                .list_packages()
                .iter()
                .map(|p| (p.name.clone(), p.version.clone()))
                .collect();
            point.packages_after = after;
            self.save()?;
        }
        Ok(())
    }

    pub fn list(&self) -> &[RollbackPoint] {
        &self.rollbacks
    }
}
