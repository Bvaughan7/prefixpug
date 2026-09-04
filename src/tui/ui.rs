use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Gauge, List, ListItem, Paragraph},
    Frame,
};

use super::app::{App, AppState, SortMode};

// Modern, understated terminal palette (slate / cyan / warm amber / soft emerald / rose)
const COLOR_ACCENT: Color = Color::Rgb(56, 189, 248); // Sky-400
const COLOR_PRIMARY: Color = Color::Rgb(129, 140, 248); // Indigo-400
const COLOR_WARN: Color = Color::Rgb(251, 191, 36); // Amber-400
const COLOR_SUCCESS: Color = Color::Rgb(74, 222, 128); // Emerald-400
const COLOR_DANGER: Color = Color::Rgb(244, 63, 94); // Rose-500
const COLOR_BORDER: Color = Color::Rgb(71, 85, 105); // Slate-600
const COLOR_TEXT: Color = Color::Rgb(226, 232, 240); // Slate-200
const COLOR_MUTED: Color = Color::Rgb(148, 163, 184); // Slate-400

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

pub fn render(f: &mut Frame, app: &App) {
    let size = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(12),   // Main columns
            Constraint::Length(4), // Progress Bar
            Constraint::Length(3), // Status & Shortcuts
        ])
        .split(size);

    render_header(f, chunks[0]);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(chunks[1]);

    render_orphan_list(f, app, main_chunks[0]);
    render_pug_and_details(f, app, main_chunks[1]);
    render_reclaim_progress(f, app, chunks[2]);
    render_status_bar(f, app, chunks[3]);

    match app.state {
        AppState::ConfirmingDeletion => render_confirm_dialog(f, app, size),
        AppState::ShowingHelp => render_help_dialog(f, size),
        _ => {}
    }
}

