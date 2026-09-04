use prefixpug::{backup, cli, scanner, tui, vdf_parser};

use anyhow::{bail, Context, Result};
use clap::{CommandFactory, Parser};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{stdin, stdout, IsTerminal, Stdout, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::cli::{Cli, Commands};
use crate::scanner::OrphanedPrefix;
use crate::tui::app::{App, AppState};

fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;

    if bytes >= GIB {
        format!("{:.2} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Executes safe prefix deletion with P0 protections:
/// - Asserts strict descendant of compatdata/shadercache
/// - Fsyncs and verifies save backup archive before any unlink
/// - Safely unlinks without following symlinks
fn execute_clean(
    orphan: &OrphanedPrefix,
    backup_root: &Path,
    skip_backup: bool,
    shaders_only: bool,
) -> Result<u64> {
    if !orphan.is_deletable() {
        bail!(
            "Safety violation: Attempted to clean protected prefix {} (Classification: {:?})",
            orphan.appid,
            orphan.classification
        );
    }

    let mut cleaned_bytes = 0;

    // 1. Vault saves first (The Pug's Nose) unless skip_backup or shaders_only is specified
    if !skip_backup && !shaders_only && !orphan.detected_saves.is_empty() {
        if let Some(archive_dir) = backup::backup_orphan_saves(orphan, backup_root)? {
            println!(
                "  [Pug Vault] Buried {} save files -> {:?}",
                orphan.detected_saves.len(),
                archive_dir
            );
        }
    }

    // 2. Remove compatdata prefix (unless shaders_only)
    if !shaders_only {
        if let Some(compat_path) = &orphan.compatdata_path {
            if compat_path.exists() {
                // P0-5: Validate path safety
                let validated_compat =
                    scanner::validate_prefix_path_for_deletion(compat_path, "compatdata")?;
                scanner::safe_delete_prefix_directory(&validated_compat)
                    .with_context(|| format!("Failed to remove compatdata at {:?}", compat_path))?;
                cleaned_bytes += orphan.compatdata_size();
            }
        }
    }

    // 3. Remove shadercache
    if let Some(shader_path) = &orphan.shadercache_path {
        if shader_path.exists() {
            let validated_shader =
                scanner::validate_prefix_path_for_deletion(shader_path, "shadercache")?;
            scanner::safe_delete_prefix_directory(&validated_shader)
                .with_context(|| format!("Failed to remove shadercache at {:?}", shader_path))?;
            cleaned_bytes += orphan.shadercache_size();
        }
    }

    Ok(cleaned_bytes)
}

fn print_cli_dog_banner(title: &str, subtitle: &str) {
    if std::io::stdout().is_terminal() {
        println!("  \x1b[36m/^-----^\\\x1b[0m    \x1b[1;36m{}\x1b[0m", title);
        println!(" \x1b[36mV  (o) (o) V\x1b[0m  \x1b[90m{}\x1b[0m", subtitle);
        println!("  \x1b[36m|   (Y)   |\x1b[0m");
        println!(" \x1b[36m/ `-------' \\\x1b[0m\n");
    } else {
        println!("{}: {}", title, subtitle);
    }
}

fn run_audit_command(
    prefixes: &[OrphanedPrefix],
    json_mode: bool,
    stale_only: bool,
    stale_days: u64,
) -> Result<i32> {
    let audit_targets: Vec<&OrphanedPrefix> = if stale_only {
        prefixes
            .iter()
            .filter(|p| p.is_stale_installed(stale_days))
            .collect()
    } else {
        prefixes.iter().collect()
    };

    if json_mode {
        let json_str = serde_json::to_string_pretty(&audit_targets)
            .context("Failed to format prefix audit as JSON")?;
        println!("{}", json_str);
        return Ok(if audit_targets.is_empty() { 4 } else { 0 });
    }

    let subtitle = if stale_only {
        format!(
            "Filtering to installed games untouched for >{} days",
            stale_days
        )
    } else {
        format!(
            "Total detected prefixes across mounted libraries: {}",
            audit_targets.len()
        )
    };
    print_cli_dog_banner("PrefixPug :: Read-Only Prefix Inventory Audit", &subtitle);

    if audit_targets.is_empty() {
        println!("No prefixes found matching criteria.");
        return Ok(4);
    }

    for p in &audit_targets {
        let title_str = p.title.as_deref().unwrap_or("unknown");
        let badge = p.classification.badge();
        let cloud_str = if p.cloud_status.is_synced() {
            "☁ SYNCED"
        } else {
            "⚠ LOCAL "
        };
        let high_val_mark = if p.is_high_value {
            " [MODS/SCRIPTS]"
        } else {
            ""
        };
        let stale_mark = if p.is_stale_installed(stale_days) {
            " \x1b[1;33m[STALE]\x1b[0m"
        } else {
            ""
        };
        println!(
            " • AppID: {:<8} | {:<12} | {:<8} | Title: {:<20} | Apparent: {:>9} | Age: {:>8} | Saves: {:>2}{}{}",
            p.appid,
            badge,
            cloud_str,
            title_str,
            format_bytes(p.total_apparent_bytes()),
            p.age_display(),
            p.detected_saves.len(),
            high_val_mark,
            stale_mark,
        );
    }

    Ok(0)
}

fn run_scan_command(orphans: &[OrphanedPrefix], json_mode: bool) -> Result<i32> {
    if json_mode {
        let json_str =
            serde_json::to_string_pretty(orphans).context("Failed to format orphans as JSON")?;
        println!("{}", json_str);
        return Ok(if orphans.is_empty() { 4 } else { 0 });
    }

    print_cli_dog_banner(
        "PrefixPug :: Steam/Proton Storage Scan",
        &format!("Found {} orphaned prefix candidate(s):", orphans.len()),
    );

    if orphans.is_empty() {
        println!("No orphaned prefixes detected. Your storage is clean.");
        return Ok(4);
    }

    let total_reclaimable: u64 = orphans.iter().map(|o| o.total_size()).sum();

    for o in orphans {
        let title_str = o.title.as_deref().unwrap_or("unknown");
        println!(
            " • AppID: {:<8} | Title: {:<20} | Size: {:>10} | Saves: {:>2} | Age: {:>8} | Compat: {:?} | Shaders: {:?}",
            o.appid,
            title_str,
            format_bytes(o.total_size()),
            o.detected_saves.len(),
            o.age_display(),
            o.compatdata_path.as_deref().map(|p| p.display().to_string()).unwrap_or_else(|| "-".into()),
            o.shadercache_path.as_deref().map(|p| p.display().to_string()).unwrap_or_else(|| "-".into()),
        );
    }

    println!(
        "\nTotal reclaimable space: \x1b[1;32m{}\x1b[0m",
        format_bytes(total_reclaimable)
    );

    Ok(0)
}

fn run_clean_command(
    cli: &Cli,
    orphans: &[OrphanedPrefix],
    appids_filter: &[String],
    backup_root: &Path,
    purge_requested: bool,
    shaders_only: bool,
) -> Result<i32> {
    let targets: Vec<&OrphanedPrefix> = if appids_filter.is_empty() {
        orphans.iter().filter(|o| o.is_deletable()).collect()
    } else {
        orphans
            .iter()
            .filter(|o| o.is_deletable() && appids_filter.contains(&o.appid))
            .collect()
    };

    if targets.is_empty() {
        println!("No orphaned prefixes matched for cleanup.");
        return Ok(4);
    }

    let is_explicit_dry_run = cli.dry_run;
    let is_authorized_live = cli.yes || cli.auto_clean || purge_requested;

    let sub = if is_explicit_dry_run || !is_authorized_live {
        "[SAFE DEFAULT: Review targets below. Deletions require explicit confirmation]"
    } else {
        "Reclamation mode active"
    };
    print_cli_dog_banner("PrefixPug :: Cleanup Target Summary", sub);

    let total_reclaimable: u64 = targets.iter().map(|o| o.total_size()).sum();
    for t in &targets {
        let title_str = t.title.as_deref().unwrap_or("unknown");
        println!(
            " • {} (AppID: {}) | Reclaimable: {}{}",
            title_str,
            t.appid,
            format_bytes(t.total_size()),
            if t.is_high_value {
                " \x1b[1;33m[HIGH-VALUE]\x1b[0m"
            } else {
                ""
            }
        );
    }
    println!(
        "Total to reclaim: \x1b[1;32m{}\x1b[0m\n",
        format_bytes(total_reclaimable)
    );

    if is_explicit_dry_run {
        println!("Dry run simulation complete. Zero bytes were modified.");
        return Ok(0);
    }

    if !is_authorized_live {
        if stdin().is_terminal() {
            print!(
                "Are you sure you want to delete these {} prefix(es) and reclaim {}? [y/N]: ",
                targets.len(),
                format_bytes(total_reclaimable)
            );
            stdout().flush()?;
            let mut input = String::new();
            stdin().read_line(&mut input)?;
            let trimmed = input.trim().to_lowercase();
            if trimmed != "y" && trimmed != "yes" {
                println!("Deletion canceled by user. Zero bytes were modified.");
                return Ok(3);
            }
        } else {
            println!(
                "Headless non-interactive deletion requires explicit confirmation. Pass --yes or --purge to proceed."
            );
            return Ok(3);
        }
    }

    // P0-6: Enforce Steam concurrency check before destructive changes
    scanner::ensure_steam_not_running(cli.ignore_running_steam)?;

    // Measure available space before purge (P1-1 statvfs)
    let sample_path = targets[0]
        .compatdata_path
        .as_ref()
        .or(targets[0].shadercache_path.as_ref())
        .map(|p| p.as_path())
        .unwrap_or_else(|| Path::new("/"));
    let free_before = scanner::get_filesystem_available_space(sample_path).unwrap_or(0);

    println!("Purging orphaned prefixes...");
    for t in &targets {
        execute_clean(t, backup_root, cli.skip_backup, shaders_only)?;
        println!("  ✓ Cleaned AppID {}", t.appid);
    }

    let free_after = scanner::get_filesystem_available_space(sample_path).unwrap_or(0);
    let measured_delta = free_after.saturating_sub(free_before);

    if measured_delta > 0 {
        println!(
            "\nReclamation complete! Measured disk space recovered: \x1b[1;32m{}\x1b[0m",
            format_bytes(measured_delta)
        );
    } else {
        println!(
            "\nReclamation complete! Recovered ~{}",
            format_bytes(total_reclaimable)
        );
    }

    Ok(0)
}

fn run_backups_command(backup_root: &Path, json_mode: bool) -> Result<i32> {
    let records = backup::list_backups(backup_root)?;

    if json_mode {
        let json_str = serde_json::to_string_pretty(&records)
            .context("Failed to format backups list as JSON")?;
        println!("{}", json_str);
        return Ok(if records.is_empty() { 4 } else { 0 });
    }

    println!("\x1b[1;36mPrefixPug :: Vaulted Save Backups\x1b[0m");
    println!("Backup root: {:?}\n", backup_root);

    if records.is_empty() {
        println!("No save backups found in vault.");
        return Ok(4);
    }

    for r in records {
        let id = r
            .directory
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?");
        let title_str = r.manifest.title.as_deref().unwrap_or("unknown");
        println!(
            " • Backup ID: {:<20} | AppID: {:<8} | Title: {:<20} | Files: {:>2} | Size: {:>10}",
            id,
            r.manifest.appid,
            title_str,
            r.manifest.files.len(),
            format_bytes(r.manifest.total_save_size)
        );
    }

    println!("\nTo verify a backup: prefixpug verify-backup <BACKUP_ID>");
    println!("To restore a backup: prefixpug restore <BACKUP_ID> --target <DEST>");
    Ok(0)
}

fn run_verify_backup_command(backup_id: &str, backup_root: &Path) -> Result<i32> {
    let report = backup::verify_backup(backup_id, backup_root)?;

    if report.is_valid {
        println!(
            "\x1b[1;32m✓ Backup '{}' is VALID:\x1b[0m {} files verified ({}) matching SHA-256 manifest.",
            report.backup_id,
            report.files_verified,
            format_bytes(report.total_bytes_verified)
        );
        Ok(0)
    } else {
        eprintln!(
            "\x1b[1;31m✗ Backup '{}' verification FAILED:\x1b[0m",
            report.backup_id
        );
        for err in report.errors {
            eprintln!("  • {}", err);
        }
        Ok(1)
    }
}

fn run_restore_command(
    backup_id: &str,
    backup_root: &Path,
    target: Option<PathBuf>,
) -> Result<i32> {
    let dest = target.unwrap_or_else(|| PathBuf::from("."));
    println!("Restoring save backup '{}' to {:?}...", backup_id, dest);
    let restored_path = backup::restore_backup(backup_id, backup_root, &dest)?;
    println!(
        "\x1b[1;32m✓ Successfully restored saves to {:?}\x1b[0m",
        restored_path
    );
    Ok(0)
}

fn run_vault_command(
    appid: &str,
    all_prefixes: &[OrphanedPrefix],
    backup_root: &Path,
) -> Result<i32> {
    println!("\x1b[1;36mPrefixPug :: Save Vault Sniffer\x1b[0m");
    println!("Target: {}\n", appid);

    let target = all_prefixes.iter().find(|p| {
        p.appid == appid
            || p.compatdata_path
                .as_ref()
                .is_some_and(|c| c == Path::new(appid))
    });

    let prefix_to_vault = if let Some(p) = target {
        p.clone()
    } else {
        let direct_path = PathBuf::from(appid);
        if direct_path.is_dir() {
            let mut warnings = Vec::new();
            let detected_saves = scanner::sniff_save_files(&direct_path, &mut warnings);
            let (is_high_value, high_value_reasons) =
                scanner::detect_high_value_prefix(&direct_path);
            let id = direct_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            OrphanedPrefix {
                appid: id,
                title: vdf_parser::infer_title_from_compatdata(&direct_path),
                classification: prefixpug::vdf_parser::PrefixClassification::Unknown,
                library_path: direct_path
                    .parent()
                    .and_then(|p| p.parent())
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| direct_path.clone()),
                compatdata_path: Some(direct_path.clone()),
                compatdata_usage: scanner::calculate_directory_usage(direct_path.as_path()).0,
                shadercache_path: None,
                shadercache_usage: scanner::DiskUsage::default(),
                last_modified: None,
                detected_saves,
                is_high_value,
                high_value_reasons,
                cloud_status: vdf_parser::SteamCloudStatus::default(),
                warnings,
            }
        } else {
            eprintln!(
                "\x1b[1;31mError:\x1b[0m No prefix matching AppID or directory '{}' found.",
                appid
            );
            return Ok(4);
        }
    };

    if prefix_to_vault.detected_saves.is_empty() {
        println!(
            "No save files or documents detected in prefix {}.",
            prefix_to_vault.appid
        );
        return Ok(4);
    }

    println!(
        "The Pug's Nose snuffed out {} save file(s) ({}) in prefix {}:",
        prefix_to_vault.detected_saves.len(),
        format_bytes(
            prefix_to_vault
                .detected_saves
                .iter()
                .map(|s| s.size_bytes)
                .sum()
        ),
        prefix_to_vault.appid
    );
    for s in prefix_to_vault.detected_saves.iter().take(10) {
        println!("  • {:?}", s.path);
    }
    if prefix_to_vault.detected_saves.len() > 10 {
        println!(
            "  ... and {} more files",
            prefix_to_vault.detected_saves.len() - 10
        );
    }

    match prefix_to_vault.cloud_status {
        vdf_parser::SteamCloudStatus::Synced => {
            println!("  ☁ Steam Cloud: Synced (remote copy detected in Steam Cloud storage)");
        }
        vdf_parser::SteamCloudStatus::NotDetected => {
            println!("  ⚠ Steam Cloud: Not detected (local save files vaulted here may be your ONLY copy!)");
        }
    }

    println!("\nArchiving and verifying vault...");
    if let Some(archive_dir) = backup::backup_orphan_saves(&prefix_to_vault, backup_root)? {
        let id = archive_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?");
        println!(
            "\x1b[1;32m✓ Successfully vaulted saves for AppID {}!\x1b[0m",
            prefix_to_vault.appid
        );
        println!("  Vault Location: {:?}", archive_dir);
        println!("  Manifest: {:?}", archive_dir.join("manifest.json"));
        println!("  Archive:  {:?}", archive_dir.join("saves.tar.gz"));
        println!("\nTo verify this vault:  prefixpug verify-backup {}", id);
        println!(
            "To restore this vault: prefixpug restore {} --target <DEST>",
            id
        );
        Ok(0)
    } else {
        println!("No files were archived.");
        Ok(4)
    }
}

fn run_tui(mut app: App) -> Result<i32> {
    enable_raw_mode().context("Failed to enable terminal raw mode")?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen).context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to initialize ratatui terminal")?;

    let res = run_tui_loop(&mut terminal, &mut app);

    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    res.map(|_| 0)
}

fn run_tui_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal
            .draw(|f| tui::ui::render(f, app))
            .map_err(|e| anyhow::anyhow!("Terminal render error: {}", e))?;

        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.state {
                        AppState::Browsing => match key.code {
                            KeyCode::Char('q') => app.should_quit = true,
                            KeyCode::Down | KeyCode::Char('j') => app.next_item(),
                            KeyCode::Up | KeyCode::Char('k') => app.prev_item(),
                            KeyCode::Char(' ') => app.toggle_selection(),
                            KeyCode::Char('a') | KeyCode::Char('A') => app.toggle_all(),
                            KeyCode::Char('i') | KeyCode::Char('I') => app.invert_selection(),
                            KeyCode::Char('s') | KeyCode::Char('S') => app.toggle_sort(),
                            KeyCode::Char('m') | KeyCode::Char('M') => app.toggle_mascot(),
                            KeyCode::Char('/') => {
                                app.state = AppState::Filtering;
                                app.filter_query.clear();
                            }
                            KeyCode::Char('c') | KeyCode::Char('C') => {
                                if !app.selected_appids.is_empty() {
                                    app.state = AppState::ConfirmingDeletion;
                                } else {
                                    app.status_message =
                                        "Select at least one prefix to clean.".to_string();
                                }
                            }
                            KeyCode::Char('?') | KeyCode::Char('h') => {
                                app.state = AppState::ShowingHelp;
                            }
                            _ => {}
                        },
                        AppState::Filtering => match key.code {
                            KeyCode::Enter | KeyCode::Esc => {
                                app.state = AppState::Browsing;
                            }
                            KeyCode::Backspace => {
                                app.filter_query.pop();
                                app.apply_filter();
                            }
                            KeyCode::Char(c) => {
                                app.filter_query.push(c);
                                app.apply_filter();
                            }
                            _ => {}
                        },
                        AppState::ShowingHelp => {
                            app.state = AppState::Browsing;
                        }
                        AppState::ConfirmingDeletion => match key.code {
                            KeyCode::Char('y') | KeyCode::Char('Y') => {
                                app.state = AppState::Cleaning;

                                // Concurrency check
                                if let Err(e) = scanner::ensure_steam_not_running(false) {
                                    app.status_message = format!("⚠ {}", e);
                                    app.state = AppState::Browsing;
                                    continue;
                                }

                                let mut reclaimed = 0;
                                let targets: Vec<OrphanedPrefix> = app
                                    .all_orphans
                                    .iter()
                                    .filter(|o| app.selected_appids.contains(&o.appid))
                                    .cloned()
                                    .collect();

                                for t in &targets {
                                    if let Ok(bytes) =
                                        execute_clean(t, &app.backup_dir, false, false)
                                    {
                                        reclaimed += bytes;
                                    }
                                }

                                app.space_reclaimed += reclaimed;
                                app.status_message = format!(
                                    "Purged {} prefix(es). Reclaimed {}!",
                                    targets.len(),
                                    format_bytes(reclaimed)
                                );
                                app.all_orphans
                                    .retain(|o| !app.selected_appids.contains(&o.appid));
                                app.selected_appids.clear();
                                app.apply_filter();
                                app.state = AppState::Done;
                            }
                            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                app.state = AppState::Browsing;
                                app.status_message = "Deletion cancelled.".to_string();
                            }
                            _ => {}
                        },
                        AppState::Done => match key.code {
                            KeyCode::Char('q')
                            | KeyCode::Esc
                            | KeyCode::Enter
                            | KeyCode::Char(' ') => {
                                app.state = AppState::Browsing;
                            }
                            _ => {}
                        },
                        AppState::Cleaning => {}
                    }
                }
            }
        }

        app.tick();
    }

    Ok(())
}

