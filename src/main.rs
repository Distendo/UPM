#![allow(dead_code)]

mod api;
mod cli;
mod config;
mod database;
mod doctor;
mod downloader;
mod errors;
mod logger;
mod package;
mod rollback;
mod verify;

use std::sync::Arc;

use clap::Parser;
use colored::*;

use api::github::GithubApi;
use cli::{Cli, Commands};
use config::Config;
use database::PackageDatabase;
use doctor::Doctor;
use downloader::Downloader;
use errors::Result;
use logger::Logger;
use package::index::PackageIndex;
use package::installer::Installer;
use rollback::RollbackManager;

fn print_welcome(logger: &Logger) {
    let version = env!("CARGO_PKG_VERSION");
    println!(
        "{}",
        format!(
            "╔══════════════════════════════════════════╗\n\
             ║         UPM v{} - Package Manager        ║\n\
             ║     Universal Package Manager v{}       ║\n\
             ╚══════════════════════════════════════════╝",
            version, version
        )
        .bright_blue()
        .bold()
    );
    logger.debug(&format!("Platform: {} {}", std::env::consts::OS, std::env::consts::ARCH));
}

fn print_package_list(packages: &[&database::InstalledPackage], logger: &Logger) {
    if packages.is_empty() {
        logger.info("No packages installed.");
        return;
    }

    println!(
        "{}",
        format!(
            "{:<25} {:<15} {:<20} {}",
            "Package", "Version", "Installed At", "Source"
        )
        .bright_cyan()
        .bold()
    );
    println!("{}", "─".repeat(80).dimmed());

    for pkg in packages {
        println!(
            "  {:<23} {:<15} {:<20} {}",
            pkg.name.bright_white(),
            pkg.version.green(),
            &pkg.installed_at[..10].yellow(),
            pkg.source.blue().dimmed()
        );
    }

    println!("\n{} packages installed\n", packages.len().to_string().bright_green());
}

fn print_search_results(results: &[&package::index::IndexPackage], query: &str, logger: &Logger) {
    if results.is_empty() {
        logger.info(&format!("No packages found for '{}'", query));
        return;
    }

    println!(
        "{} {}",
        "Search results for:".bright_white(),
        query.bright_cyan()
    );
    println!("{}", "─".repeat(80).dimmed());

    for pkg in results {
        println!(
            "  {} v{}",
            pkg.name.bright_green().bold(),
            pkg.version.bright_white()
        );
        println!("    {}", pkg.description.dimmed());
        println!("    {} {}\n", "Source:".dimmed(), pkg.repository.blue().dimmed());
    }

    logger.info(&format!("Found {} packages", results.len()));
}

fn resolve_index_path(config: &Config) -> std::path::PathBuf {
    config.config_dir.join("package_index.json")
}

async fn load_or_fetch_index(
    config: &Config,
    downloader: &Downloader,
    logger: &Logger,
) -> Result<PackageIndex> {
    let index_path = resolve_index_path(config);

    if index_path.exists() {
        logger.debug("Loading cached package index...");
        match PackageIndex::load(&index_path) {
            Ok(index) => return Ok(index),
            Err(e) => logger.warning(&format!("Failed to load cached index: {e}")),
        }
    }

    logger.step("Fetching package index from GitHub...");
    match PackageIndex::load_remote(&config.index_url, downloader).await {
        Ok(index) => {
            index.save(&index_path)?;
            logger.success(&format!("Loaded {} packages", index.package_count()));
            Ok(index)
        }
        Err(e) => {
            logger.warning(&format!("Could not fetch remote index: {e}"));
            logger.info("Using empty index. You can install packages by specifying full GitHub URL.");
            Ok(PackageIndex::new("local", "Local package index"))
        }
    }
}

