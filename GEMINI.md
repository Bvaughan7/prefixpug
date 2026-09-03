# PrefixPug Project Context
- We are building a CLI utility in Rust to clean up Steam/Proton `compatdata`.
- Code must be safe, using `Result` for error handling instead of `unwrap()`.
- Always prompt for user confirmation before executing any file modification or deletion commands.

## PrefixPug: Project Specification

This project is a high-performance Rust utility designed to reclaim NVMe storage by sniffing out and cleaning up orphaned Steam/Proton `compatdata` and shader caches. The tool must prioritize user safety by digging up and archiving local save files before executing any deletion commands. 

*   **Primary Language:** Rust (Edition 2021) compiled as a statically linked binary.
*   **Core Dependencies:** `ratatui` (UI rendering), `clap` (CLI argument parsing), `keyvalues-parser` (Steam VDF format parsing), and `walkdir` (fast filesystem traversal).
*   **Aesthetic Identity:** Cyberpunk/Synthwave color palette (neon pinks, bright cyans) featuring a cute terminal mascot—a neon grid-wire pug that "sniffs out" your saves. Expect smooth terminal animations and block-character UI borders.

## Core Architecture & Data Flow

The agent must implement a safe, read-first data pipeline to map the user's storage state.

*   **Steam Discovery:** The application will locate `~/.steam/root/steamapps/libraryfolders.vdf` to dynamically map all mounted storage drives.
*   **AppID Mapping:** Parse all `appmanifest_*.acf` files across detected drives to construct an accurate hashmap of currently installed games.
*   **Orphan Detection:** Traverse `compatdata/` and `shadercache/` directories, comparing the numeric folder names against the installed hashmap to flag abandoned prefixes.
*   **The Pug's Nose (Save Heuristics):** Before any removal, sniff through orphaned prefixes for standard local save locations (e.g., `pfx/drive_c/users/steamuser/Saved Games/`, `Documents/`, or `.ess`/`.sav` extensions) and bury (backup) the findings in `~/.local/share/prefixpug/backups/`.

## Directory Structure

    prefixpug/
    ├── Cargo.toml
    ├── src/
    │   ├── main.rs          (Entry point, initialization)
    │   ├── cli.rs           (Clap command definitions)
    │   ├── scanner.rs       (I/O operations, walkdir logic)
    │   ├── vdf_parser.rs    (ACF/VDF file ingestion)
    │   └── backup.rs        (File duplication and archiving)
    │   └── tui/             (Ratatui components)
    │       ├── app.rs       (State management)
    │       └── ui.rs        (Widget rendering)

## Agent Implementation Phases

Guide the agent through these strict development milestones to ensure code quality and prevent hallucination.

*   **Phase 1 (Data Layer):** Implement `vdf_parser.rs` and `scanner.rs` to successfully parse Steam libraries and print a raw list of orphaned AppIDs to the terminal.
*   **Phase 2 (CLI & Core Logic):** Build the `clap` interface supporting `--dry-run` and `--auto-clean`, alongside the `backup.rs` file archiving logic.
*   **Phase 3 (TUI Dashboard):** Construct the interactive `ratatui` dashboard featuring a three-pane layout, interactive selection checkboxes, a "sniffing" animation spinner, and a visual space-reclamation progress bar.
