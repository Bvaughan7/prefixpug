use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

use prefixpug::scanner::{OrphanedPrefix, SaveFileInfo};
use prefixpug::tui::app::{App, AppState};

#[derive(Serialize)]
struct RenderCell {
    x: u16,
    y: u16,
    ch: String,
    fg: (u8, u8, u8),
    bg: (u8, u8, u8),
    bold: bool,
}

#[derive(Serialize)]
struct RenderFrame {
    width: u16,
    height: u16,
    cells: Vec<RenderCell>,
}

fn color_to_rgb(c: ratatui::style::Color) -> (u8, u8, u8) {
    match c {
        ratatui::style::Color::Rgb(r, g, b) => (r, g, b),
        ratatui::style::Color::Black => (15, 12, 28),
        ratatui::style::Color::Red => (255, 69, 58),
        ratatui::style::Color::Green => (50, 205, 50),
        ratatui::style::Color::Yellow => (255, 215, 0),
        ratatui::style::Color::Blue => (10, 132, 255),
        ratatui::style::Color::Magenta => (255, 20, 147),
        ratatui::style::Color::Cyan => (0, 245, 255),
        ratatui::style::Color::Gray => (142, 142, 147),
        ratatui::style::Color::DarkGray => (99, 99, 102),
        ratatui::style::Color::LightRed => (255, 105, 97),
        ratatui::style::Color::LightGreen => (48, 209, 88),
        ratatui::style::Color::LightYellow => (255, 214, 10),
        ratatui::style::Color::LightBlue => (100, 210, 255),
        ratatui::style::Color::LightMagenta => (255, 55, 95),
        ratatui::style::Color::LightCyan => (102, 212, 207),
        ratatui::style::Color::White => (255, 255, 255),
        ratatui::style::Color::Reset => (220, 220, 235),
        _ => (200, 200, 200),
    }
}

fn capture_frame(terminal: &mut Terminal<TestBackend>, app: &App) -> RenderFrame {
    terminal.draw(|f| prefixpug::tui::ui::render(f, app)).expect("draw");
    let buf = terminal.backend().buffer();
    let area = buf.area;

    let mut cells = Vec::new();
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = buf.cell((x, y)).expect("cell");
            let fg = color_to_rgb(cell.fg);
            let bg = match cell.bg {
                ratatui::style::Color::Reset => (15, 12, 28),
                c => color_to_rgb(c),
            };
            cells.push(RenderCell {
                x,
                y,
                ch: cell.symbol().to_string(),
                fg,
                bg,
                bold: cell.modifier.contains(ratatui::style::Modifier::BOLD),
            });
        }
    }

    RenderFrame {
        width: area.width,
        height: area.height,
        cells,
    }
}

