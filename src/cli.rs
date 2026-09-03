use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "prefixpug",
    author,
    version,
    about = "⚡ Safely sniffs out and reclaims orphaned Steam/Proton compatdata and shader caches",
    long_about = "A safe Rust utility to reclaim storage by identifying orphaned Steam/Proton \
                  compatdata and shader caches. Protects non-Steam shortcuts, respects multi-library \
                  mounts, and automatically archives local save files to an fsynced vault before removal."
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

    /// Scan and simulate actions without modifying or deleting files (default for Clean)
    #[arg(short, long, global = true)]
    pub dry_run: bool,

    /// Actually execute deletions headlessly without interactive prompt (requires confirmation)
    #[arg(long, short = 'y', global = true)]
    pub yes: bool,

    /// Automatically clean orphaned prefixes without interactive confirmation
    #[arg(long, global = true)]
    pub auto_clean: bool,

    /// Output results in JSON format (ideal for scripts and automated tooling)
    #[arg(long, global = true)]
    pub json: bool,

    /// Only match prefixes that have been untouched/unmodified for at least N days
    #[arg(long, global = true, value_name = "DAYS")]
    pub older_than: Option<u64>,

    /// Ignore active Steam process detection (use with care; intended for testing)
    #[arg(long, global = true)]
    pub ignore_running_steam: bool,

    /// Custom directory to store save file backups
    #[arg(long, global = true, value_name = "DIR")]
    pub backup_dir: Option<PathBuf>,

    /// Clean only shader caches, leaving Wine/Proton compatdata prefixes untouched
    #[arg(long, global = true)]
    pub shaders_only: bool,

    /// Skip backing up detected save files before deletion (not recommended)
    #[arg(long, global = true)]
    pub skip_backup: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Read-only audit of all prefixes (installed, non-Steam shortcuts, runtimes, orphans)
    Audit {
        /// Filter audit to specific AppIDs (comma-separated or multiple args)
        #[arg(short, long, value_delimiter = ',')]
        appids: Vec<String>,
    },

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

        /// Confirm deletion headlessly (same as --yes)
        #[arg(long)]
        purge: bool,
    },

    /// Low-risk mode: clean only shader caches without touching any compatdata prefixes
    Shaders {
        /// Specific AppIDs to clean shader caches for (leave empty for all)
        #[arg(short, long, value_delimiter = ',')]
        appids: Vec<String>,

        /// Confirm deletion headlessly
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// List all archived save vaults created by PrefixPug
    Backups,

    /// Verify an archived save vault against its manifest and SHA-256 checksums
    VerifyBackup {
        /// Backup folder name or full path (from `prefixpug backups`)
        backup_id: String,
    },

    /// Restore save files from an archived backup vault
    Restore {
        /// Backup folder name or full path (from `prefixpug backups`)
        backup_id: String,

        /// Destination directory to extract save files (defaults to current directory)
        #[arg(short, long, value_name = "DEST")]
        target: Option<PathBuf>,
    },

    /// Generate shell completions (bash, zsh, fish, powershell, elvish)
    Completions {
        /// Target shell
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}