fn render_header(f: &mut Frame, area: Rect) {
    let title = Line::from(vec![
        Span::styled(
            "PREFIXPUG",
            Style::default()
                .fg(COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" — ", Style::default().fg(COLOR_BORDER)),
        Span::styled(
            "Steam/Proton Prefix Reclamation",
            Style::default().fg(COLOR_TEXT),
        ),
        Span::styled(
            format!(" [v{}]", env!("CARGO_PKG_VERSION")),
            Style::default().fg(COLOR_MUTED),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER));

    let header_widget = Paragraph::new(title)
        .block(block)
        .alignment(Alignment::Center);
    f.render_widget(header_widget, area);
}

fn render_orphan_list(f: &mut Frame, app: &App, area: Rect) {
    let sort_label = match app.sort_mode {
        SortMode::Size => "Size",
        SortMode::Age => "Age",
        SortMode::AppId => "AppID",
    };

    let title = if app.state == AppState::Filtering {
        format!(" Filter: {}_ ", app.filter_query)
    } else if !app.filter_query.is_empty() {
        format!(
            " Prefixes (Filter: {}, Sort: {}) ",
            app.filter_query, sort_label
        )
    } else {
        format!(" Prefixes (Sort: {}) ", sort_label)
    };

    let border_color = if app.state == AppState::Filtering {
        COLOR_WARN
    } else {
        COLOR_ACCENT
    };

    let block = Block::default()
        .title(title)
        .title_style(
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER));

    if app.filtered_indices.is_empty() {
        let msg = if app.all_orphans.is_empty() {
            "  No orphaned prefixes detected. Storage is clean."
        } else {
            "  No prefixes match the current filter."
        };
        let p = Paragraph::new(msg)
            .style(Style::default().fg(COLOR_MUTED))
            .block(block);
        f.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .enumerate()
        .map(|(display_idx, &real_idx)| {
            let orphan = &app.all_orphans[real_idx];
            let is_selected = app.selected_appids.contains(&orphan.appid);
            let is_cursor = display_idx == app.cursor_index;

            let checkbox = match (orphan.is_deletable(), is_selected) {
                (false, _) => Span::styled(" [LOCK] ", Style::default().fg(COLOR_BORDER)),
                (true, true) => Span::styled(
                    "  [■]   ",
                    Style::default()
                        .fg(COLOR_DANGER)
                        .add_modifier(Modifier::BOLD),
                ),
                (true, false) => Span::styled("  [ ]   ", Style::default().fg(COLOR_MUTED)),
            };

            let appid_span = Span::styled(
                format!("{:<9} ", orphan.appid),
                if is_cursor {
                    Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(COLOR_MUTED)
                },
            );

            let high_val_mark = if orphan.is_high_value {
                Span::styled(
                    " [HIGH-VAL]",
                    Style::default().fg(COLOR_WARN).add_modifier(Modifier::BOLD),
                )
            } else {
                Span::raw("")
            };

            let (badge_text, badge_color) = match orphan.classification {
                crate::vdf_parser::PrefixClassification::Orphaned => ("[ORPHAN]  ", COLOR_DANGER),
                crate::vdf_parser::PrefixClassification::LiveGame(_) => {
                    ("[LIVE]    ", COLOR_SUCCESS)
                }
                crate::vdf_parser::PrefixClassification::NonSteamShortcut(_) => {
                    ("[SHORTCUT]", COLOR_ACCENT)
                }
                crate::vdf_parser::PrefixClassification::SteamInfrastructure(_) => {
                    ("[RUNTIME] ", COLOR_PRIMARY)
                }
                crate::vdf_parser::PrefixClassification::Unknown => ("[UNKNOWN] ", COLOR_BORDER),
            };
            let badge_span = Span::styled(badge_text, Style::default().fg(badge_color));

            let title_str = orphan.title.as_deref().unwrap_or("Unknown / Unindexed");
            let title_span = Span::styled(
                format!(" {:<20}", title_str),
                if is_cursor {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(COLOR_TEXT)
                },
            );

            let size_span = Span::styled(
                format!("{:>10}", format_bytes(orphan.total_apparent_bytes())),
                Style::default().fg(COLOR_WARN),
            );

            let age_span = Span::styled(
                format!("{:>9}", orphan.age_display()),
                Style::default().fg(if is_cursor { Color::White } else { COLOR_MUTED }),
            );

            let saves_span = if orphan.detected_saves.is_empty() {
                Span::styled("   -  ", Style::default().fg(COLOR_BORDER))
            } else {
                Span::styled(
                    format!("  {:>2} sv", orphan.detected_saves.len()),
                    Style::default().fg(COLOR_SUCCESS),
                )
            };

            let line = Line::from(vec![
                checkbox,
                badge_span,
                Span::raw(" "),
                appid_span,
                title_span,
                size_span,
                age_span,
                saves_span,
                high_val_mark,
            ]);

            let item_style = if is_cursor {
                Style::default()
                    .bg(Color::Rgb(30, 41, 59))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(line).style(item_style)
        })
        .collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_pug_and_details(f: &mut Frame, app: &App, area: Rect) {
    if app.show_mascot {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(9), Constraint::Min(6)])
            .split(area);

        // Mascot: clean, charming ASCII pug sentry
        let anim_cycle = (app.animation_frame / 6) % 4;
        let (eyes, action) = match anim_cycle {
            0 => ("(  o . o  )", "Sniffing save vaults..."),
            1 => ("(  ^ . ^  )", "All saves accounted for."),
            2 => ("(  • . •  )", "Scanning directories..."),
            _ => ("(  - . -  )", "Standing by."),
        };

        let pug_art = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("       ___   ___  ", Style::default().fg(COLOR_ACCENT)),
                Span::styled(
                    "   PrefixPug Sentry",
                    Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("      /   \\_/   \\ ", Style::default().fg(COLOR_ACCENT)),
                Span::styled(format!("   {}", action), Style::default().fg(COLOR_MUTED)),
            ]),
            Line::from(vec![
                Span::styled(
                    format!("     {} ", eyes),
                    Style::default()
                        .fg(COLOR_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "   Vault: ~/.local/share/prefixpug/backups/",
                    Style::default().fg(COLOR_BORDER),
                ),
            ]),
            Line::from(vec![
                Span::styled("      (   =v=   ) ", Style::default().fg(COLOR_ACCENT)),
                Span::styled(
                    "   Press [m] to hide mascot",
                    Style::default().fg(COLOR_BORDER),
                ),
            ]),
            Line::from(vec![Span::styled(
                "       \\_______/  ",
                Style::default().fg(COLOR_ACCENT),
            )]),
        ];

        let pug_block = Block::default()
            .title(" PrefixPug Sentry ")
            .title_style(Style::default().fg(COLOR_ACCENT))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_BORDER));

        let pug_widget = Paragraph::new(pug_art).block(pug_block);
        f.render_widget(pug_widget, chunks[0]);

        render_inspector(f, app, chunks[1]);
    } else {
        render_inspector(f, app, area);
    }
}

