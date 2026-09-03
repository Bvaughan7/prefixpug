pub mod backup;
pub mod cli;
pub mod scanner;
pub mod tui;
pub mod vdf_parser;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, stdout, Stdout, Write};
use std::path::Path;
use std::time::Duration;

use crate::cli::Cli;
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

fn execute_clean(
    orphan: &OrphanedPrefix,
    backup_root: &Path,
    skip_backup: bool,
) -> Result<u64> {
    let mut cleaned_bytes = 0;

    // 1. Dig up and bury save files first (The Pug's Nose)
    if !skip_backup && !orphan.detected_saves.is_empty() {
        if let Some(archive_dir) = backup::backup_orphan_saves(orphan, backup_root)? {
            println!("  [Pug Vault] Buried {} save files -> {:?}", orphan.detected_saves.len(), archive_dir);
        }
    }

    // 2. Remove compatdata prefix
    if let Some(compat_path) = &orphan.compatdata_path {
        if compat_path.is_dir() {
            std::fs::remove_dir_all(compat_path)
                .with_context(|| format!("Failed to remove compatdata at {:?}", compat_path))?;
            cleaned_bytes += orphan.compatdata_size;
        }
    }

    // 3. Remove shadercache
    if let Some(shader_path) = &orphan.shadercache_path {
        if shader_path.is_dir() {
            std::fs::remove_dir_all(shader_path)
                .with_context(|| format!("Failed to remove shadercache at {:?}", shader_path))?;
            cleaned_bytes += orphan.shadercache_size;
        }
    }

    Ok(cleaned_bytes)
}

fn run_cli_mode(cli: &Cli, orphans: &[OrphanedPrefix], backup_dir: &Path) -> Result<()> {
    println!("\x1b[1;35m⚡ PREFIXPUG :: Steam/Proton Storage Reclamation\x1b[0m");
    println!("Found {} orphaned prefix candidates:\n", orphans.len());

    if orphans.is_empty() {
        println!("✨ No orphaned prefixes detected. Your storage is squeaky clean!");
        return Ok(());
    }

    let total_reclaimable: u64 = orphans.iter().map(|o| o.total_size()).sum();

    for o in orphans {
        println!(
            " • AppID: {:<8} | Size: {:>10} | Saves: {} | Compat: {:?} | Shaders: {:?}",
            o.appid,
            format_bytes(o.total_size()),
            o.detected_saves.len(),
            o.compatdata_path.as_deref().map(|p| p.display().to_string()).unwrap_or_else(|| "-".into()),
            o.shadercache_path.as_deref().map(|p| p.display().to_string()).unwrap_or_else(|| "-".into()),
        );
    }

    println!("\nTotal reclaimable space: {}", format_bytes(total_reclaimable));

    if cli.dry_run {
        println!("\n[Dry Run] No files modified or removed.");
        return Ok(());
    }

    // Safety rule: prompt for user confirmation before deletion unless --auto-clean was passed
    if !cli.auto_clean {
        print!("\nAre you sure you want to bury saves and delete these prefixes? (y/N): ");
        stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Operation aborted by user.");
            return Ok(());
        }
    }

    println!("\nPurging orphaned prefixes...");
    let mut total_cleaned = 0;
    for orphan in orphans {
        match execute_clean(orphan, backup_dir, cli.skip_backup) {
            Ok(bytes) => {
                total_cleaned += bytes;
                println!("  ✓ Cleaned AppID {}", orphan.appid);
            }
            Err(e) => {
                eprintln!("  ✗ Error cleaning AppID {}: {}", orphan.appid, e);
            }
        }
    }

    println!("\n🎉 Reclamation complete! Cleaned {}.", format_bytes(total_cleaned));
    Ok(())
}

fn run_tui(mut app: App) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout_handle = stdout();
    execute!(stdout_handle, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout_handle);
    let mut terminal = Terminal::new(backend)?;

    let res = run_tui_event_loop(&mut terminal, &mut app);

    // Restore terminal state
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

fn run_tui_event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal
            .draw(|f| tui::ui::render(f, app))
            .map_err(|e| anyhow::anyhow!("Terminal draw error: {}", e))?;

        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.state {
                        AppState::Browsing => match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                app.should_quit = true;
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                app.prev_item();
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                app.next_item();
                            }
                            KeyCode::Char(' ') => {
                                app.toggle_selection();
                            }
                            KeyCode::Char('a') => {
                                app.toggle_all();
                            }
                            KeyCode::Char('c') => {
                                if !app.selected_appids.is_empty() {
                                    app.state = AppState::ConfirmingDeletion;
                                } else {
                                    app.status_message = "No prefixes selected to clean.".to_string();
                                }
                            }
                            _ => {}
                        },
                        AppState::ConfirmingDeletion => match key.code {
                            KeyCode::Char('y') | KeyCode::Char('Y') => {
                                app.state = AppState::Cleaning;
                                let to_clean: Vec<OrphanedPrefix> = app
                                    .orphans
                                    .iter()
                                    .filter(|o| app.selected_appids.contains(&o.appid))
                                    .cloned()
                                    .collect();

                                let mut cleaned = 0;
                                for orphan in &to_clean {
                                    if let Ok(bytes) = execute_clean(orphan, &app.backup_dir, false) {
                                        cleaned += bytes;
                                    }
                                }
                                app.space_reclaimed = cleaned;
                                app.status_message = format!(
                                    "Successfully buried saves and reclaimed {}!",
                                    format_bytes(cleaned)
                                );
                                // Remove cleaned items from list
                                app.orphans.retain(|o| !app.selected_appids.contains(&o.appid));
                                app.selected_appids.clear();
                                app.cursor_index = 0;
                                app.state = AppState::Done;
                            }
                            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                app.state = AppState::Browsing;
                                app.status_message = "Deletion cancelled.".to_string();
                            }
                            _ => {}
                        },
                        AppState::Done => match key.code {
                            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') => {
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    let vdf_path = match &cli.library_vdf {
        Some(p) => p.clone(),
        None => vdf_parser::default_library_vdf_path()
            .context("Failed to locate Steam libraryfolders.vdf. Specify path using --library-vdf <PATH>")?,
    };

    let backup_dir = match &cli.backup_dir {
        Some(p) => p.clone(),
        None => backup::default_backup_root()?,
    };

    let library_folders = vdf_parser::parse_library_folders(&vdf_path)
        .with_context(|| format!("Failed to parse library folders from {:?}", vdf_path))?;

    let installed_games = vdf_parser::discover_installed_games(&library_folders)?;
    let orphans = scanner::scan_orphans(&library_folders, &installed_games)?;

    if cli.no_tui || cli.dry_run || cli.auto_clean {
        run_cli_mode(&cli, &orphans, &backup_dir)?;
    } else {
        let app = App::new(orphans, backup_dir);
        run_tui(app)?;
    }

    Ok(())
}
