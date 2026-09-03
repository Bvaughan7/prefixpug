use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Gauge, List, ListItem, Paragraph},
    Frame,
};

use super::app::{App, AppState};

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
    let title = if app.state == AppState::Filtering {
        format!(" [ Filter: {}_ ] ", app.filter_query)
    } else if !app.filter_query.is_empty() {
        format!(" [ Sniffed Orphans (Filter: {}) ] ", app.filter_query)
    } else {
        " [ Sniffed Orphan Prefixes ] ".to_string()
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
        let empty_p = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(msg, Style::default().fg(NEON_YELLOW))),
        ])
        .block(block);
        f.render_widget(empty_p, area);
        return;
    }

    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .enumerate()
        .map(|(disp_idx, &actual_idx)| {
            let orphan = &app.all_orphans[actual_idx];
            let is_cursor = disp_idx == app.cursor_index;
            let is_selected = app.selected_appids.contains(&orphan.appid);

            let checkbox = if is_selected {
                Span::styled(
                    "[■] ",
                    Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled("[ ] ", Style::default().fg(Color::DarkGray))
            };

            let appid_text = format!("AppID: {:<7}", orphan.appid);
            let name_text = match &orphan.title {
                Some(t) => {
                    let mut s = t.clone();
                    if s.len() > 14 {
                        s.truncate(12);
                        s.push_str("..");
                    }
                    format!(" {:<14}", s)
                }
                None => " {:<14}".replace("{:<14}", " (unknown)    "),
            };

            let appid_span = Span::styled(
                appid_text,
                if is_cursor {
                    Style::default()
                        .fg(NEON_YELLOW)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                },
            );

            let name_span = Span::styled(name_text, Style::default().fg(NEON_PURPLE));

            let size_span = Span::styled(
                format!(" {:>9}", format_bytes(orphan.total_size())),
                Style::default().fg(NEON_CYAN),
            );

            let saves_span = if !orphan.detected_saves.is_empty() {
                Span::styled(
                    format!(" 🦴 {} saves", orphan.detected_saves.len()),
                    Style::default().fg(NEON_PINK),
                )
            } else {
                Span::styled("   --        ", Style::default().fg(Color::DarkGray))
            };

            let indicator = if is_cursor {
                Span::styled(
                    "▶ ",
                    Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD),
                )
            } else {
                Span::raw("  ")
            };

            let spans = vec![
                indicator, checkbox, appid_span, name_span, size_span, saves_span,
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
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(11), Constraint::Min(6)])
        .split(area);

    // Neon Cyberpug Mascot with animated sniffing frames
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

    // Inspector Pane
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
            ]),
            Line::from(vec![
                Span::styled("Title:        ", Style::default().fg(Color::Gray)),
                Span::styled(
                    orphan.title.as_deref().unwrap_or("Unknown / Unindexed"),
                    Style::default().fg(NEON_CYAN).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Compatdata:   ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format_bytes(orphan.compatdata_size),
                    Style::default().fg(NEON_CYAN),
                ),
            ]),
            Line::from(vec![
                Span::styled("Shadercache:  ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format_bytes(orphan.shadercache_size),
                    Style::default().fg(NEON_CYAN),
                ),
            ]),
            Line::from(vec![
                Span::styled("Total Size:   ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format_bytes(orphan.total_size()),
                    Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                format!("Sniffed Save Files ({}):", orphan.detected_saves.len()),
                Style::default().fg(NEON_YELLOW),
            )),
        ];

        for save in orphan.detected_saves.iter().take(4) {
            let path_str = save.path.to_string_lossy();
            let truncated = if path_str.len() > 36 {
                format!("...{}", &path_str[path_str.len() - 33..])
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

        if orphan.detected_saves.len() > 4 {
            lines.push(Line::from(Span::styled(
                format!(
                    "   ...and {} more save files",
                    orphan.detected_saves.len() - 4
                ),
                Style::default().fg(Color::DarkGray),
            )));
        }

        lines
    } else {
        vec![Line::from(Span::styled(
            "No prefix selected.",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let inspector_widget = Paragraph::new(details).block(inspector_block);
    f.render_widget(inspector_widget, chunks[1]);
}

fn render_reclaim_progress(f: &mut Frame, app: &App, area: Rect) {
    let total_reclaimable = app.total_orphans_size();
    let selected_size = app.selected_total_size();

    let ratio = if total_reclaimable > 0 {
        (selected_size as f64 / total_reclaimable as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let gauge_title = format!(
        " Reclamation Target: {} / {} ({:.1}%) ",
        format_bytes(selected_size),
        format_bytes(total_reclaimable),
        ratio * 100.0
    );

    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(gauge_title)
                .title_style(
                    Style::default()
                        .fg(NEON_YELLOW)
                        .add_modifier(Modifier::BOLD),
                )
                .borders(Borders::ALL)
                .border_style(Style::default().fg(NEON_CYAN)),
        )
        .gauge_style(Style::default().fg(NEON_PINK).bg(Color::Rgb(40, 20, 60)))
        .ratio(ratio);

    f.render_widget(gauge, area);
}

fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(NEON_PURPLE));

    let content = Line::from(vec![
        Span::styled(" [↑/↓] Move ", Style::default().fg(NEON_CYAN)),
        Span::styled("[Space] Toggle ", Style::default().fg(NEON_CYAN)),
        Span::styled("[a] All ", Style::default().fg(NEON_CYAN)),
        Span::styled("[i] Invert ", Style::default().fg(NEON_CYAN)),
        Span::styled("[/] Search ", Style::default().fg(NEON_CYAN)),
        Span::styled(
            "[c] Clean ",
            Style::default()
                .fg(NEON_YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("[?] Help ", Style::default().fg(NEON_GREEN)),
        Span::styled("[q] Quit │ ", Style::default().fg(NEON_PINK)),
        Span::styled(&app.status_message, Style::default().fg(Color::White)),
    ]);

    let widget = Paragraph::new(content).block(block);
    f.render_widget(widget, area);
}

fn render_confirm_dialog(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" ⚠ CONFIRM COMPATDATA PURGE ⚠ ")
        .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Red));

    let dialog_area = Rect {
        x: area.width.saturating_sub(64) / 2,
        y: area.height.saturating_sub(12) / 2,
        width: 64.min(area.width),
        height: 12.min(area.height),
    };

    let count = app.selected_appids.len();
    let bytes = app.selected_total_size();

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!(
                " Reclaiming {} across {} orphaned prefix(es).",
                format_bytes(bytes),
                count
            ),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " 🐾 The Pug's Nose will bury (backup) all saves to:",
            Style::default().fg(NEON_CYAN),
        )),
        Line::from(Span::styled(
            format!(" {:?}", app.backup_dir),
            Style::default().fg(NEON_YELLOW),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Compatdata and shader caches will be permanently purged.",
            Style::default().fg(Color::LightRed),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Press ", Style::default().fg(Color::Gray)),
            Span::styled(
                "[Y]",
                Style::default().fg(NEON_GREEN).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " to confirm and purge, or ",
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                "[N / Esc]",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to cancel.", Style::default().fg(Color::Gray)),
        ]),
    ];

    f.render_widget(Clear, dialog_area);
    let p = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);
    f.render_widget(p, dialog_area);
}

