# Contributing to PrefixPug 🐾

Thank you for your interest in making **PrefixPug** better and safer! PrefixPug is built to safeguard user save data while recovering gigabytes of storage from orphaned Steam/Proton compatdata.

---

## 🛡️ The Prime Directive

> **User Safety Above All.**  
> If there is *any* ambiguity about whether a prefix is active, whether a file is a save, or whether an external drive is attached, PrefixPug **must resolve toward keeping the data**.

Before submitting any code changes, read [`SAFETY.md`](SAFETY.md) carefully to understand our security model and safety guarantees.

---

## 📋 Strict Coding Standards

1. **Zero `.unwrap()` in Production Code:**  
   Every error must be represented as a `Result` and handled with clear context via `anyhow` or pattern matching. Unwrapping in `src/` is strictly prohibited and will fail CI checks.
2. **Mandatory Confirmation on Mutations:**  
   Destructive actions (such as unlinking directories) must never occur silently. Interactive sessions must prompt for confirmation, and non-interactive runs must require explicit `--yes` or `--purge` flags.
3. **Strict Path Traversal Defenses:**  
   Never delete paths directly from user input or relative paths. All deletion candidates must pass `scanner::validate_prefix_path_for_deletion()`, proving they are strictly children of `compatdata` or `shadercache`, have numeric AppID names, and are not system directories or root.
4. **Symlink Isolation:**  
   Never follow symlinks during recursive directory scans or deletions (`WalkDir::follow_links(false)`). Escaping symlinks must be logged as warnings and skipped.
5. **Atomic, fsynced Save Vaults:**  
   Before unlinking any prefix, all discovered save files must be archived to a compressed tarball, accompanied by an uncompressed `manifest.json` containing per-file and archive SHA-256 hashes, and flushed to disk via `File::sync_all()`.

---

## 🛠️ Development Workflow

### Prerequisites
- Rust stable (2021 edition)
- Cargo

### Building and Testing
Always run the complete test and lint suite prior to pushing or opening a pull request:

```bash
# 1. Format check
cargo fmt --all -- --check

# 2. Strict clippy lints
cargo clippy --all-targets --all-features -- -D warnings

# 3. Unit and integration test suite
cargo test --all-targets --all-features
```

### Writing Tests
When adding new features or bug fixes, always write corresponding synthetic unit or integration tests in `tests/integration_tests.rs`. Use the `SyntheticSteamFixture` helper struct to create mock multi-library environments without touching the real Steam directory on your machine.

---

## 🚀 Submitting a Pull Request

1. Fork the repository and create a feature branch (`git checkout -b feature/my-feature`).
2. Implement your changes, adhering to the safety standards above.
3. Add tests to cover your changes.
4. Verify all tests and lints pass (`cargo test --all-targets`).
5. Update `CHANGELOG.md` under the `[Unreleased]` or current milestone section.
6. Submit your pull request with a detailed description of the changes and safety implications.
