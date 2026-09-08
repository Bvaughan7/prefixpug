# PrefixPug Repositioning & Presentation Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reposition PrefixPug from an over-marketed startup-style pitch into a direct, outcome-first, developer-respected open-source utility for Linux and Steam Deck gamers.

**Architecture:** Replace branding buzzwords ("The Pug's Nose", "Prime Directive", "P0/P1 classifications", "CYBERPUG DISK SENTINEL", "ELI5") across documentation, CLI output, and web presentation with clean, plain-spoken technical documentation, practical outcomes (20–100GB space recovery), and verifiable automated test evidence.

**Tech Stack:** Rust (Edition 2021), `ratatui`, `clap`, Markdown, HTML/CSS, React/TypeScript (`@decky/ui`).

**Spec:** Repository Feedback Report (September 2026) advising a 30–50% reduction in marketing language, outcome-first README structure, and concrete evidence-based safety documentation.

## Global Constraints

- Code must remain 100% safe, using `Result` for error handling with zero `.unwrap()` in production paths.
- Preserve all existing safety features (mount guard, shortcuts parser, save backup vault, process lock detection, symlink jail).
- Do not break backward compatibility for CLI arguments or JSON output schemas.
- Maintain test coverage across all unit and integration tests (`cargo test --all-targets`).

---

### Task 1: De-Market CLI Output & Engine Terminology

**Files:**
- Modify: `src/main.rs:60-65, 508-518`
- Modify: `src/scanner.rs:260-265`
- Modify: `decky-plugin/src/index.tsx:104-122`
- Test: `tests/integration_tests.rs:430-480`

**Interfaces:**
- CLI log messages: Change `[Pug Vault] Buried X save files` to `[Backup] Archived X save file(s) -> <PATH>`
- CLI vault command output: Change `The Pug's Nose snuffed out X save file(s)` to `Discovered X save file(s) (<SIZE>) in prefix <APPID>:`
- Decky UI: Change `⚡ CYBERPUG DISK SENTINEL` to `PrefixPug Storage Cleaner` and remove "The Pug's Nose" references.

- [ ] **Step 1: Update terminal output in `src/main.rs`**

In `execute_clean`:
```rust
        if let Some(archive_dir) = backup::backup_orphan_saves(orphan, backup_root)? {
            println!(
                "  [Backup] Archived {} save file(s) -> {:?}",
                orphan.detected_saves.len(),
                archive_dir
            );
        }
```

In `run_vault_command`:
```rust
    println!(
        "Discovered {} save file(s) ({}) in prefix {}:",
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
```

- [ ] **Step 2: Update code comments in `src/scanner.rs`**

Change doc comments from `"The Pug's Nose: Sniffs through save roots..."` to `"Save detection: Scans standard Windows/Wine save locations using a blocklist..."`.

- [ ] **Step 3: Update Decky plugin UI in `decky-plugin/src/index.tsx`**

Replace:
```tsx
<div style={{ color: "#ff007f", fontWeight: "bold" }}>⚡ CYBERPUG DISK SENTINEL</div>
```
With:
```tsx
<div style={{ color: "#ff007f", fontWeight: "bold" }}>PREFIXPUG STORAGE CLEANER</div>
```

Replace:
```tsx
<p style={{ color: "#00ffff", fontSize: "12px", marginTop: "8px" }}>
  🛡 The Pug's Nose will automatically vault all local save files to ~/.local/share/prefixpug/backups/ before deletion!
</p>
```
With:
```tsx
<p style={{ color: "#00ffff", fontSize: "12px", marginTop: "8px" }}>
  🛡 Save files are automatically backed up to ~/.local/share/prefixpug/backups/ before deletion.
</p>
```

- [ ] **Step 4: Run cargo test to verify no tests broken**

Run: `cargo test --all-targets`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/scanner.rs decky-plugin/src/index.tsx
git commit -m "refactor(cli): remove mascot branding from terminal logs and UI text"
```

---

### Task 2: Rewrite `README.md` to be Outcome-First & Developer-Credible

**Files:**
- Modify: `README.md`

**Objectives:**
- Cut marketing/SaaS buzzwords by 40%.
- Lead with outcomes: "Reclaim 20–100GB of abandoned Proton prefixes and shader caches without risking your saves."
- Put the problem and outcome right at the top without corporate/startup tropes.
- Replace "ELI5" heading with clear, natural language: "The Problem: Why Storage Disappears on Linux & Steam Deck".
- Remove "The Prime Directive" and "P0/P1" labels; describe the safeguards as direct technical mechanisms with references to `tests/integration_tests.rs`.
- Highlight Steam Deck storage recovery as a primary practical use case.
- Show concrete verification and restore commands early.

- [ ] **Step 1: Draft the restructured README**
  - Banner & subtitle: "Safe Steam/Proton compatdata and shader cache cleaner for Linux and Steam Deck."
  - "The Problem": Direct 3-paragraph explanation of Proton prefixes, uninstalled game leftovers, and why `rm -rf` deletes save files.
  - "Quick Start": Immediate 3-step usage guide (`prefixpug`, browse, press `c`).
  - "How PrefixPug Protects Your Data": Replace "P0/P1" jargon with 5 concrete safety mechanisms (Mount checking, Non-Steam shortcut discovery, Pre-deletion save backup, Concurrency locks, Symlink boundary enforcement).
  - Include explicit mention of automated integration tests (`tests/integration_tests.rs`) to prove claims.
  - CLI usage examples and restore walkthrough (`prefixpug restore <ID> --target ~/RestoredSaves/`).
  - Steam Deck section with Decky Loader integration.
  - Installation and sandbox instructions.

- [ ] **Step 2: Replace contents of `README.md`**
- [ ] **Step 3: Review markdown formatting and link integrity**
- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: overhaul README with outcome-first structure and grounded technical tone"
```