fn render_inspector(f: &mut Frame, app: &App, area: Rect) {
    let inspector_block = Block::default()
        .title(" Prefix Inspector ")
        .title_style(Style::default().fg(COLOR_ACCENT))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER));

    let details = if let Some(orphan) = app.current_orphan() {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("AppID:        ", Style::default().fg(COLOR_MUTED)),
                Span::styled(
                    &orphan.appid,
                    Style::default().fg(COLOR_WARN).add_modifier(Modifier::BOLD),
                ),
                Span::styled("  Status: ", Style::default().fg(COLOR_MUTED)),
                Span::styled(
                    orphan.classification.badge(),
                    match orphan.classification {
                        crate::vdf_parser::PrefixClassification::Orphaned => {
                            Style::default().fg(COLOR_DANGER)
                        }
                        crate::vdf_parser::PrefixClassification::LiveGame(_) => {
                            Style::default().fg(COLOR_SUCCESS)
                        }
                        crate::vdf_parser::PrefixClassification::NonSteamShortcut(_) => {
                            Style::default().fg(COLOR_ACCENT)
                        }
                        _ => Style::default().fg(COLOR_PRIMARY),
                    },
                ),
            ]),
            Line::from(vec![
                Span::styled("Title:        ", Style::default().fg(COLOR_MUTED)),
                Span::styled(
                    orphan.title.as_deref().unwrap_or("Unknown / Unindexed"),
                    Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Apparent:     ", Style::default().fg(COLOR_MUTED)),
                Span::styled(
                    format_bytes(orphan.total_apparent_bytes()),
                    Style::default().fg(COLOR_TEXT),
                ),
                Span::styled("  Allocated: ", Style::default().fg(COLOR_MUTED)),
                Span::styled(
                    format_bytes(orphan.total_size()),
                    Style::default().fg(COLOR_WARN).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Last Touched: ", Style::default().fg(COLOR_MUTED)),
                Span::styled(orphan.age_display(), Style::default().fg(COLOR_WARN)),
            ]),
            Line::from(vec![
                Span::styled("Steam Cloud:  ", Style::default().fg(COLOR_MUTED)),
                match orphan.cloud_status {
                    crate::vdf_parser::SteamCloudStatus::Synced => {
                        Span::styled("Synced upstream ✓", Style::default().fg(COLOR_SUCCESS))
                    }
                    crate::vdf_parser::SteamCloudStatus::NotDetected => {
                        Span::styled("Not detected (Local only)", Style::default().fg(COLOR_WARN))
                    }
                },
            ]),
            Line::from(vec![
                Span::styled("Library:      ", Style::default().fg(COLOR_MUTED)),
                Span::styled(
                    orphan.library_path.display().to_string(),
                    Style::default().fg(COLOR_MUTED),
                ),
            ]),
        ];

        if let Some(cp) = &orphan.compatdata_path {
            lines.push(Line::from(vec![
                Span::styled("Compatdata:   ", Style::default().fg(COLOR_MUTED)),
                Span::styled(
                    format!(
                        "{} ({})",
                        cp.display(),
                        format_bytes(orphan.compatdata_usage.allocated_bytes)
                    ),
                    Style::default().fg(COLOR_MUTED),
                ),
            ]));
        }

        if let Some(sp) = &orphan.shadercache_path {
            lines.push(Line::from(vec![
                Span::styled("Shader Cache: ", Style::default().fg(COLOR_MUTED)),
                Span::styled(
                    format!(
                        "{} ({})",
                        sp.display(),
                        format_bytes(orphan.shadercache_usage.allocated_bytes)
                    ),
                    Style::default().fg(COLOR_MUTED),
                ),
            ]));
        }

        if orphan.is_high_value {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(
                    "High-Value Prefix: ",
                    Style::default().fg(COLOR_WARN).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    orphan.high_value_reasons.join(", "),
                    Style::default().fg(COLOR_TEXT),
                ),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Detected Save Files: ", Style::default().fg(COLOR_ACCENT)),
            Span::styled(
                format!("{} files", orphan.detected_saves.len()),
                Style::default().fg(if orphan.detected_saves.is_empty() {
                    COLOR_MUTED
                } else {
                    COLOR_SUCCESS
                }),
            ),
        ]));

        for save in orphan.detected_saves.iter().take(6) {
            let fname = save
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("save");
            lines.push(Line::from(Span::styled(
                format!(" • {} ({})", fname, format_bytes(save.size_bytes)),
                Style::default().fg(COLOR_MUTED),
            )));
        }

        if orphan.detected_saves.len() > 6 {
            lines.push(Line::from(Span::styled(
                format!("   ... and {} more files", orphan.detected_saves.len() - 6),
                Style::default().fg(COLOR_MUTED),
            )));
        }

        lines
    } else if app.all_orphans.is_empty() {
        vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  Storage Clean",
                Style::default()
                    .fg(COLOR_SUCCESS)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "  No abandoned or orphaned Steam prefixes found.",
                Style::default().fg(COLOR_TEXT),
            )]),
            Line::from(vec![Span::styled(
                "  All Wine/Proton prefixes belong to live games or shortcuts.",
                Style::default().fg(COLOR_MUTED),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "  Quick Actions:",
                Style::default()
                    .fg(COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![
                Span::styled("   • ", Style::default().fg(COLOR_PRIMARY)),
                Span::styled(
                    "prefixpug audit",
                    Style::default().fg(COLOR_WARN).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "        Inventory all installed games",
                    Style::default().fg(COLOR_MUTED),
                ),
            ]),
            Line::from(vec![
                Span::styled("   • ", Style::default().fg(COLOR_PRIMARY)),
                Span::styled(
                    "prefixpug audit --stale",
                    Style::default().fg(COLOR_WARN).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "  Audit games untouched >90 days",
                    Style::default().fg(COLOR_MUTED),
                ),
            ]),
            Line::from(vec![
                Span::styled("   • ", Style::default().fg(COLOR_PRIMARY)),
                Span::styled(
                    "prefixpug shaders",
                    Style::default().fg(COLOR_WARN).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "      Reclaim space from GPU caches",
                    Style::default().fg(COLOR_MUTED),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Press ", Style::default().fg(COLOR_MUTED)),
                Span::styled(
                    "[q]",
                    Style::default()
                        .fg(COLOR_DANGER)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" or ", Style::default().fg(COLOR_MUTED)),
                Span::styled(
                    "[Esc]",
                    Style::default()
                        .fg(COLOR_DANGER)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" to exit.", Style::default().fg(COLOR_MUTED)),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            "Select a prefix from the list to inspect details.",
            Style::default().fg(COLOR_MUTED),
        ))]
    };

    let p = Paragraph::new(details).block(inspector_block);
    f.render_widget(p, area);
}

fn render_reclaim_progress(f: &mut Frame, app: &App, area: Rect) {
    let total_reclaimable = app.total_orphans_size();
    let selected_bytes = app.selected_total_size();

    let ratio = if total_reclaimable > 0 {
        (selected_bytes as f64 / total_reclaimable as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let label = format!(
        " Reclamation Target: {} / {} ({:.1}%) ",
        format_bytes(selected_bytes),
        format_bytes(total_reclaimable),
        ratio * 100.0
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER))
        .title(Span::styled(
            label,
            Style::default()
                .fg(COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        ));

    let gauge = Gauge::default()
        .block(block)
        .gauge_style(
            Style::default()
                .fg(COLOR_ACCENT)
                .bg(Color::Rgb(30, 41, 59))
                .add_modifier(Modifier::BOLD),
        )
        .percent((ratio * 100.0) as u16);

    f.render_widget(gauge, area);
}

fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER));

    let shortcuts = Line::from(vec![
        Span::styled("[↑/↓] ", Style::default().fg(COLOR_ACCENT)),
        Span::styled("Move ", Style::default().fg(COLOR_MUTED)),
        Span::styled("[Space] ", Style::default().fg(COLOR_ACCENT)),
        Span::styled("Toggle ", Style::default().fg(COLOR_MUTED)),
        Span::styled("[a] ", Style::default().fg(COLOR_ACCENT)),
        Span::styled("All ", Style::default().fg(COLOR_MUTED)),
        Span::styled("[s] ", Style::default().fg(COLOR_ACCENT)),
        Span::styled("Sort ", Style::default().fg(COLOR_MUTED)),
        Span::styled("[m] ", Style::default().fg(COLOR_ACCENT)),
        Span::styled("Mascot ", Style::default().fg(COLOR_MUTED)),
        Span::styled("[c] ", Style::default().fg(COLOR_WARN)),
        Span::styled("Clean ", Style::default().fg(COLOR_WARN)),
        Span::styled("[?] ", Style::default().fg(COLOR_SUCCESS)),
        Span::styled("Help ", Style::default().fg(COLOR_MUTED)),
        Span::styled("[q] ", Style::default().fg(COLOR_DANGER)),
        Span::styled("Quit ", Style::default().fg(COLOR_MUTED)),
        Span::styled("│ ", Style::default().fg(COLOR_BORDER)),
        Span::styled(&app.status_message, Style::default().fg(COLOR_TEXT)),
    ]);

    let p = Paragraph::new(shortcuts).block(block);
    f.render_widget(p, area);
}