fn render_help_dialog(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" ⚡ PREFIXPUG CONTROL MATRIX & HELP ⚡ ")
        .title_style(
            Style::default()
                .fg(NEON_YELLOW)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(NEON_CYAN));

    let dialog_area = Rect {
        x: area.width.saturating_sub(68) / 2,
        y: area.height.saturating_sub(14) / 2,
        width: 68.min(area.width),
        height: 14.min(area.height),
    };

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  [↑] / [k]       ", Style::default().fg(NEON_YELLOW)),
            Span::styled(
                "Navigate up in prefix list",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  [↓] / [j]       ", Style::default().fg(NEON_YELLOW)),
            Span::styled(
                "Navigate down in prefix list",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  [Space]         ", Style::default().fg(NEON_YELLOW)),
            Span::styled(
                "Toggle selection checkbox for current prefix",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  [a]             ", Style::default().fg(NEON_YELLOW)),
            Span::styled(
                "Select all or deselect all visible prefixes",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  [i]             ", Style::default().fg(NEON_YELLOW)),
            Span::styled(
                "Invert selection across visible prefixes",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  [/]             ", Style::default().fg(NEON_YELLOW)),
            Span::styled(
                "Filter list by AppID or game name",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  [c]             ", Style::default().fg(NEON_YELLOW)),
            Span::styled(
                "Clean selected prefixes (prompts confirmation)",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  [?] / [h]       ", Style::default().fg(NEON_YELLOW)),
            Span::styled("Toggle this help screen", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  [q] / [Esc]     ", Style::default().fg(NEON_YELLOW)),
            Span::styled(
                "Exit dialog or quit application",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            " Press [Esc] or [?] to close this help window.",
            Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD),
        )),
    ];

    f.render_widget(Clear, dialog_area);
    let p = Paragraph::new(text).block(block);
    f.render_widget(p, dialog_area);
}