fn handle_remove(package: &str, db: &mut PackageDatabase, logger: &Logger, rollback: &mut RollbackManager) {
    logger.header(&format!("Remove: {}", package));
    if !db.is_installed(package) {
        logger.error(&format!("Package '{}' is not installed", package));
        return;
    }

    let rp = match rollback.create_point(db, &format!("Remove {}", package)) {
        Ok(r) => r,
        Err(e) => {
            logger.error(&format!("Rollback error: {e}"));
            return;
        }
    };

    let installed = match db.remove_package(package) {
        Ok(p) => p,
        Err(e) => {
            logger.error(&format!("Failed to remove from database: {e}"));
            return;
        }
    };

    if installed.install_path.exists() {
        if std::fs::remove_dir_all(&installed.install_path).is_err() {
            logger.warning("Could not remove all package files");
        }
    }

    if let Err(e) = rollback.finalize_point(&rp.id, db) {
        logger.warning(&format!("Rollback finalize error: {e}"));
    }

    logger.success(&format!("Removed package '{}'", package));
}

fn handle_clean(config: &Config, logger: &Logger) {
    logger.header("Cleaning Cache");
    let cache_dir = &config.cache_dir;
    if cache_dir.exists() {
        let count = std::fs::read_dir(cache_dir)
            .map(|e| e.count())
            .unwrap_or(0);
        std::fs::remove_dir_all(cache_dir).ok();
        std::fs::create_dir_all(cache_dir).ok();
        logger.success(&format!("Cleaned {} cache entries", count));
    } else {
        logger.info("Cache is already empty.");
    }
}

