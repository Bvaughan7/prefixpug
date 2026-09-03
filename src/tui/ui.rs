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

    // Base background layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title Header
            Constraint::Min(12),   // Main 2 columns
            Constraint::Length(4), // Progress & Reclaim
            Constraint::Length(3), // Status & Keybindings
        ])
        .split(size);

    render_header(f, chunks[0]);

    // Split main section into left (Orphans List) and right (Pug & Details)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(chunks[1]);

    render_orphan_list(f, app, main_chunks[0]);
    render_pug_and_details(f, app, main_chunks[1]);
    render_reclaim_progress(f, app, chunks[2]);
    render_status_bar(f, app, chunks[3]);

    if app.state == AppState::ConfirmingDeletion {
        render_confirm_dialog(f, app, size);
    }
}

fn render_header(f: &mut Frame, area: Rect) {
    let title = Line::from(vec![
        Span::styled("⚡ ", Style::default().fg(NEON_YELLOW)),
        Span::styled(
            "PREFIXPUG",
            Style::default()
                .fg(NEON_PINK)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" :: ", Style::default().fg(NEON_CYAN)),
        Span::styled(
            "Steam/Proton Prefix Reclamation Rig",
            Style::default().fg(NEON_CYAN),
        ),
        Span::styled(" ⚡", Style::default().fg(NEON_YELLOW)),
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
    let block = Block::default()
        .title(" [ Sniffed Orphan Prefixes ] ")
        .title_style(Style::default().fg(NEON_CYAN).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(NEON_CYAN));

    if app.orphans.is_empty() {
        let empty_p = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  ✨ No orphaned prefixes detected! Storage is clean.",
                Style::default().fg(NEON_YELLOW),
            )),
        ])
        .block(block);
        f.render_widget(empty_p, area);
        return;
    }

    let items: Vec<ListItem> = app
        .orphans
        .iter()
        .enumerate()
        .map(|(idx, orphan)| {
            let is_cursor = idx == app.cursor_index;
            let is_selected = app.selected_appids.contains(&orphan.appid);

            let checkbox = if is_selected {
                Span::styled("[■] ", Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD))
            } else {
                Span::styled("[ ] ", Style::default().fg(Color::DarkGray))
            };

            let appid_span = Span::styled(
                format!("AppID: {:<8}", orphan.appid),
                if is_cursor {
                    Style::default().fg(NEON_YELLOW).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                },
            );

            let size_span = Span::styled(
                format!(" {:>10}", format_bytes(orphan.total_size())),
                Style::default().fg(NEON_CYAN),
            );

            let saves_span = if !orphan.detected_saves.is_empty() {
                Span::styled(
                    format!("  🦴 {} saves", orphan.detected_saves.len()),
                    Style::default().fg(NEON_PINK),
                )
            } else {
                Span::styled("  -- no saves", Style::default().fg(Color::DarkGray))
            };

            let spans = vec![
                if is_cursor {
                    Span::styled(" ▶ ", Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD))
                } else {
                    Span::raw("   ")
                },
                checkbox,
                appid_span,
                size_span,
                saves_span,
            ];

            let line = Line::from(spans);
            let item = ListItem::new(line);

            if is_cursor {
                item.style(Style::default().bg(Color::Rgb(30, 20, 45)))
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
        .constraints([Constraint::Length(10), Constraint::Min(5)])
        .split(area);

    // Neon Cyberpug Mascot
    let sniffer_anim = match (app.animation_frame / 4) % 4 {
        0 => "  (◕ᴥ◕)  *sniff*     ",
        1 => "  ( •ᴥ•)  ~ ~ *snort* ",
        2 => "  (⊙ᴥ⊙)  *SNIFF!*    ",
        _ => "  (⚆ᴥ⚆)  ~ *digging*  ",
    };

    let pug_art = vec![
        Line::from(Span::styled("       ┌──────────┐", Style::default().fg(NEON_PINK))),
        Line::from(vec![
            Span::styled("  /\\_/\\│ ", Style::default().fg(NEON_CYAN)),
            Span::styled("CYBERPUG", Style::default().fg(NEON_YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled(" │", Style::default().fg(NEON_PINK)),
        ]),
        Line::from(Span::styled(format!(" {}", sniffer_anim), Style::default().fg(NEON_CYAN).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("  /     \\  │ Archiving saves", Style::default().fg(NEON_PURPLE))),
        Line::from(Span::styled(" (  \" \"  ) │ before byte purge", Style::default().fg(NEON_PURPLE))),
        Line::from(Span::styled("  └───────┘", Style::default().fg(NEON_PINK))),
    ];

    let pug_block = Block::default()
        .title(" [ Pug Terminal Mascot ] ")
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

    let details = if let Some(orphan) = app.orphans.get(app.cursor_index) {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Target AppID: ", Style::default().fg(Color::Gray)),
                Span::styled(&orphan.appid, Style::default().fg(NEON_YELLOW).add_modifier(Modifier::BOLD)),
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
                format!("Detected Local Saves ({}):", orphan.detected_saves.len()),
                Style::default().fg(NEON_YELLOW),
            )),
        ];

        for save in orphan.detected_saves.iter().take(4) {
            let path_str = save.to_string_lossy();
            let truncated = if path_str.len() > 38 {
                format!("...{}", &path_str[path_str.len() - 35..])
            } else {
                path_str.to_string()
            };
            lines.push(Line::from(Span::styled(
                format!("  • {}", truncated),
                Style::default().fg(Color::LightCyan),
            )));
        }

        if orphan.detected_saves.len() > 4 {
            lines.push(Line::from(Span::styled(
                format!("  ...and {} more save files", orphan.detected_saves.len() - 4),
                Style::default().fg(Color::DarkGray),
            )));
        }

        lines
    } else {
        vec![Line::from("Select a prefix to inspect.")]
    };

    let inspector_widget = Paragraph::new(details).block(inspector_block);
    f.render_widget(inspector_widget, chunks[1]);
}