fn main() {
    let out_dir = PathBuf::from("/tmp/pug_render_frames");
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).expect("create out_dir");

    let width = 106;
    let height = 30;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");

    let mock_orphans = vec![
        OrphanedPrefix {
            appid: "489830".to_string(),
            title: Some("Skyrim SE".to_string()),
            library_path: PathBuf::from("/games/nvme0n1/Steam"),
            compatdata_path: Some(PathBuf::from("/games/nvme0n1/Steam/steamapps/compatdata/489830")),
            compatdata_size: 1_288_490_188,
            shadercache_path: Some(PathBuf::from("/games/nvme0n1/Steam/steamapps/shadercache/489830")),
            shadercache_size: 125_829_120,
            detected_saves: vec![
                SaveFileInfo {
                    path: PathBuf::from("pfx/drive_c/users/steamuser/Saved Games/Skyrim/Save1.ess"),
                    size_bytes: 14_680_064,
                },
                SaveFileInfo {
                    path: PathBuf::from("pfx/drive_c/users/steamuser/Documents/My Games/Skyrim/quicksave.sav"),
                    size_bytes: 8_388_608,
                },
            ],
        },
        OrphanedPrefix {
            appid: "292030".to_string(),
            title: Some("Witcher 3".to_string()),
            library_path: PathBuf::from("/games/nvme0n1/Steam"),
            compatdata_path: Some(PathBuf::from("/games/nvme0n1/Steam/steamapps/compatdata/292030")),
            compatdata_size: 2_952_790_016,
            shadercache_path: Some(PathBuf::from("/games/nvme0n1/Steam/steamapps/shadercache/292030")),
            shadercache_size: 250_000_000,
            detected_saves: vec![
                SaveFileInfo {
                    path: PathBuf::from("pfx/drive_c/users/steamuser/Documents/The Witcher 3/gamesaves/ManualSave_1.sav"),
                    size_bytes: 25_165_824,
                },
            ],
        },
        OrphanedPrefix {
            appid: "1091500".to_string(),
            title: Some("CyberPug".to_string()),
            library_path: PathBuf::from("/games/nvme0n1/Steam"),
            compatdata_path: Some(PathBuf::from("/games/nvme0n1/Steam/steamapps/compatdata/1091500")),
            compatdata_size: 450_000_000,
            shadercache_path: Some(PathBuf::from("/games/nvme0n1/Steam/steamapps/shadercache/1091500")),
            shadercache_size: 75_000_000,
            detected_saves: vec![
                SaveFileInfo {
                    path: PathBuf::from("pfx/drive_c/users/steamuser/AppData/Local/CyberPug/profile.dat"),
                    size_bytes: 4_194_304,
                },
            ],
        },
        OrphanedPrefix {
            appid: "22380".to_string(),
            title: Some("Fallout: NV".to_string()),
            library_path: PathBuf::from("/games/nvme0n1/Steam"),
            compatdata_path: Some(PathBuf::from("/games/nvme0n1/Steam/steamapps/compatdata/22380")),
            compatdata_size: 890_000_000,
            shadercache_path: Some(PathBuf::from("/games/nvme0n1/Steam/steamapps/shadercache/22380")),
            shadercache_size: 45_000_000,
            detected_saves: vec![
                SaveFileInfo {
                    path: PathBuf::from("pfx/drive_c/users/steamuser/Documents/My Games/FalloutNV/Saves/Save1.fos"),
                    size_bytes: 9_437_184,
                },
            ],
        },
        OrphanedPrefix {
            appid: "1245620".to_string(),
            title: Some("Elden Ring".to_string()),
            library_path: PathBuf::from("/games/nvme0n1/Steam"),
            compatdata_path: Some(PathBuf::from("/games/nvme0n1/Steam/steamapps/compatdata/1245620")),
            compatdata_size: 1_850_000_000,
            shadercache_path: Some(PathBuf::from("/games/nvme0n1/Steam/steamapps/shadercache/1245620")),
            shadercache_size: 420_000_000,
            detected_saves: vec![
                SaveFileInfo {
                    path: PathBuf::from("pfx/drive_c/users/steamuser/AppData/Roaming/EldenRing/ER0000.sl2"),
                    size_bytes: 33_554_432,
                },
            ],
        },
    ];

    let mut app = App::new(mock_orphans, PathBuf::from("/home/papab/.local/share/prefixpug/backups"));

    let mut frame_id = 0;

    // Phase 1: Sniffing animation loop (Pug sniffing nose & particles)
    for _ in 0..12 {
        let frame = capture_frame(&mut terminal, &app);
        let path = out_dir.join(format!("frame_{:03}.json", frame_id));
        let json = serde_json::to_string(&frame).unwrap();
        fs::write(path, json).unwrap();
        frame_id += 1;
        app.tick();
    }

    // Phase 2: User navigating down through list
    for _ in 0..4 {
        app.next_item();
        for _ in 0..2 {
            let frame = capture_frame(&mut terminal, &app);
            let path = out_dir.join(format!("frame_{:03}.json", frame_id));
            let json = serde_json::to_string(&frame).unwrap();
            fs::write(path, json).unwrap();
            frame_id += 1;
            app.tick();
        }
    }

    // Phase 3: Toggle selection
    app.toggle_selection();
    for _ in 0..3 {
        let frame = capture_frame(&mut terminal, &app);
        let path = out_dir.join(format!("frame_{:03}.json", frame_id));
        let json = serde_json::to_string(&frame).unwrap();
        fs::write(path, json).unwrap();
        frame_id += 1;
        app.tick();
    }

    // Phase 4: Open confirmation modal
    app.state = AppState::ConfirmingDeletion;
    for _ in 0..8 {
        let frame = capture_frame(&mut terminal, &app);
        let path = out_dir.join(format!("frame_{:03}.json", frame_id));
        let json = serde_json::to_string(&frame).unwrap();
        fs::write(path, json).unwrap();
        frame_id += 1;
        app.tick();
    }

    // Phase 5: Help screen modal
    app.state = AppState::ShowingHelp;
    for _ in 0..6 {
        let frame = capture_frame(&mut terminal, &app);
        let path = out_dir.join(format!("frame_{:03}.json", frame_id));
        let json = serde_json::to_string(&frame).unwrap();
        fs::write(path, json).unwrap();
        frame_id += 1;
        app.tick();
    }

    println!("✓ Rendered {} frames into {:?}", frame_id, out_dir);
}