fn render_confirm_dialog(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Confirm Compatdata Purge ")
        .title_style(
            Style::default()
                .fg(COLOR_DANGER)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_DANGER));

    let dialog_area = centered_rect(60, 45, area);
    f.render_widget(Clear, dialog_area);

    let selected_count = app.selected_appids.len();
    let reclaim_bytes = app.selected_total_size();

    let mut text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Reclaiming ", Style::default().fg(COLOR_TEXT)),
            Span::styled(
                format_bytes(reclaim_bytes),
                Style::default().fg(COLOR_WARN).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" across {} prefix(es).", selected_count),
                Style::default().fg(COLOR_TEXT),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  The Pug's Nose will archive all detected saves to:",
            Style::default().fg(COLOR_ACCENT),
        )),
        Line::from(Span::styled(
            format!("     \"{}\"", app.backup_dir.display()),
            Style::default().fg(COLOR_WARN),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Selected compatdata and shader caches will be purged.",
            Style::default().fg(COLOR_MUTED),
        )),
    ];

    // Check if any selected is high-value
    let has_high_value = app
        .all_orphans
        .iter()
        .any(|o| app.selected_appids.contains(&o.appid) && o.is_high_value);

    if has_high_value {
        text.push(Line::from(""));
        text.push(Line::from(Span::styled(
            "  ⚠ WARNING: High-value prefixes (mod loaders / protontricks) selected!",
            Style::default().fg(COLOR_WARN).add_modifier(Modifier::BOLD),
        )));
    }

    text.push(Line::from(""));
    text.push(Line::from(vec![
        Span::styled("  Press ", Style::default().fg(COLOR_TEXT)),
        Span::styled(
            "[Y]",
            Style::default()
                .fg(COLOR_SUCCESS)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " to confirm and purge, or ",
            Style::default().fg(COLOR_TEXT),
        ),
        Span::styled(
            "[N / Esc]",
            Style::default()
                .fg(COLOR_DANGER)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" to cancel.", Style::default().fg(COLOR_TEXT)),
    ]));

    let p = Paragraph::new(text).block(block);
    f.render_widget(p, dialog_area);
}