fn run(cli: Cli) -> Result<i32> {
    let vdf_path = match &cli.library_vdf {
        Some(p) => p.clone(),
        None => vdf_parser::default_library_vdf_path().context(
            "Failed to locate Steam libraryfolders.vdf. Specify path using --library-vdf <PATH>",
        )?,
    };

    let backup_dir = match &cli.backup_dir {
        Some(p) => p.clone(),
        None => backup::default_backup_root()?,
    };

    // Subcommand dispatch (independent of library scanning)
    match &cli.command {
        Some(Commands::Backups) => {
            return run_backups_command(&backup_dir, cli.json);
        }
        Some(Commands::VerifyBackup { backup_id }) => {
            return run_verify_backup_command(backup_id, &backup_dir);
        }
        Some(Commands::Restore { backup_id, target }) => {
            return run_restore_command(backup_id, &backup_dir, target.clone());
        }
        Some(Commands::Completions { shell }) => {
            let mut cmd = Cli::command();
            clap_complete::generate(*shell, &mut cmd, "prefixpug", &mut std::io::stdout());
            return Ok(0);
        }
        _ => {}
    }

    // P0-1: Parse all library folders and validate reachability
    let library_folders = vdf_parser::parse_library_folders(&vdf_path)
        .with_context(|| format!("Failed to parse library folders from {:?}", vdf_path))?;

    // Collect steam roots for userdata / shortcuts.vdf discovery
    let mut steam_roots = Vec::new();
    for lib in &library_folders {
        steam_roots.push(lib.path.clone());
    }
    if let Some(parent) = vdf_path.parent().and_then(|p| p.parent()) {
        steam_roots.push(parent.to_path_buf());
    }

    // P0-2: Discover non-Steam shortcuts across all user profiles
    let protected_shortcuts = vdf_parser::discover_non_steam_shortcuts(&steam_roots)?;

    // Discover installed games across all reachable libraries
    let installed_games = vdf_parser::discover_installed_games(&library_folders)?;

    // P1-2: Support --older-than flag
    let older_than_dur = cli.older_than.map(|d| Duration::from_secs(d * 86400));

    let all_prefixes = scanner::scan_all_prefixes(
        &library_folders,
        &installed_games,
        &protected_shortcuts,
        older_than_dur,
    )?;

    let orphans: Vec<OrphanedPrefix> = all_prefixes
        .iter()
        .filter(|p| p.is_deletable())
        .cloned()
        .collect();

    match &cli.command {
        Some(Commands::Audit { appids, stale }) => {
            let filtered: Vec<OrphanedPrefix> = if appids.is_empty() {
                all_prefixes
            } else {
                all_prefixes
                    .into_iter()
                    .filter(|p| appids.contains(&p.appid))
                    .collect()
            };
            let stale_days = cli.older_than.unwrap_or(90);
            run_audit_command(&filtered, cli.json, *stale, stale_days)
        }
        Some(Commands::Scan { appids }) => {
            let filtered: Vec<OrphanedPrefix> = if appids.is_empty() {
                orphans
            } else {
                orphans
                    .into_iter()
                    .filter(|o| appids.contains(&o.appid))
                    .collect()
            };
            run_scan_command(&filtered, cli.json)
        }
        Some(Commands::Clean { appids, purge }) => run_clean_command(
            &cli,
            &orphans,
            appids,
            &backup_dir,
            *purge,
            cli.shaders_only,
        ),
        Some(Commands::Shaders { appids, yes }) => {
            run_clean_command(&cli, &orphans, appids, &backup_dir, *yes, true)
        }
        Some(Commands::Vault { appid }) => run_vault_command(appid, &all_prefixes, &backup_dir),
        Some(Commands::Backups)
        | Some(Commands::VerifyBackup { .. })
        | Some(Commands::Restore { .. })
        | Some(Commands::Completions { .. }) => unreachable!(),
        None => {
            if cli.no_tui || cli.dry_run || cli.auto_clean || cli.json || cli.shaders_only {
                if cli.auto_clean {
                    run_clean_command(&cli, &orphans, &[], &backup_dir, cli.yes, cli.shaders_only)
                } else {
                    run_scan_command(&orphans, cli.json)
                }
            } else {
                let app = App::new(orphans, backup_dir);
                run_tui(app)
            }
        }
    }
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => std::process::ExitCode::from(code as u8),
        Err(err) => {
            let err_msg = format!("{:#}", err);
            eprintln!("\x1b[1;31mError:\x1b[0m {}", err_msg);
            if err_msg.contains("Safety violation")
                || err_msg.contains("Steam is currently running")
                || err_msg.contains("Unmounted library")
                || err_msg.contains("escapes outside")
            {
                std::process::ExitCode::from(2)
            } else {
                std::process::ExitCode::from(1)
            }
        }
    }
}
