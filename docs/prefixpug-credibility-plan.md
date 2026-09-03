# PrefixPug — Credibility & Scope Plan

**Companion to:** `prefixpug-hardening-tasks.md` (the technical fix list)
**This doc answers two objections you will get on day one:**

> "This is AI slop."
> "This should just be a Decky plugin / a shell script."

Both are fair right now. Neither has to stay true. This is what closes them.

---

## Part 1 — Why it currently reads as slop, and what to change

The Linux gaming crowd has developed a fast pattern-match for LLM-generated repos. The tells are not about whether AI was used; they're about whether the thing has ever been *run*. Every item below is a tell the README currently trips.

### 1.1 Assets must be real captures, not staged mockups

This is the single biggest one. The demo screenshot lists `AppID 1091500` labelled **CyberPug**. That app ID is Cyberpunk 2077. Anyone who checks (and someone will) learns the demo was constructed rather than captured, and every other claim in the README becomes suspect by association.

**Do:**
- Record the demo with `asciinema` on a real machine with real orphaned prefixes, and embed the cast or a GIF generated from it. Real app IDs, real names, real sizes, real timings.
- If you want to show the purge confirmation, actually purge something disposable and show it.
- Delete the mascot ASCII art panel from the TUI, or make it a togglable easter egg that is off by default. It occupies prime screen real estate that should show data.
- Keep the hero banner if you like it. One decorative image is fine. Decorative *data* is not.

### 1.2 Claims must be falsifiable and true

- Cut "high-performance." Directory traversal is I/O-bound; the phrase invites someone to benchmark you against `du` and post the result.
- Cut "Zero Unsafe Deletions" until the P0 fixes land and there are tests proving it. Right now the tool can delete a live non-Steam prefix, which makes that line actively false.
- Replace superlatives with numbers you measured: "reclaimed 11.3 GiB across 9 orphaned prefixes on my machine (CachyOS, btrfs+zstd)." Specific and checkable beats impressive and vague.
- State the honest ceiling early: most people reclaim single-digit to low-double-digit GiB. Under-promising here buys you enormous credibility.

### 1.3 Structure and tone

- Emoji-per-heading, bold-per-sentence, and a marketing structure ("Overview / Aesthetics / Safety / Installation / Usage / Testing") on a pre-1.0 project reads as a template. Compress the README to: what it does, the dangerous cases it handles correctly, install, usage, safety model, license. Move the rest to `docs/`.
- Write a `SAFETY.md` that explains the orphan-detection logic, the save-vaulting logic, and the known limitations in plain prose. This is the document that converts a skeptic. Nobody else in this niche has one.
- Known limitations sections are credibility multipliers. List what it does *not* handle (Flatpak Steam if untested, Heroic prefixes, Lutris, Bottles, network shares).

### 1.4 Disclose AI assistance, briefly and without apology

A one-line note in the README ("Developed with AI assistance; all logic is tested against real Steam installs — see `tests/`") costs nothing and defuses the accusation before it's made. Being caught denying it is far worse than saying it. The tests are what make the statement land, so this only works after the test suite from the hardening doc exists.

### 1.5 Fix the badges before anyone sees the page

`CI: repo or workflow not found` sitting under a hero banner is the visual equivalent of a broken window. Green CI, one tagged release, then re-add badges.

---

## Part 2 — Answering "this should just be a plugin"

The objection has a real point buried in it: **the people with the most acute need are Steam Deck users**, they don't open terminals, and they'd be served by a Decky Loader plugin. If your answer is "but I wanted to write a Rust TUI," you lose.

The correct answer is architectural.

### 2.1 Split the crate now: `prefixpug-core` + frontends

```
prefixpug-core/     # library: discovery, classification, vaulting, deletion
prefixpug-cli/      # clap + ratatui frontend
prefixpug-daemon/   # (later) thin JSON-RPC/HTTP server for a Decky plugin
```

`prefixpug-core` exposes a stable, side-effect-free scan API returning structured results, and a separate explicitly-destructive API. No `println!`, no TUI types, no `std::process::exit` anywhere in core.

This buys you three things:
1. The Decky plugin becomes a frontend project, not a rewrite. You can say "the plugin is on the roadmap and the core already supports it" and mean it.
2. Other projects can depend on your Steam-layout logic. That's the actual reusable asset.
3. It forces the testability the safety claims require.

### 2.2 The real moat is being correct about Steam's on-disk layout

Anyone can write a shell script that deletes `compatdata` dirs with no matching manifest. What nobody has packaged carefully is the full set of cases where that naive rule destroys data:

- Multiple library roots, including unmounted ones
- Non-Steam shortcuts across multiple `userdata` profiles
- Flatpak Steam paths
- Steam Deck internal storage plus SD card
- Proton and Steam Linux Runtime tool prefixes
- Prefixes with protontricks-installed dependencies and mod loaders
- Games mid-download, mid-update, or with Steam actively running

