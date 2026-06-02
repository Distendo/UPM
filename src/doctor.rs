use colored::*;

use crate::config::Config;
use crate::errors::Result;
use crate::logger::Logger;
use crate::setup::Setup;

pub struct Doctor;

impl Doctor {
    pub fn run(config: &Config, logger: &Logger) -> Result<()> {
        logger.header("System Diagnostics");

        let checks = vec![
            Self::check_github_api(),
            Self::check_git_installed(),
            Self::check_upm_directories(config),
            Self::check_permissions(config),
            Self::check_filesystem(config),
            Self::check_network(),
            Self::check_path(config),
        ];

        let total = checks.len();
        let passed = checks.iter().filter(|c| c.1).count();

        println!();
        for (msg, ok) in &checks {
            let status = if *ok {
                format!("{} PASS", "✓".bright_green())
            } else {
                format!("{} FAIL", "✗".bright_red())
            };
            println!("  {}  {}", status, msg);
        }

        println!();
        logger.info(&format!("Results: {}/{} checks passed", passed, total));

        if passed < total {
            logger.warning("Some checks failed. Run 'upm doctor --verbose' for details.");
        } else {
            logger.success("All checks passed!");
        }

        Ok(())
    }

    fn check_github_api() -> (String, bool) {
        let ok = std::env::var("GITHUB_TOKEN").is_ok()
            || std::env::var("UPM_GITHUB_TOKEN").is_ok();
        let msg = if ok {
            "GitHub API: configured".to_string()
        } else {
            "GitHub API: no token set (rate limits apply)".to_string()
        };
        (msg, true)
    }

    fn check_git_installed() -> (String, bool) {
        let ok = which_git().is_some();
        let msg = if ok {
            "Git: installed".to_string()
        } else {
            "Git: NOT found".to_string()
        };
        (msg, ok)
    }

    fn check_upm_directories(config: &Config) -> (String, bool) {
        let dirs = [
            &config.upm_dir,
            &config.cache_dir,
            &config.installed_dir,
            &config.logs_dir,
            &config.config_dir,
        ];
        let mut all_ok = true;
        for dir in dirs {
            if !dir.exists() {
                if std::fs::create_dir_all(dir).is_err() {
                    all_ok = false;
                }
            }
        }
        (format!("UPM directories: {}", if all_ok { "OK" } else { "FAIL" }), all_ok)
    }

    fn check_permissions(config: &Config) -> (String, bool) {
        let dirs = [&config.cache_dir, &config.installed_dir, &config.logs_dir];
        let mut all_ok = true;
        for dir in &dirs {
            if let Ok(meta) = std::fs::metadata(dir) {
                if meta.permissions().readonly() {
                    all_ok = false;
                }
            }
        }
        (format!("Permissions: {}", if all_ok { "OK" } else { "FAIL" }), all_ok)
    }

    fn check_filesystem(config: &Config) -> (String, bool) {
        let test_file = config.cache_dir.join(".upm_test_write");
        let write_ok = std::fs::write(&test_file, b"test").is_ok();
        let read_ok = std::fs::read(&test_file).is_ok();
        let clean_ok = std::fs::remove_file(&test_file).is_ok();
        let ok = write_ok && read_ok && clean_ok;
        (format!("Filesystem: {}", if ok { "read/write OK" } else { "FAIL" }), ok)
    }

    fn check_network() -> (String, bool) {
        ("Network: deferred to runtime".to_string(), true)
    }

    fn check_path(config: &Config) -> (String, bool) {
        let in_path = Setup::check_path_bool(config);
        let msg = if in_path {
            "PATH: UPM bin directory found".to_string()
        } else {
            "PATH: UPM bin directory NOT found (run `upm init`)".to_string()
        };
        (msg, in_path)
    }
}

fn which_git() -> Option<std::path::PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let git_path = dir.join("git");
        if git_path.is_file() || git_path.with_extension("exe").is_file() {
            return Some(git_path);
        }
    }
    None
}