fn handle_info(package: &str, db: &PackageDatabase, index: &PackageIndex, logger: &Logger) {
    if let Some(installed) = db.get_package(package) {
        println!("{}", format!("\n{} v{}", installed.name, installed.version).bright_green().bold());
        println!("{}", "─".repeat(50).dimmed());
        println!("  {} {}", "Description:".dimmed(), installed.manifest.description.as_deref().unwrap_or("N/A"));
        println!("  {} {}", "License:".dimmed(), installed.manifest.license.as_deref().unwrap_or("N/A"));
        println!("  {} {}", "Source:".dimmed(), installed.source.dimmed());
        println!("  {} {}", "Installed:".dimmed(), &installed.installed_at[..19].yellow());
        println!("  {} {}", "Path:".dimmed(), installed.install_path.to_string_lossy().dimmed());
        println!("  {} {}", "Files:".dimmed(), installed.files.len().to_string().cyan());
        let deps = if installed.dependencies.is_empty() {
            "None".dimmed().to_string()
        } else {
            installed.dependencies.join(", ").yellow().to_string()
        };
        println!("  {} {}", "Dependencies:".dimmed(), deps);
        println!("  {} {}", "Checksum:".dimmed(), &installed.checksum[..16.min(installed.checksum.len())].dimmed());
    } else if let Some(idx_pkg) = index.find_package(package) {
        println!("{}", format!("\n{} v{}", idx_pkg.name, idx_pkg.version).bright_cyan().bold());
        println!("{}", "─".repeat(50).dimmed());
        println!("  {} {}", "Description:".dimmed(), idx_pkg.description.dimmed());
        println!("  {} {}", "License:".dimmed(), idx_pkg.license.as_deref().unwrap_or("N/A"));
        println!("  {} {}", "Repository:".dimmed(), idx_pkg.repository.blue());
        println!("  {} {}", "Platforms:".dimmed(), idx_pkg.platforms.join(", ").cyan());
        let deps = if idx_pkg.dependencies.is_empty() {
            "None".dimmed().to_string()
        } else {
            idx_pkg.dependencies.join(", ").yellow().to_string()
        };
        println!("  {} {}", "Dependencies:".dimmed(), deps);
        println!("  {} {}", "Status:".dimmed(), "not installed".yellow());
    } else {
        logger.error(&format!("Package '{}' not found", package));
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let verbose = cli.verbose;

    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{} Failed to load config: {}", "✗".red().bold(), e);
            std::process::exit(1);
        }
    };

    let logger = Logger::new(config.logs_dir.clone(), verbose);

    print_welcome(&logger);

    let github_token = config.github_token.clone()
        .or_else(|| std::env::var("UPM_GITHUB_TOKEN").ok())
        .or_else(|| std::env::var("GITHUB_TOKEN").ok());

    if github_token.is_some() {
        logger.debug("GitHub token configured");
    } else {
        logger.debug("No GitHub token (anonymous, rate limits apply)");
    }

    let github = GithubApi::new(github_token);
    let downloader = Downloader::new(config.concurrency);
    let downloader = Arc::new(downloader);

    let mut db = match PackageDatabase::load(config.installed_dir.clone()) {
        Ok(db) => db,
        Err(e) => {
            logger.warning(&format!("Could not load database: {e}. Creating new one."));
            PackageDatabase::new(config.installed_dir.clone())
        }
    };

    let mut rollback = match RollbackManager::load(config.upm_dir.clone()) {
        Ok(r) => r,
        Err(e) => {
            logger.warning(&format!("Could not load rollback data: {e}"));
            RollbackManager::new(config.upm_dir.clone())
        }
    };

    let index = match load_or_fetch_index(&config, &downloader, &logger).await {
        Ok(i) => i,
        Err(e) => {
            logger.error(&format!("Failed to load index: {e}"));
            PackageIndex::new("local", "Local package index")
        }
    };

    match &cli.command {
        Some(Commands::Install { package }) => {
            logger.header(&format!("Install: {}", package));
            match Installer::install_package(
                package,
                &config,
                &logger,
                &mut db,
                &github,
                &index,
                &mut rollback,
            )
            .await
            {
                Ok(()) => {
                    logger.success(&format!("Package '{}' installed successfully", package));
                }
                Err(e) => {
                    logger.error(&format!("Installation failed: {e}"));
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Remove { package }) => {
            handle_remove(package, &mut db, &logger, &mut rollback);
        }
        Some(Commands::Update { package }) => {
            match package {
                Some(name) => {
                    logger.header(&format!("Update: {}", name));
                    if !db.is_installed(name) {
                        logger.error(&format!("Package '{}' is not installed", name));
                        return;
                    }
                    match Installer::install_package(
                        name,
                        &config,
                        &logger,
                        &mut db,
                        &github,
                        &index,
                        &mut rollback,
                    )
                    .await
                    {
                        Ok(()) => logger.success(&format!("Updated '{}'", name)),
                        Err(e) => logger.error(&format!("Update failed: {e}")),
                    }
                }
                None => {
                    logger.header("Update All Packages");
                    let packages: Vec<String> =
                        db.list_packages().iter().map(|p| p.name.clone()).collect();
                    if packages.is_empty() {
                        logger.info("No packages to update.");
                        return;
                    }
                    for name in packages {
                        match Installer::install_package(
                            &name,
                            &config,
                            &logger,
                            &mut db,
                            &github,
                            &index,
                            &mut rollback,
                        )
                        .await
                        {
                            Ok(()) => logger.success(&format!("Updated '{}'", name)),
                            Err(e) => logger.warning(&format!("Could not update '{}': {e}", name)),
                        }
                    }
                }
            }
        }
        Some(Commands::Search { query }) => {
            let results = index.search(query);
            print_search_results(&results, query, &logger);
        }
        Some(Commands::List) => {
            let packages = db.list_packages();
            print_package_list(&packages, &logger);
        }
        Some(Commands::Doctor) => {
            if let Err(e) = Doctor::run(&config, &logger) {
                logger.error(&format!("Doctor check failed: {e}"));
            }
        }
        Some(Commands::Clean) => {
            handle_clean(&config, &logger);
        }
        Some(Commands::Info { package }) => {
            handle_info(package, &db, &index, &logger);
        }
        None => {}
    }
}
