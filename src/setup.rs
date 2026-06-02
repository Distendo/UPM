use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

use crate::config::Config;
use crate::errors::{Result, UpmError};
use crate::logger::Logger;

pub struct Setup;

impl Setup {
    pub fn init(config: &Config, logger: &Logger, assume_yes: bool) -> Result<()> {
        logger.header("UPM Setup");
        let bin_dir = config.installed_dir.join("bin");

        if Self::already_in_path(&bin_dir) {
            logger.success("UPM bin directory is already in PATH");
            return Ok(());
        }

        let rc_file = Self::detect_rc_file()?;
        let line = Self::export_line(&bin_dir);

        logger.info(&format!("Detected shell config: {}", rc_file.display()));
        logger.step(&format!("Adding: {}", &line));

        if !assume_yes {
            print!("  Add to {}? [Y/n] ", rc_file.display());
            io::stdout().flush().ok();
            let mut input = String::new();
            io::stdin().read_line(&mut input).ok();
            let input = input.trim().to_lowercase();
            if input == "n" || input == "no" {
                logger.info("Skipped. Add manually:");
                logger.info(&format!("  {}", &line));
                return Ok(());
            }
        }

        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&rc_file)
            .map_err(|e| UpmError::General(format!("Cannot open {}: {}", rc_file.display(), e)))?;

        writeln!(file, "\n# UPM - Universal Package Manager")?;
        writeln!(file, "{}", line)?;

        logger.success(&format!("Added to {}", rc_file.display()));
        logger.step("Restart your shell or run:");
        logger.info(&format!("  source {}", rc_file.display()));

        Ok(())
    }

    pub fn check_path(config: &Config, logger: &Logger) -> Result<()> {
        let bin_dir = config.installed_dir.join("bin");

        if Self::already_in_path(&bin_dir) {
            logger.success("UPM bin directory is in PATH");
        } else {
            logger.warning("UPM bin directory is NOT in PATH");
            logger.info("Run `upm init` to add it automatically");
        }

        Ok(())
    }

    pub fn check_path_bool(config: &Config) -> bool {
        let bin_dir = config.installed_dir.join("bin");
        Self::already_in_path(&bin_dir)
    }

    fn already_in_path(bin_dir: &PathBuf) -> bool {
        env::var_os("PATH")
            .map(|path| {
                env::split_paths(&path).any(|p| {
                    fs::canonicalize(&p).ok() == fs::canonicalize(bin_dir).ok()
                        || p == *bin_dir
                })
            })
            .unwrap_or(false)
    }

    fn detect_rc_file() -> Result<PathBuf> {
        let shell = env::var("SHELL").unwrap_or_default();
        let home = dirs::home_dir().ok_or_else(|| UpmError::General("Cannot find home directory".into()))?;

        let candidates: Vec<PathBuf> = if shell.ends_with("fish") {
            vec![
                home.join(".config/fish/config.fish"),
                home.join(".fishrc"),
            ]
        } else if shell.ends_with("zsh") {
            vec![
                home.join(".zshrc"),
                home.join(".zshenv"),
                home.join(".zprofile"),
            ]
        } else {
            vec![
                home.join(".bashrc"),
                home.join(".bash_profile"),
                home.join(".profile"),
            ]
        };

        for candidate in &candidates {
            if candidate.exists() {
                return Ok(candidate.clone());
            }
        }

        Ok(candidates[0].clone())
    }

    fn export_line(bin_dir: &PathBuf) -> String {
        let shell = env::var("SHELL").unwrap_or_default();
        if shell.ends_with("fish") {
            format!("fish_add_path {}", bin_dir.display())
        } else {
            format!("export PATH=\"$PATH:{}\"", bin_dir.display())
        }
    }
}