fn render_help_dialog(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" Controls & Shortcuts ")
        .title_style(
            Style::default()
                .fg(COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER));

    let dialog_area = centered_rect(65, 60, area);
    f.render_widget(Clear, dialog_area);

    let help_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  ↑ / k            ", Style::default().fg(COLOR_ACCENT)),
            Span::styled(
                "Navigate up in prefix list",
                Style::default().fg(COLOR_TEXT),
            ),
        ]),
        Line::from(vec![
            Span::styled("  ↓ / j            ", Style::default().fg(COLOR_ACCENT)),
            Span::styled(
                "Navigate down in prefix list",
                Style::default().fg(COLOR_TEXT),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Space            ", Style::default().fg(COLOR_ACCENT)),
            Span::styled(
                "Toggle prefix selection for cleanup",
                Style::default().fg(COLOR_TEXT),
            ),
        ]),
        Line::from(vec![
            Span::styled("  a                ", Style::default().fg(COLOR_ACCENT)),
            Span::styled(
                "Select/Deselect all visible prefixes",
                Style::default().fg(COLOR_TEXT),
            ),
        ]),
        Line::from(vec![
            Span::styled("  i                ", Style::default().fg(COLOR_ACCENT)),
            Span::styled("Invert current selection", Style::default().fg(COLOR_TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  s                ", Style::default().fg(COLOR_ACCENT)),
            Span::styled(
                "Cycle sort: Size → Age → AppID",
                Style::default().fg(COLOR_TEXT),
            ),
        ]),
        Line::from(vec![
            Span::styled("  m                ", Style::default().fg(COLOR_ACCENT)),
            Span::styled(
                "Toggle PrefixPug mascot sentry",
                Style::default().fg(COLOR_TEXT),
            ),
        ]),
        Line::from(vec![
            Span::styled("  /                ", Style::default().fg(COLOR_ACCENT)),
            Span::styled(
                "Search/filter by AppID or game title",
                Style::default().fg(COLOR_TEXT),
            ),
        ]),
        Line::from(vec![
            Span::styled("  c                ", Style::default().fg(COLOR_WARN)),
            Span::styled(
                "Trigger save vaulting & prefix cleanup",
                Style::default().fg(COLOR_TEXT),
            ),
        ]),
        Line::from(vec![
            Span::styled("  ? / h            ", Style::default().fg(COLOR_SUCCESS)),
            Span::styled("Toggle this help dialog", Style::default().fg(COLOR_TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  q / Esc          ", Style::default().fg(COLOR_DANGER)),
            Span::styled(
                "Quit application or close modal",
                Style::default().fg(COLOR_TEXT),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Press any key to return to dashboard.",
            Style::default().fg(COLOR_MUTED),
        )),
    ];

    let p = Paragraph::new(help_text).block(block);
    f.render_widget(p, dialog_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
