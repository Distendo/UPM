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

    #[command(about = "Clean cache")]
    Clean,

    #[command(about = "Show information about a package")]
    Info {
        #[arg(help = "Package name")]
        package: String,
    },
}
