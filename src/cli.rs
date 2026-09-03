use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "prefixpug",
    author,
    version,
    about = "Sniffs out and cleans up orphaned Steam/Proton compatdata and shader caches",
    long_about = "A high-performance Rust utility designed to reclaim NVMe storage by sniffing out and cleaning up orphaned Steam/Proton compatdata and shader caches safely."
)]
pub struct Cli {
    /// Custom path to Steam libraryfolders.vdf
    #[arg(long, value_name = "PATH")]
    pub library_vdf: Option<PathBuf>,

    /// Run in non-interactive CLI mode without launching the TUI
    #[arg(long)]
    pub no_tui: bool,

    /// Scan and report orphaned prefixes without deleting anything
    #[arg(short, long)]
    pub dry_run: bool,

    /// Automatically clean orphaned prefixes without interactive confirmation
    #[arg(long)]
    pub auto_clean: bool,

    /// Directory to store save file backups
    #[arg(long, value_name = "BACKUP_DIR")]
    pub backup_dir: Option<PathBuf>,

    /// Skip backing up detected save files before deletion (not recommended)
    #[arg(long)]
    pub skip_backup: bool,
}