fn render_reclaim_progress(f: &mut Frame, app: &App, area: Rect) {
    let total_reclaimable: u64 = app.orphans.iter().map(|o| o.total_size()).sum();
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
                .title_style(Style::default().fg(NEON_YELLOW).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(NEON_CYAN)),
        )
        .gauge_style(
            Style::default()
                .fg(NEON_PINK)
                .bg(Color::Rgb(40, 20, 60)),
        )
        .ratio(ratio);

    f.render_widget(gauge, area);
}

fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(NEON_PURPLE));

    let content = Line::from(vec![
        Span::styled(" [↑/↓] Navigate  ", Style::default().fg(NEON_CYAN)),
        Span::styled("[Space] Select  ", Style::default().fg(NEON_CYAN)),
        Span::styled("[a] Select All  ", Style::default().fg(NEON_CYAN)),
        Span::styled("[c] Clean Selected  ", Style::default().fg(NEON_YELLOW).add_modifier(Modifier::BOLD)),
        Span::styled("[q] Exit │ ", Style::default().fg(NEON_PINK)),
        Span::styled(&app.status_message, Style::default().fg(Color::White)),
    ]);

    let widget = Paragraph::new(content).block(block);
    f.render_widget(widget, area);
}

fn render_confirm_dialog(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" ⚠ CONFIRM PERMANENT REMOVAL ⚠ ")
        .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Red));

    let dialog_area = Rect {
        x: area.width.saturating_sub(60) / 2,
        y: area.height.saturating_sub(10) / 2,
        width: 60.min(area.width),
        height: 10.min(area.height),
    };

    let count = app.selected_appids.len();
    let bytes = app.selected_total_size();

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!(" Reclaiming {} from {} orphaned prefix(es).", format_bytes(bytes), count),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " All detected save files will be safely buried to:",
            Style::default().fg(NEON_CYAN),
        )),
        Line::from(Span::styled(
            format!(" {:?}", app.backup_dir),
            Style::default().fg(NEON_YELLOW),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Press ", Style::default().fg(Color::Gray)),
            Span::styled("[Y]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(" to confirm removal, or ", Style::default().fg(Color::Gray)),
            Span::styled("[N/Esc]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled(" to cancel.", Style::default().fg(Color::Gray)),
        ]),
    ];

    f.render_widget(Clear, dialog_area);
    let p = Paragraph::new(text).block(block).alignment(Alignment::Center);
    f.render_widget(p, dialog_area);
}
