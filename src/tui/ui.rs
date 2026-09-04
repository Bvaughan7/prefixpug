use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Gauge, List, ListItem, Paragraph},
    Frame,
};

use super::app::{App, AppState, SortMode};

const NEON_PINK: Color = Color::Rgb(255, 20, 147);
const NEON_CYAN: Color = Color::Rgb(0, 245, 255);
const NEON_PURPLE: Color = Color::Rgb(186, 85, 211);
const NEON_YELLOW: Color = Color::Rgb(255, 215, 0);
const NEON_GREEN: Color = Color::Rgb(50, 205, 50);

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
        Span::styled("⚡ ", Style::default().fg(NEON_YELLOW)),
        Span::styled(
            "PREFIXPUG",
            Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" :: ", Style::default().fg(NEON_CYAN)),
        Span::styled(
            "Steam/Proton Prefix Reclamation Rig",
            Style::default().fg(NEON_CYAN),
        ),
        Span::styled(" [SYNTHWAVE v0.1] ⚡", Style::default().fg(NEON_YELLOW)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(NEON_PINK));

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
        format!(" [ Filter: {}_ ] ", app.filter_query)
    } else if !app.filter_query.is_empty() {
        format!(
            " [ Prefixes (Filter: {}, Sort: {}) ] ",
            app.filter_query, sort_label
        )
    } else {
        format!(" [ Prefixes (Sort: {}) ] ", sort_label)
    };

    let block = Block::default()
        .title(title)
        .title_style(
            Style::default()
                .fg(if app.state == AppState::Filtering {
                    NEON_YELLOW
                } else {
                    NEON_CYAN
                })
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(NEON_CYAN));

    if app.filtered_indices.is_empty() {
        let msg = if app.all_orphans.is_empty() {
            "  ✨ No orphaned prefixes detected! Storage is squeaky clean."
        } else {
            "  🔍 No prefixes match the current filter."
        };
        let p = Paragraph::new(msg)
            .style(Style::default().fg(Color::Gray))
            .block(block);
        f.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .enumerate()
        .map(|(list_idx, &orphan_idx)| {
            let orphan = &app.all_orphans[orphan_idx];
            let is_cursor = list_idx == app.cursor_index;
            let is_selected = app.selected_appids.contains(&orphan.appid);

            let cursor_mark = if is_cursor { "▶" } else { " " };

            let (check_box, check_style) = if !orphan.is_deletable() {
                ("[P]", Style::default().fg(Color::DarkGray))
            } else if is_selected {
                (
                    "[■]",
                    Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD),
                )
            } else {
                ("[ ]", Style::default().fg(Color::DarkGray))
            };

            let title_display = match &orphan.title {
                Some(t) => {
                    if t.len() > 14 {
                        format!("{}...", &t[..11])
                    } else {
                        format!("{:<14}", t)
                    }
                }
                None => format!("{:<14}", "Unknown"),
            };

            let saves_span = if !orphan.detected_saves.is_empty() {
                Span::styled(
                    format!(" ★ {} saves", orphan.detected_saves.len()),
                    Style::default().fg(NEON_PINK),
                )
            } else {
                Span::styled("   --        ", Style::default().fg(Color::DarkGray))
            };

            let status_badge = Span::styled(
                format!(" {:<10}", orphan.classification.badge()),
                match orphan.classification {
                    crate::vdf_parser::PrefixClassification::Orphaned => {
                        Style::default().fg(NEON_PINK)
                    }
                    crate::vdf_parser::PrefixClassification::LiveGame(_) => {
                        Style::default().fg(NEON_GREEN)
                    }
                    crate::vdf_parser::PrefixClassification::NonSteamShortcut(_) => {
                        Style::default().fg(NEON_CYAN)
                    }
                    crate::vdf_parser::PrefixClassification::SteamInfrastructure(_) => {
                        Style::default().fg(NEON_PURPLE)
                    }
                    crate::vdf_parser::PrefixClassification::Unknown => {
                        Style::default().fg(Color::DarkGray)
                    }
                },
            );

            let spans = vec![
                Span::styled(
                    format!("{} ", cursor_mark),
                    Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{} ", check_box), check_style),
                Span::styled(
                    format!("AppID: {:<7} ", orphan.appid),
                    Style::default().fg(NEON_YELLOW),
                ),
                Span::styled(
                    format!("{} ", title_display),
                    Style::default()
                        .fg(if is_cursor { Color::White } else { NEON_PURPLE })
                        .add_modifier(if is_cursor {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::styled(
                    format!(" {:>9}", format_bytes(orphan.total_size())),
                    Style::default().fg(NEON_CYAN),
                ),
                saves_span,
                status_badge,
            ];

            let line = Line::from(spans);
            let item = ListItem::new(line);

            if is_cursor {
                item.style(Style::default().bg(Color::Rgb(35, 20, 50)))
            } else {
                item
            }
        })
        .collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_pug_and_details(f: &mut Frame, app: &App, area: Rect) {
    if app.show_mascot {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(11), Constraint::Min(6)])
            .split(area);

        // Mascot
        let anim_cycle = (app.animation_frame / 4) % 6;
        let (snout, particles, action_text) = match anim_cycle {
            0 => ("( •ᴥ•) ", "  *sniff*     ", "Sniffing prefix storage..."),
            1 => ("( •ᴥ•) ", "  ~ ~ *snort* ", "Scanning Wine registry..."),
            2 => ("( ⊙ᴥ⊙)", "  **SNIFF!**  ", "Digging up save candidates!"),
            3 => ("( ⊙ᴥ⊙)", "  ~ *dig dig* ", "Burying saves in vault..."),
            4 => ("( -ᴥ- )", "  zzz...      ", "Reclaiming NVMe sectors..."),
            _ => ("( ◕ᴥ◕)", "  ~ *pant*    ", "Cyberpug is on the prowl."),
        };

        let pug_art = vec![
            Line::from(vec![Span::styled(
                "      ┌──────────────────────┐",
                Style::default().fg(NEON_PINK),
            )]),
            Line::from(vec![
                Span::styled(" /\\_/\\│ ", Style::default().fg(NEON_CYAN)),
                Span::styled(
                    "CYBERPUG NEON M-01",
                    Style::default()
                        .fg(NEON_YELLOW)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("   │", Style::default().fg(NEON_PINK)),
            ]),
            Line::from(vec![
                Span::styled(
                    format!(" {}│ ", snout),
                    Style::default().fg(NEON_CYAN).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    particles,
                    Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD),
                ),
                Span::styled("      │", Style::default().fg(NEON_PINK)),
            ]),
            Line::from(vec![
                Span::styled(" /    \\│ ", Style::default().fg(NEON_CYAN)),
                Span::styled(action_text, Style::default().fg(NEON_PURPLE)),
            ]),
            Line::from(vec![
                Span::styled("( \"  \" )", Style::default().fg(NEON_CYAN)),
                Span::styled("└──────────────────────┘", Style::default().fg(NEON_PINK)),
            ]),
        ];

        let pug_block = Block::default()
            .title(" [ Mascot: Cyberpug ] ")
            .title_style(Style::default().fg(NEON_PINK))
            .borders(Borders::ALL)
            .border_type(BorderType::Thick)
            .border_style(Style::default().fg(NEON_PINK));

        let pug_widget = Paragraph::new(pug_art).block(pug_block);
        f.render_widget(pug_widget, chunks[0]);

        render_inspector(f, app, chunks[1]);
    } else {
        render_inspector(f, app, area);
    }
}

fn render_inspector(f: &mut Frame, app: &App, area: Rect) {
    let inspector_block = Block::default()
        .title(" [ Prefix Inspector ] ")
        .title_style(Style::default().fg(NEON_CYAN))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(NEON_PURPLE));

    let details = if let Some(orphan) = app.current_orphan() {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("AppID:        ", Style::default().fg(Color::Gray)),
                Span::styled(
                    &orphan.appid,
                    Style::default()
                        .fg(NEON_YELLOW)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("  Status: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    orphan.classification.badge(),
                    match orphan.classification {
                        crate::vdf_parser::PrefixClassification::Orphaned => {
                            Style::default().fg(NEON_PINK)
                        }
                        crate::vdf_parser::PrefixClassification::LiveGame(_) => {
                            Style::default().fg(NEON_GREEN)
                        }
                        crate::vdf_parser::PrefixClassification::NonSteamShortcut(_) => {
                            Style::default().fg(NEON_CYAN)
                        }
                        _ => Style::default().fg(NEON_PURPLE),
                    },
                ),
            ]),
            Line::from(vec![
                Span::styled("Title:        ", Style::default().fg(Color::Gray)),
                Span::styled(
                    orphan.title.as_deref().unwrap_or("Unknown / Unindexed"),
                    Style::default().fg(NEON_CYAN).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Apparent:     ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format_bytes(orphan.total_apparent_bytes()),
                    Style::default().fg(NEON_CYAN),
                ),
                Span::styled("  Allocated: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format_bytes(orphan.total_allocated_bytes()),
                    Style::default().fg(NEON_PURPLE),
                ),
            ]),
            Line::from(vec![
                Span::styled("Last Touched: ", Style::default().fg(Color::Gray)),
                Span::styled(orphan.age_display(), Style::default().fg(NEON_YELLOW)),
            ]),
            Line::from(vec![
                Span::styled("Steam Cloud:  ", Style::default().fg(Color::Gray)),
                Span::styled(
                    if orphan.cloud_status.is_synced() {
                        "☁ Synced (Remote Cloud Backup Available)"
                    } else {
                        "⚠ Not Detected (Local Saves Are ONLY Copy)"
                    },
                    if orphan.cloud_status.is_synced() {
                        Style::default().fg(NEON_GREEN)
                    } else {
                        Style::default().fg(NEON_YELLOW)
                    },
                ),
            ]),
        ];

        if orphan.is_high_value {
            lines.push(Line::from(vec![
                Span::styled(
                    "⚠ WARNING:    ",
                    Style::default()
                        .fg(NEON_YELLOW)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "High-value prefix (mods / protontricks detected)",
                    Style::default().fg(NEON_YELLOW),
                ),
            ]));
            for reason in &orphan.high_value_reasons {
                lines.push(Line::from(vec![
                    Span::styled("   • ", Style::default().fg(NEON_YELLOW)),
                    Span::styled(reason, Style::default().fg(Color::LightYellow)),
                ]));
            }
        }

        for warning in &orphan.warnings {
            lines.push(Line::from(vec![
                Span::styled(" ⚠ ", Style::default().fg(NEON_YELLOW)),
                Span::styled(warning, Style::default().fg(Color::LightRed)),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("Sniffed Save Files ({}):", orphan.detected_saves.len()),
            Style::default().fg(NEON_YELLOW),
        )));

        for save in orphan.detected_saves.iter().take(6) {
            let path_str = save.path.to_string_lossy();
            let truncated = if path_str.len() > 38 {
                format!("...{}", &path_str[path_str.len() - 35..])
            } else {
                path_str.to_string()
            };
            lines.push(Line::from(vec![
                Span::styled(" • ", Style::default().fg(NEON_PINK)),
                Span::styled(truncated, Style::default().fg(Color::LightCyan)),
                Span::styled(
                    format!(" ({})", format_bytes(save.size_bytes)),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        if orphan.detected_saves.len() > 6 {
            lines.push(Line::from(Span::styled(
                format!("   ... and {} more files", orphan.detected_saves.len() - 6),
                Style::default().fg(Color::DarkGray),
            )));
        }

        lines
    } else {
        vec![Line::from(Span::styled(
            "Select a prefix from the list to inspect details.",
            Style::default().fg(Color::Gray),
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
        "Reclamation Target: {} / {} ({:.1}%)",
        format_bytes(selected_bytes),
        format_bytes(total_reclaimable),
        ratio * 100.0
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(NEON_CYAN))
        .title(Span::styled(
            label,
            Style::default()
                .fg(NEON_YELLOW)
                .add_modifier(Modifier::BOLD),
        ));

    let gauge = Gauge::default()
        .block(block)
        .gauge_style(
            Style::default()
                .fg(NEON_PINK)
                .bg(Color::Rgb(30, 15, 45))
                .add_modifier(Modifier::BOLD),
        )
        .percent((ratio * 100.0) as u16);

    f.render_widget(gauge, area);
}

fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(NEON_PURPLE));

    let shortcuts = Line::from(vec![
        Span::styled("[↑/↓] ", Style::default().fg(NEON_CYAN)),
        Span::styled("Move ", Style::default().fg(Color::Gray)),
        Span::styled("[Space] ", Style::default().fg(NEON_CYAN)),
        Span::styled("Toggle ", Style::default().fg(Color::Gray)),
        Span::styled("[a] ", Style::default().fg(NEON_CYAN)),
        Span::styled("All ", Style::default().fg(Color::Gray)),
        Span::styled("[s] ", Style::default().fg(NEON_CYAN)),
        Span::styled("Sort ", Style::default().fg(Color::Gray)),
        Span::styled("[m] ", Style::default().fg(NEON_CYAN)),
        Span::styled("Mascot ", Style::default().fg(Color::Gray)),
        Span::styled("[c] ", Style::default().fg(NEON_YELLOW)),
        Span::styled("Clean ", Style::default().fg(NEON_YELLOW)),
        Span::styled("[?] ", Style::default().fg(NEON_GREEN)),
        Span::styled("Help ", Style::default().fg(Color::Gray)),
        Span::styled("[q] ", Style::default().fg(NEON_PINK)),
        Span::styled("Quit ", Style::default().fg(Color::Gray)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled(&app.status_message, Style::default().fg(Color::White)),
    ]);

    let p = Paragraph::new(shortcuts).block(block);
    f.render_widget(p, area);
}

fn render_confirm_dialog(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" ⚠ CONFIRM COMPATDATA PURGE ⚠ ")
        .title_style(
            Style::default()
                .fg(Color::Rgb(255, 69, 58))
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Rgb(255, 69, 58)));

    let dialog_area = centered_rect(60, 45, area);
    f.render_widget(Clear, dialog_area);

    let selected_count = app.selected_appids.len();
    let reclaim_bytes = app.selected_total_size();

    let mut text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Reclaiming ", Style::default().fg(Color::White)),
            Span::styled(
                format_bytes(reclaim_bytes),
                Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" across {} prefix(es).", selected_count),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  ⚡ The Pug's Nose will bury (backup) all saves to:",
            Style::default().fg(NEON_CYAN),
        )),
        Line::from(Span::styled(
            format!("     \"{}\"", app.backup_dir.display()),
            Style::default().fg(NEON_YELLOW),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Compatdata and shader caches will be permanently purged.",
            Style::default().fg(Color::Rgb(255, 120, 120)),
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
            Style::default()
                .fg(NEON_YELLOW)
                .add_modifier(Modifier::BOLD),
        )));
    }

    text.push(Line::from(""));
    text.push(Line::from(vec![
        Span::styled("  Press ", Style::default().fg(Color::White)),
        Span::styled(
            "[Y]",
            Style::default().fg(NEON_GREEN).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " to confirm and purge, or ",
            Style::default().fg(Color::White),
        ),
        Span::styled(
            "[N / Esc]",
            Style::default()
                .fg(Color::Rgb(255, 69, 58))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" to cancel.", Style::default().fg(Color::White)),
    ]));

    let p = Paragraph::new(text).block(block);
    f.render_widget(p, dialog_area);
}

fn render_help_dialog(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" [ Control Matrix ] ")
        .title_style(Style::default().fg(NEON_GREEN).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(NEON_GREEN));

    let dialog_area = centered_rect(65, 60, area);
    f.render_widget(Clear, dialog_area);

    let help_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  ↑ / k            ", Style::default().fg(NEON_CYAN)),
            Span::styled(
                "Navigate up in prefix list",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  ↓ / j            ", Style::default().fg(NEON_CYAN)),
            Span::styled(
                "Navigate down in prefix list",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Space            ", Style::default().fg(NEON_CYAN)),
            Span::styled(
                "Toggle prefix selection for cleanup",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  a                ", Style::default().fg(NEON_CYAN)),
            Span::styled(
                "Select/Deselect all visible prefixes",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  i                ", Style::default().fg(NEON_CYAN)),
            Span::styled(
                "Invert current selection",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  s                ", Style::default().fg(NEON_CYAN)),
            Span::styled(
                "Cycle sort: Size → Age → AppID",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  m                ", Style::default().fg(NEON_CYAN)),
            Span::styled(
                "Toggle Cyberpug mascot display",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  /                ", Style::default().fg(NEON_CYAN)),
            Span::styled(
                "Search/filter by AppID or game title",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  c                ", Style::default().fg(NEON_YELLOW)),
            Span::styled(
                "Trigger save vaulting & prefix cleanup",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  ? / h            ", Style::default().fg(NEON_GREEN)),
            Span::styled("Toggle this help dialog", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  q / Esc          ", Style::default().fg(NEON_PINK)),
            Span::styled(
                "Quit application or close modal",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Press any key to return to dashboard.",
            Style::default().fg(Color::DarkGray),
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
