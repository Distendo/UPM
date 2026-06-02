use chrono::Local;
use colored::*;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Logger {
    file: Option<Mutex<std::fs::File>>,
    verbose: bool,
}

impl Logger {
    pub fn new(log_dir: PathBuf, verbose: bool) -> Self {
        fs::create_dir_all(&log_dir).ok();
        let log_file = log_dir.join(format!("upm_{}.log", Local::now().format("%Y%m%d")));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .ok()
            .map(Mutex::new);

        Self { file, verbose }
    }

    pub fn info(&self, msg: &str) {
        let line = format!("[{}] INFO: {}", Self::timestamp(), msg);
        println!("{} {} {} {}",
            "[UPM]".bright_blue().bold(),
            "●".green(),
            msg.bright_white(),
            "".clear()
        );
        self.write_log(&line);
    }

    pub fn success(&self, msg: &str) {
        println!("{} {} {}",
            "[UPM]".bright_blue().bold(),
            "✓".bright_green().bold(),
            msg.green()
        );
        self.write_log(&format!("[{}] SUCCESS: {}", Self::timestamp(), msg));
    }

    pub fn warning(&self, msg: &str) {
        println!("{} {} {}",
            "[UPM]".bright_blue().bold(),
            "⚠".yellow().bold(),
            msg.yellow()
        );
        self.write_log(&format!("[{}] WARN: {}", Self::timestamp(), msg));
    }

    pub fn error(&self, msg: &str) {
        eprintln!("{} {} {}",
            "[UPM]".bright_blue().bold(),
            "✗".bright_red().bold(),
            msg.red()
        );
        self.write_log(&format!("[{}] ERROR: {}", Self::timestamp(), msg));
    }

    pub fn step(&self, msg: &str) {
        println!("  {} {}",
            "→".cyan(),
            msg.cyan()
        );
        self.write_log(&format!("[{}] STEP: {}", Self::timestamp(), msg));
    }

    pub fn debug(&self, msg: &str) {
        if self.verbose {
            println!("  {} {}",
                "→".bright_black(),
                msg.bright_black()
            );
        }
        self.write_log(&format!("[{}] DEBUG: {}", Self::timestamp(), msg));
    }

    pub fn header(&self, msg: &str) {
        let line = format!("═══ {} ═══", msg);
        println!("\n{}\n",
            line.bright_magenta().bold()
        );
    }

    fn timestamp() -> String {
        Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
    }

    fn write_log(&self, msg: &str) {
        if let Some(ref file_mutex) = self.file {
            if let Ok(mut file) = file_mutex.lock() {
                writeln!(file, "{}", msg).ok();
            }
        }
    }
}
