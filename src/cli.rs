use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "upm")]
#[command(about = "Universal Package Manager", long_about = None)]
#[command(version = "0.1.0")]
#[command(arg_required_else_help = true)]
#[command(after_help = "Made by Distendo (discord: esedik11)")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(short = 'v', long = "verbose", global = true)]
    pub verbose: bool,

    #[arg(short = 'y', long = "yes", global = true, help = "Assume yes to prompts")]
    pub assume_yes: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Install a package")]
    Install {
        #[arg(help = "Package name to install")]
        package: String,

        #[arg(long = "ai", help = "Use AI to generate build/install plan (requires GROQ_API_KEY)")]
        use_ai: bool,
    },

    #[command(about = "Remove an installed package")]
    Remove {
        #[arg(help = "Package name to remove")]
        package: String,
    },

    #[command(about = "Update package(s)")]
    Update {
        #[arg(help = "Package name to update (updates all if not specified)")]
        package: Option<String>,
    },

    #[command(about = "Search for packages")]
    Search {
        #[arg(help = "Search query")]
        query: String,
    },

    #[command(about = "List installed packages")]
    List,

    #[command(about = "Run system diagnostics")]
    Doctor,

    #[command(about = "Register a new package in the official index")]
    Add {
        #[arg(help = "Package name")]
        name: String,
        #[arg(help = "GitHub repository URL")]
        repo: String,
        #[arg(short = 'V', long = "version", default_value = "1.0.0")]
        version: String,
        #[arg(short = 'd', long = "description", default_value = "")]
        description: String,
        #[arg(short = 'l', long = "license", default_value = "MIT")]
        license: String,
    },

    #[command(about = "Initialize UPM and add bin directory to PATH")]
    Init,

    #[command(about = "Clean cache")]
    Clean,

    #[command(about = "Show information about a package")]
    Info {
        #[arg(help = "Package name")]
        package: String,
    },

    #[command(about = "Verify installed package integrity")]
    Verify {
        #[arg(help = "Package name to verify")]
        package: String,
    },

    #[command(about = "List packages with available updates")]
    Outdated,

    #[command(about = "Show detailed package info")]
    Show {
        #[arg(help = "Package name")]
        package: String,
    },

    #[command(about = "Manage rollback points")]
    Rollback {
        #[command(subcommand)]
        action: RollbackAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum RollbackAction {
    #[command(about = "List all rollback points")]
    List,

    #[command(about = "Show details of a rollback point")]
    Show {
        #[arg(help = "Rollback point ID")]
        id: String,
    },
}
