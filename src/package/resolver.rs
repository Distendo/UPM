use std::collections::{HashSet, VecDeque};

use crate::database::PackageDatabase;
use crate::errors::{Result, UpmError};
use crate::package::index::PackageIndex;
use crate::package::manifest::Manifest;

pub struct DependencyResolver;

#[derive(Debug, Clone)]
pub struct ResolvedDependency {
    pub name: String,
    pub version: String,
    pub source: String,
    pub manifest: Manifest,
    pub depth: usize,
}

impl DependencyResolver {
    pub fn resolve(
        package_name: &str,
        index: &PackageIndex,
        db: &PackageDatabase,
    ) -> Result<Vec<ResolvedDependency>> {
        let mut resolved = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        let index_pkg = index
            .find_package(package_name)
            .ok_or_else(|| UpmError::PackageNotFound(package_name.to_string()))?;

        visited.insert(index_pkg.name.clone());

        for dep in &index_pkg.dependencies {
            if !visited.contains(dep) {
                queue.push_back((dep.clone(), 0usize));
            }
        }

        while let Some((name, depth)) = queue.pop_front() {
            if !visited.insert(name.clone()) {
                continue;
            }

            let pkg = index
                .find_package(&name)
                .ok_or_else(|| UpmError::DependencyError(format!("Package '{}' not found in index", name)))?;

            // Still queue transitive deps even if installed, so children get installed
            for dep in &pkg.dependencies {
                if !visited.contains(dep) {
                    queue.push_back((dep.clone(), depth + 1));
                }
            }

            if db.is_installed(&name) {
                continue;
            }

            let manifest = Manifest {
                package: pkg.name.clone(),
                version: pkg.version.clone(),
                description: Some(pkg.description.clone()),
                license: pkg.license.clone(),
                platforms: pkg.platforms.clone(),
                source: crate::package::manifest::PackageSource {
                    url: pkg.repository.clone(),
                    source_type: crate::package::manifest::SourceType::Github,
                    branch: None,
                    tag: Some(pkg.version.clone()),
                },
                dependencies: pkg
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
                sha256: pkg.sha256.clone(),
            };

            for dep in &pkg.dependencies {
                if !visited.contains(dep) {
                    queue.push_back((dep.clone(), depth + 1));
                }
            }

            resolved.push(ResolvedDependency {
                name: pkg.name.clone(),
                version: pkg.version.clone(),
                source: pkg.repository.clone(),
                manifest,
                depth,
            });
        }

        resolved.sort_by_key(|d| d.depth);

        Ok(resolved)
    }

    pub fn check_circular_dependencies(
        index: &PackageIndex,
        package_name: &str,
    ) -> Result<Vec<Vec<String>>> {
        let mut cycles = Vec::new();

        fn dfs(
            current: &str,
            path: &mut Vec<String>,
            visited: &mut HashSet<String>,
            index: &PackageIndex,
            cycles: &mut Vec<Vec<String>>,
        ) {
            if let Some(pos) = path.iter().position(|p| p == current) {
                cycles.push(path[pos..].to_vec());
                return;
            }

            if !visited.insert(current.to_string()) {
                return;
            }

            path.push(current.to_string());

            if let Some(pkg) = index.find_package(current) {
                for dep in &pkg.dependencies {
                    dfs(dep, path, visited, index, cycles);
                }
            }

            path.pop();
        }

        dfs(
            package_name,
            &mut Vec::new(),
            &mut HashSet::new(),
            index,
            &mut cycles,
        );

        Ok(cycles)
    }
}
