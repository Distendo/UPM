#![allow(dead_code)]

mod api;
mod cli;
mod config;
mod database;
mod doctor;
mod downloader;
mod errors;
mod groq;
mod groq_key;
mod logger;
mod package;
mod registry;
mod rollback;
mod setup;
mod verify;

use std::sync::Arc;

use clap::Parser;
use colored::*;

use api::github::GithubApi;
use cli::{Cli, Commands, RollbackAction};
use config::Config;
use database::PackageDatabase;
use doctor::Doctor;
use downloader::Downloader;
use errors::Result;
use logger::Logger;
use package::index::PackageIndex;
use package::installer::Installer;
use registry::Registry;
use rollback::RollbackManager;
use setup::Setup;

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
        let installed_at = pkg.installed_at.get(..10).unwrap_or("???");
        println!(
            "  {:<23} {:<15} {:<20} {}",
            pkg.name.bright_white(),
            pkg.version.green(),
            installed_at.yellow(),
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
        println!("  {} {}", "Installed:".dimmed(), installed.installed_at.get(..19).unwrap_or("?").yellow());
        println!("  {} {}", "Path:".dimmed(), installed.install_path.to_string_lossy().dimmed());
        println!("  {} {}", "Files:".dimmed(), installed.files.len().to_string().cyan());
        let deps = if installed.dependencies.is_empty() {
            "None".dimmed().to_string()
        } else {
            installed.dependencies.join(", ").yellow().to_string()
        };
        println!("  {} {}", "Dependencies:".dimmed(), deps);
        let checksum = installed.checksum.get(..16).unwrap_or("?");
        println!("  {} {}", "Checksum:".dimmed(), checksum.dimmed());
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

fn handle_verify(package: &str, db: &PackageDatabase, logger: &Logger) {
    logger.header(&format!("Verify: {}", package));
    let installed = match db.get_package(package) {
        Some(p) => p,
        None => {
            logger.error(&format!("Package '{}' is not installed", package));
            return;
        }
    };

    if installed.checksum.is_empty() {
        logger.warning("No checksum recorded for this package");
        return;
    }

    let path = &installed.install_path;
    let db_path = db.db_path.parent().unwrap_or(path);
    let source_path = db_path.join(format!("{}-{}.tgz", package, installed.version));

    if source_path.exists() {
        match verify::sha256_file(&source_path) {
            Ok(actual) if actual == installed.checksum => {
                logger.success("Checksum matches — package integrity verified");
            }
            Ok(actual) => {
                logger.error(&format!(
                    "Checksum MISMATCH!\n  Expected: {}\n  Actual:   {}",
                    installed.checksum, actual
                ));
            }
            Err(e) => {
                logger.error(&format!("Could not compute checksum: {e}"));
            }
        }
    } else {
        logger.info("Cached source not found. Use 'upm update' to re-download and verify.");
    }
}

fn handle_outdated(db: &PackageDatabase, index: &PackageIndex, logger: &Logger) {
    logger.header("Checking for outdated packages");

    let installed = db.list_packages();
    if installed.is_empty() {
        logger.info("No packages installed.");
        return;
    }

    let mut outdated = Vec::new();
    for pkg in &installed {
        if let Some(idx_pkg) = index.find_package(&pkg.name) {
            let installed_ver = pkg.version.trim_start_matches('v');
            let index_ver = idx_pkg.version.trim_start_matches('v');
            if installed_ver != index_ver {
                outdated.push((pkg.name.clone(), pkg.version.clone(), idx_pkg.version.clone()));
            }
        }
    }

    if outdated.is_empty() {
        logger.success("All packages are up to date");
        return;
    }

    println!(
        "{}",
        format!("{:<25} {:<20} {:<20}", "Package", "Installed", "Available")
            .bright_cyan().bold()
    );
    println!("{}", "─".repeat(65).dimmed());

    for (name, installed_ver, available_ver) in &outdated {
        println!(
            "  {:<23} {:<20} {:<20}",
            name.bright_white(),
            installed_ver.yellow(),
            available_ver.green()
        );
    }

    println!();
    logger.info(&format!("{} packages can be updated", outdated.len()));
}

fn handle_show(package: &str, db: &PackageDatabase, index: &PackageIndex, logger: &Logger) {
    logger.header(&format!("Show: {}", package));
    handle_info(package, db, index, logger);
}

fn handle_rollback(action: &RollbackAction, rollback: &RollbackManager, logger: &Logger) {
    match action {
        RollbackAction::List => {
            logger.header("Rollback Points");
            let points = rollback.list();
            if points.is_empty() {
                logger.info("No rollback points available.");
                return;
            }

            println!(
                "{}",
                format!("{:<25} {:<20} {:<15} {:<15} {}", "ID", "Timestamp", "Before", "After", "Description")
                    .bright_cyan().bold()
            );
            println!("{}", "─".repeat(100).dimmed());

            for point in points {
                let id_short = point.id.get(..20).unwrap_or(&point.id);
                let ts = point.timestamp.get(..19).unwrap_or(&point.timestamp);
                let before_count = point.packages_before.len();
                let after_count = point.packages_after.len();
                println!(
                    "  {:<23} {:<20} {:<15} {:<15} {}",
                    id_short.bright_white(),
                    ts.yellow(),
                    before_count.to_string().cyan(),
                    after_count.to_string().cyan(),
                    point.description.dimmed()
                );
            }
            println!();
            logger.info(&format!("{} rollback points", points.len()));
        }
        RollbackAction::Show { id } => {
            let point = match rollback.rollbacks.iter().find(|p| p.id == *id) {
                Some(p) => p,
                None => {
                    logger.error(&format!("Rollback point '{}' not found", id));
                    return;
                }
            };

            println!("{}", format!("\nRollback Point: {}", point.id).bright_cyan().bold());
            println!("{}", "─".repeat(50).dimmed());
            println!("  {} {}", "Timestamp:".dimmed(), point.timestamp.yellow());
            println!("  {} {}", "Description:".dimmed(), point.description);
            println!("  {} {} packages", "Before:".dimmed(), point.packages_before.len().to_string().cyan());
            for (name, ver) in &point.packages_before {
                println!("    {} {}", name.green(), ver.dimmed());
            }
            println!("  {} {} packages", "After:".dimmed(), point.packages_after.len().to_string().cyan());
            for (name, ver) in &point.packages_after {
                println!("    {} {}", name.green(), ver.dimmed());
            }
        }
    }
}

fn platform_binary_name() -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    match (os, arch) {
        ("macos", "aarch64") => "upm-aarch64-apple-darwin".into(),
        ("macos", "x86_64") => "upm-x86_64-apple-darwin".into(),
        ("linux", "x86_64") => "upm-x86_64-unknown-linux-gnu".into(),
        ("linux", "aarch64") => "upm-aarch64-unknown-linux-gnu".into(),
        ("windows", "x86_64") => "upm-x86_64-pc-windows-msvc.exe".into(),
        _ => format!("upm-{}-{}", arch, os),
    }
}

