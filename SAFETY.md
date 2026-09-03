# PrefixPug Safety Architecture & Guarantees

> **The Prime Directive:** PrefixPug's headline claim is **safety**. Any ambiguity must resolve toward *"keep the data."* Deleting a live prefix or silently missing a save is a total product failure; leaving a few gigabytes unreclaimed is not.

PrefixPug is the Steam prefix cleaner that does **not** delete your *Battle.net* prefix, does not nuke your Proton installation, and does not destroy extensionless save files. This document details the exact rules, boundaries, and safety layers implemented in the engine.

---

## 1. Multi-Library Resolution & The Unreachable Mount Guard (P0-1)

### Problem
Naive prefix cleaners scan only the default Steam library (`~/.local/share/Steam/steamapps`). When games are installed on secondary NVMe drives or external SSDs, their prefixes are flagged as "orphaned" and destroyed. Worse, if a secondary drive is temporarily disconnected, a tool that treats missing libraries as empty will wipe all prefixes belonging to that drive.

### Defense
1. PrefixPug parses `libraryfolders.vdf` across all mounted storage drives and builds a comprehensive global index of all installed `appmanifest_*.acf` manifests.
2. **The Unreachable Mount Guard:** If *any* library configured in `libraryfolders.vdf` is unmounted or unreachable on the filesystem at scan time, PrefixPug **aborts the operation immediately with an error**. It never treats an inaccessible drive as an empty library.

---

## 2. Non-Steam Shortcut Protection (P0-2)

### Problem
Non-Steam games (Battle.net, Epic Games Store, emulators, standalone launchers) added to Steam receive Wine prefixes in `steamapps/compatdata/`, but **never have an `appmanifest_*.acf` file**. Any cleaner comparing `compatdata` exclusively against `appmanifest_*.acf` will destroy every hand-configured non-Steam prefix.

### Defense
1. PrefixPug scans `userdata/<steamid3>/config/shortcuts.vdf` across **every** user profile in the Steam installation.
2. It parses Valve's binary KeyValues format and extracts both:
   - The direct unsigned 32-bit `appid`
   - The calculated compatdata directory CRC: `crc32(exe + appname) | 0x80000000`
3. Both IDs are permanently registered in the protected set with their application names (e.g. `[SHORTCUT] Battle.net`).
4. **The Fail-Safe Parse Rule:** If a `shortcuts.vdf` file exists but cannot be parsed (e.g. corruption), PrefixPug **aborts the scan**. It will never proceed with a partial or degraded view of protected shortcuts.

---

## 3. Steam Infrastructure & Runtime Deny-List (P0-3)

### Problem
Proton releases, Proton Experimental, Proton Hotfix, and Steam Linux Runtime containers create their own directories in `compatdata` and `shadercache`. Deleting them corrupts the Proton compatibility layer for all games.

### Defense
1. Steam infrastructure tools are tracked in `appmanifest` files across libraries.
2. PrefixPug maintains an internal hardcoded deny-list backstop covering known Proton versions (3.7 through 9.0, Experimental, Hotfix, Steam Linux Runtime soldier/sniper/scout, and Common Redistributables).
3. Any AppID that cannot be positively classified is quarantined into an `[UNKNOWN]` bucket that is **non-selectable and never offered for deletion**.

---

## 4. The Pug's Nose: Blocklist Save Engine (P0-4)

### Problem
Many games store saves in non-standard formats: extensionless binary blobs, `.json` state trees, `.xml` files, numbered slots, or custom `.bin` files. Cleaners using an extension allowlist (`.sav`, `.ess`) report "saves backed up" while silently leaving the actual saves behind to be deleted.

### Defense
PrefixPug **inverts the save detection logic**: it archives the **entire contents** of save roots minus a strict blocklist of known expendable junk.