**That list is your pitch.** "PrefixPug is the tool that doesn't delete your Battle.net prefix" is a far stronger sentence than anything about synthwave. Lead `SAFETY.md` with it.

### 2.3 Roadmap that makes the plugin question moot

- **v0.1** — CLI/TUI, all P0 fixes, shader-cache-only mode, tests, AUR package
- **v0.2** — `prefixpug-core` split, `--json` stable, verified restore
- **v0.3** — Decky Loader plugin using core via the daemon; Deck-specific paths (SD card, internal) explicitly tested
- Not on the roadmap: prefix editing, Proton version management, launcher features. Protontricks and Steam Tinker Launch own those. Staying narrow is what makes you trustworthy near `rm -rf`.

---

## Part 3 — Making it an actually useful tool

Reclamation alone is a run-it-twice-a-year utility. These extend usefulness without expanding blast radius.

### 3.1 Read-only audit mode (highest value per unit of risk)

`prefixpug audit` — no deletion capability compiled into the path at all. Shows every prefix with: app ID, resolved name, source of that name, on-disk size, last-touched date, live/orphan/shortcut/tool/unknown classification, whether saves were detected, whether the prefix shows signs of manual configuration.

Many people will use only this, and it's the mode you can recommend to strangers with a clear conscience. It's also what turns the tool from a deleter into an inventory tool, which is a much better long-term identity.

### 3.2 Save vaulting as a standalone feature

`prefixpug vault <appid>` — archive saves from a prefix without deleting anything. Useful before a Proton version change, before reinstalling, before switching distros, or for backing up games that don't use Steam Cloud. This is arguably more broadly useful than the deletion feature and shares all the same code.

Pair it with a check of whether the app has Steam Cloud enabled, so the tool can tell the user "this one's synced, local copy is disposable" versus "this is the only copy."

### 3.3 Staleness over orphanhood

Surface "installed but untouched for 14 months, 6.2 GiB" alongside orphans. That is actionable information the user cannot easily get any other way, and it doesn't require the tool to delete anything.

### 3.4 Machine-readable everything

Stable `--json` for `audit` and `scan` means people can wire it into their own scripts, status bars, and cron. Tools that compose get adopted; tools that only run interactively get forgotten.

---

## Part 4 — Distribution and signals of seriousness

- **Packaging:** publish to crates.io, submit a PKGBUILD to the AUR. Do **not** ship a `curl | bash` installer; that alone will get the project dismissed. If `install.sh` stays, make it a thin convenience wrapper that prints exactly what it will do.
- **XDG compliance:** config in `$XDG_CONFIG_HOME/prefixpug/`, data in `$XDG_DATA_HOME/prefixpug/`, cache in `$XDG_CACHE_HOME/`. Honor the env vars rather than hardcoding `~/.local/share`.
- **Basics that signal a real project:** `--version`, `--help` that's actually useful, a man page, shell completions (bash/zsh/fish), semantic versioning, a `CHANGELOG.md`, `LICENSE` file present and matching the badge.
- **Releases:** tagged, with prebuilt binaries and checksums. Reproducible if you can manage it.
- **Repo hygiene:** issue templates, a `CONTRIBUTING.md`, and responsive replies. A maintainer who answers issues within a day is the strongest anti-slop signal that exists, because slop repos are abandoned at launch.

---

## Part 5 — Launch sequence

Do not post it anywhere until P0 is done and CI is green. A skeptical first thread is very hard to recover from, and this is a small community with a long memory.

1. Land P0 fixes + tests + green CI + one tagged release
2. Write `SAFETY.md`; rewrite `README.md` around it
3. Record a real asciinema demo on your own machine
4. Ship the AUR package
5. Run it yourself for a month. Fix what breaks. Accumulate real numbers.
6. *Then* post to r/linux_gaming — framed as "I kept almost deleting my non-Steam prefixes by hand, so I wrote something that understands Steam's layout properly," not as a product launch. Lead with the safety model, link `SAFETY.md`, state the honest reclaim ceiling, and ask for testing on setups you don't have (Flatpak, Deck, multi-drive).

**Realistic outcome:** a few dozen stars, a handful of genuine users, an AUR package with real installs, and a couple of bug reports from setups you never anticipated. That is what success looks like for a utility this narrow, and it's worth far more than a viral thread. Same credibility target you set for Goblin Mode Pro: respected by Linux power users, which is earned through correctness and responsiveness rather than presentation.

---

## Anti-goals

- Do not add features to make it feel bigger. Narrow scope near destructive operations is the product.
- Do not add more branding, mascots, or ASCII art. There is already more than the substance supports.
- Do not generate more README prose. Every remaining word should be something you can defend under questioning.
- Do not claim safety properties that are not covered by a test.