async fn handle_update_upm(config: &Config, logger: &Logger, github: &api::github::GithubApi) {
    logger.header("UPM Self-Update");

    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            logger.error(&format!("Cannot determine current executable path: {e}"));
            return;
        }
    };

    logger.step("Checking for latest version...");
    let releases_url = "https://api.github.com/repos/Distendo/UPM/releases/latest";
    let release_data = match github.client().download_bytes(releases_url).await {
        Ok(data) => data,
        Err(e) => {
            logger.error(&format!("Failed to fetch latest release: {e}"));
            return;
        }
    };

    let release: serde_json::Value = match serde_json::from_slice(&release_data) {
        Ok(v) => v,
        Err(e) => {
            logger.error(&format!("Failed to parse release data: {e}"));
            return;
        }
    };

    let latest_version = release["tag_name"]
        .as_str()
        .unwrap_or("unknown")
        .trim_start_matches('v');

    let current_version = env!("CARGO_PKG_VERSION");
    if latest_version <= current_version {
        logger.success(&format!("Already up to date (v{})", current_version));
        return;
    }

    logger.info(&format!("Found new version: v{} (current: v{})", latest_version, current_version));

    let binary_name = platform_binary_name();
    let download_url = format!(
        "https://github.com/Distendo/UPM/releases/download/v{}/{}",
        latest_version, binary_name
    );

    logger.step(&format!("Downloading {}...", binary_name));
    let tmp_dir = config.cache_dir.join(".upm_update_tmp");
    std::fs::create_dir_all(&tmp_dir).ok();
    let tmp_path = tmp_dir.join(&binary_name);

    if let Err(e) = github.client().download_file(&download_url, &tmp_path).await {
        logger.error(&format!("Download failed: {e}"));
        logger.info(&format!("Download URL was: {}", download_url));
        std::fs::remove_dir_all(&tmp_dir).ok();
        return;
    }

    logger.step("Installing update...");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755)) {
            logger.warning(&format!("Could not set executable permission: {e}"));
        }
    }

    let backup_path = current_exe.with_extension("upm.bak");
    std::fs::rename(&current_exe, &backup_path).ok();

    match std::fs::rename(&tmp_path, &current_exe) {
        Ok(()) => {
            std::fs::remove_file(&backup_path).ok();
            std::fs::remove_dir_all(&tmp_dir).ok();
            logger.success(&format!("Updated to v{}!", latest_version));
            logger.info("Please restart any running UPM processes.");
        }
        Err(e) => {
            logger.error(&format!("Failed to replace binary: {e}"));
            logger.info("Restoring backup...");
            std::fs::rename(&backup_path, &current_exe).ok();
        }
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
        Some(Commands::Install { package, use_ai }) => {
            logger.header(&format!("Install: {}", package));
            match Installer::install_package(
                package,
                &config,
                &logger,
                &mut db,
                &github,
                &index,
                &mut rollback,
                *use_ai,
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
                    match Installer::reinstall_package(
                        name,
                        &config,
                        &logger,
                        &mut db,
                        &github,
                        &index,
                        &mut rollback,
                        false,
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
                        match Installer::reinstall_package(
                            &name,
                            &config,
                            &logger,
                            &mut db,
                            &github,
                            &index,
                            &mut rollback,
                            false,
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
        Some(Commands::Add { name, repo, version, description, license }) => {
            if let Err(e) = Registry::add(name, repo, version, description, license, &config, &logger, cli.assume_yes) {
                logger.error(&format!("Failed to register package: {e}"));
            }
        }
        Some(Commands::Init) => {
            if let Err(e) = Setup::init(&config, &logger, cli.assume_yes) {
                logger.error(&format!("Setup failed: {e}"));
            }
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
        Some(Commands::Verify { package }) => {
            handle_verify(package, &db, &logger);
        }
        Some(Commands::Outdated) => {
            handle_outdated(&db, &index, &logger);
        }
        Some(Commands::Show { package }) => {
            handle_show(package, &db, &index, &logger);
        }
        Some(Commands::Rollback { action }) => {
            handle_rollback(action, &rollback, &logger);
        }
        Some(Commands::UpdateUpm) => {
            handle_update_upm(&config, &logger, &github).await;
        }
        None => {}
    }
}