**Roots Examined:**
- `%USERPROFILE%/Saved Games/*`
- `%USERPROFILE%/Documents/*` (including `My Games/*`)
- `%APPDATA%` (`AppData/Roaming/*`)
- `%LOCALAPPDATA%` (`AppData/Local/*` and `AppData/LocalLow/*`)
- `%PROGRAMDATA%` (`drive_c/ProgramData/*`)
- Legacy Wine aliases (`Application Data`, `Local Settings/Application Data`, `My Documents`)

**Blocklist (Excluded from backup):**
- Crash dumps (`*.dmp`, `*.mdmp`, `CrashDumps/`, `CrashReports/`)
- Log files (`*.log`, `logs/`, `Logs/`)
- Temporary files (`*.tmp`, `Temp/`)
- Browser/CEF caches (`CEF/`, `Cache/`, `GPUCache/`, `Code Cache/`)
- GPU shader/pipeline caches (`D3DSCache/`, `DXCache/`, `NVIDIA/`, `AMD/`, `Intel/`)
- Wine internal bundles (`Mono/`, `Gecko/`)

**Size Safety Guard:** If the save payload for a single prefix exceeds **2.00 GiB**, PrefixPug flags the prefix with an explicit warning and does not silently truncate or skip files.

---

## 5. Symlink & Path-Traversal Safety (P0-5)

### Problem
Wine prefixes frequently contain symlinks pointing outside the prefix into the user's `$HOME` (e.g. desktop integration or user documents). Walking or deleting through these symlinks can archive personal files or recursively delete directories across the host filesystem.

### Defense
1. **Never Follow Symlinks:** Directory traversal strictly uses `WalkDir::follow_links(false)`.
2. **Escaping Symlink Guard:** During save sniffing, if any directory or symlink resolves outside the prefix root (`compatdata/<appid>`), it is skipped and logged as a safety warning.
3. **Pre-Deletion Canonicalization:** Before any destructive operation, the target path is canonicalized and validated:
   - Must be a direct child of a directory named exactly `compatdata` or `shadercache`.
   - Must have a strictly numeric directory name (AppID).
   - Rejects `/`, `/home`, `$HOME`, and library roots.
4. **Non-Recursive Symlink Removal:** If a target itself is a symlink, PrefixPug deletes only the symlink file (`fs::remove_file`), never the directory it targets.

---

## 6. Process & Concurrency Guard (P0-6)

### Problem
Running a prefix cleaner while Steam is actively downloading, updating, or running a game causes race conditions, corrupted filesystems, or partial deletions of live data.

### Defense
PrefixPug inspects `/proc` for active `steam` and `steamwebhelper` processes, as well as held `pfx.lock` files. If Steam is running, destructive cleanup operations are blocked unless explicitly overridden with `--ignore-running-steam` in isolated test environments.

---

## 7. Verifiable & Restorable Save Vaults (P1-5)

1. **Cryptographic Checksums:** Every file archived by PrefixPug has its SHA-256 computed and recorded in `manifest.json`. The entire archive's SHA-256 is also recorded.
2. **Fsync Before Unlink:** Archives are explicitly flushed and fsynced (`file.sync_all()`) to physical storage, and the archive headers are verified through decompression before any prefix deletion begins.
3. **Self-Verification:** Any backup can be verified at any time:
   ```bash
   prefixpug verify-backup <BACKUP_ID>
   ```
4. **Failsafe Restoration:** Backups can be unpacked back to disk with a single command:
   ```bash
   prefixpug restore <BACKUP_ID> --target ~/RestoredSaves/
   ```

---

## 8. Known Boundaries & Limitations

PrefixPug is intentionally narrow in scope. It does **not**:
- Manage or clean prefixes managed by third-party launchers (**Heroic Games Launcher, Lutris, Bottles**). Those tools manage their own lifecycle.
- Modify or install Proton versions (use **Protontricks** or **ProtonUp-Qt** for this).
- Traverse network-mounted filesystems (NFS/SMB) with broken POSIX locking semantics.
