use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "prefixpug",
    author,
    version,
    about = "⚡ Sniffs out and safely reclaims orphaned Steam/Proton compatdata and shader caches",
    long_about = "A high-performance Rust utility designed to reclaim NVMe storage by sniffing out and cleaning up orphaned Steam/Proton compatdata and shader caches. Safely digs up and archives local save files before removal."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Custom path to Steam libraryfolders.vdf
    #[arg(long, global = true, value_name = "PATH")]
    pub library_vdf: Option<PathBuf>,

    /// Run headlessly without launching the interactive Ratatui TUI
    #[arg(long, global = true)]
    pub no_tui: bool,

    /// Scan and simulate actions without modifying or deleting files
    #[arg(short, long, global = true)]
    pub dry_run: bool,

    /// Automatically clean orphaned prefixes without interactive confirmation
    #[arg(long, global = true)]
    pub auto_clean: bool,

    /// Output results in JSON format (ideal for scripts and automated tooling)
    #[arg(long, global = true)]
    pub json: bool,

    /// Custom directory to store save file backups
    #[arg(long, global = true, value_name = "DIR")]
    pub backup_dir: Option<PathBuf>,

    /// Skip backing up detected save files before deletion (not recommended)
    #[arg(long, global = true)]
    pub skip_backup: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Scan and list all orphaned compatdata and shadercache prefixes
    Scan {
        /// Filter scan to specific AppIDs (comma-separated or multiple args)
        #[arg(short, long, value_delimiter = ',')]
        appids: Vec<String>,
    },

    /// Clean orphaned prefixes with save preservation
    Clean {
        /// Specific AppIDs to clean (leave empty to process all detected orphans)
        #[arg(short, long, value_delimiter = ',')]
        appids: Vec<String>,
    },

    /// List all archived save vaults created by PrefixPug
    Backups,

    /// Restore save files from an archived backup vault
    Restore {
        /// Backup folder name or full path (from `prefixpug backups`)
        backup_id: String,

        /// Destination directory to extract save files (defaults to current directory)
        #[arg(short, long, value_name = "DEST")]
        target: Option<PathBuf>,
    },
}