---

### Task 3: Ground `SAFETY.md` & `man/prefixpug.1` as Engineering Specs

**Files:**
- Modify: `SAFETY.md`
- Modify: `man/prefixpug.1`

**Objectives:**
- Strip "The Prime Directive", "The Pug's Nose", and internal project sprint notation (P0-1, P0-2, P1-5) from `SAFETY.md`.
- Present `SAFETY.md` as an authoritative threat model and safety architecture document.
- Detail failure scenarios, technical root causes, exact code defense, and the test verifying each invariant.
- Update `man/prefixpug.1` to remove any lingering mascot/marketing phrasing.

- [ ] **Step 1: Rewrite `SAFETY.md` into structured threat model format**
  - Section 1: Overview & Design Goal (Safely reclaim disk space with zero data loss tolerance).
  - Section 2: Threat Model & Defenses:
    1. Unmounted Storage Disconnection (Secondary NVMe / MicroSD).
    2. Custom Non-Steam Games & Launchers (`shortcuts.vdf` CRC resolution).
    3. Steam Runtime & Proton Compatibility Corruption (Permanent infrastructure lock).
    4. Save File Deletion in Non-Cloud Games (Blocklist save engine).
    5. Host Directory Deletion via Wine Symlinks (`WalkDir::follow_links(false)` + canonical jail).
    6. Concurrency & Write Races (`/proc` check + non-blocking `pfx.lock` flock).
    7. Archive Corruption & Atomic Unlinking (`flate2` + SHA-256 manifest + `fsync`).
  - Section 3: Automated Verification Matrix (table linking each threat to tests in `tests/integration_tests.rs`).
  - Section 4: Non-Goals & Scope Limits.

- [ ] **Step 2: Update `man/prefixpug.1`**
  - Replace "The Pug's Nose (Save Vaulting)" with "Save File Archiving (Pre-Deletion Vault)".

- [ ] **Step 3: Commit**

```bash
git add SAFETY.md man/prefixpug.1
git commit -m "docs: transform SAFETY.md and manpage into engineering threat models"
```

---

### Task 4: Align Website (`web/index.html`) with Utility Tool Identity

**Files:**
- Modify: `web/index.html`

**Objectives:**
- Eliminate SaaS/startup tone from the GitHub Pages site.
- Change hero copy to focus on real outcome: "Reclaim 20–100GB of Abandoned Proton Prefixes on Linux & Steam Deck."
- Remove "Prime Directive" and "The Pug's Nose" headings.
- Present the 4 key problems and solutions cleanly:
  1. Virtual Windows Drives (`compatdata`)
  2. The Leftover Storage Leak
  3. Why Simple Scripts Delete Saves
  4. How PrefixPug Safeguards Data
- Ensure terminal showcase and copy-paste install remain clear and responsive.

- [ ] **Step 1: Update hero section and copy in `web/index.html`**
- [ ] **Step 2: Update safety and feature cards to match grounded terminology**
- [ ] **Step 3: Commit**

```bash
git add web/index.html
git commit -m "docs(pages): recalibrate website presentation to match open-source utility identity"
```

---

### Task 5: Verify Entire Repository & Update Changelog

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Run comprehensive test suite**
  Run: `cargo test --all-targets --all-features`
  Run: `cargo clippy --all-targets --all-features -- -D warnings`
  Run: `cargo fmt --all -- --check`

- [ ] **Step 2: Test sandbox workflow**
  Run: `./tests/test_sandbox.sh`
  Run: `cargo run -- --library-vdf /tmp/prefixpug_mock_steam/steamapps/libraryfolders.vdf scan`

- [ ] **Step 3: Update `CHANGELOG.md`**
  Record presentation recalibration, removal of over-marketing, and engineering documentation improvements.

- [ ] **Step 4: Commit and push**
  Commit all finalized changes and push to `origin/main`.
