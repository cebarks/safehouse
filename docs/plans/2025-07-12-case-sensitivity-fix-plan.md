# Case-Sensitivity Fix Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task.

**Goal:** Create lowercase symlinks for files/directories with uppercase ASCII characters in PZ server installs, fixing case-sensitivity breakage on Linux.

**Architecture:** A new `src/pz/case_fix.rs` module provides a single `fix_case(root)` function that walks a directory tree, cleans dangling symlinks, and creates lowercase symlinks. It's called automatically before server start and after SteamCMD installs, plus available as `safehouse mods fix-case`.

**Tech Stack:** Rust, `walkdir` crate for recursive traversal, `std::os::unix::fs::symlink` for symlink creation.

**Design Spec:** `docs/plans/2025-07-12-case-sensitivity-fix-design.md`

---

### Task 1: Add `walkdir` dependency

**Files:**

- Modify: `Cargo.toml:14` (dependencies section)

**Step 1: Add walkdir to dependencies**

Add `walkdir` to the `[dependencies]` section in `Cargo.toml`:

```toml
walkdir = "2"
```

Place it alphabetically after `tracing-subscriber`.

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles with no new errors

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "deps: add walkdir for case-sensitivity fix"
```

---

### Task 2: Write the core `fix_case` module — failing tests first

**Files:**

- Create: `src/pz/case_fix.rs`
- Modify: `src/pz/mod.rs`

**Step 1: Create the module file with public API stubs and all tests**

Create `src/pz/case_fix.rs` with the `CaseFixResult` struct, a stub `fix_case` function that returns `Ok(CaseFixResult::default())`, and the full test suite.

```rust
use std::path::Path;

use anyhow::{Context, Result};
use tracing::warn;
use walkdir::WalkDir;

/// Summary of a case-fix scan.
#[derive(Debug, Default)]
pub struct CaseFixResult {
    /// Number of lowercase symlinks created.
    pub symlinks_created: u32,
    /// Number of dangling symlinks removed.
    pub symlinks_cleaned: u32,
    /// Number of symlink operations that failed (logged individually).
    pub failures: u32,
    /// Warnings about collisions or other non-fatal issues.
    pub warnings: Vec<String>,
}

/// Scan `root` recursively, clean dangling symlinks, and create
/// lowercase symlinks for any file/directory with uppercase ASCII characters
/// in its name. This fixes Project Zomboid's case-insensitive path lookups
/// on Linux's case-sensitive filesystems.
///
/// The scan is best-effort: individual failures are logged and counted but
/// do not abort the overall operation.
pub fn fix_case(root: &Path) -> Result<CaseFixResult> {
    // Stub — will be implemented in Task 3
    let _ = root;
    Ok(CaseFixResult::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs as unix_fs;
    use tempfile::tempdir;

    #[test]
    fn test_basic_fix_creates_lowercase_symlinks() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();

        // Create Media/AnimSets/Player/
        fs::create_dir_all(root.join("Media/AnimSets/Player")).expect("mkdir");

        let result = fix_case(root).expect("fix_case");
        assert!(result.symlinks_created >= 3, "expected at least 3 symlinks, got {}", result.symlinks_created);

        // Verify symlinks exist
        assert!(root.join("media").symlink_metadata().expect("media").is_symlink());
        assert!(root.join("Media/animsets").symlink_metadata().expect("animsets").is_symlink());
        assert!(root.join("Media/AnimSets/player").symlink_metadata().expect("player").is_symlink());
    }

    #[test]
    fn test_already_lowercase_no_symlinks() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();

        fs::create_dir_all(root.join("media/scripts")).expect("mkdir");

        let result = fix_case(root).expect("fix_case");
        assert_eq!(result.symlinks_created, 0);
    }

    #[test]
    fn test_mixed_content() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();

        // Uppercase dir + lowercase dir + uppercase file
        fs::create_dir_all(root.join("Media")).expect("mkdir");
        fs::create_dir_all(root.join("scripts")).expect("mkdir");
        fs::write(root.join("Media/Icon.png"), b"img").expect("write");

        let result = fix_case(root).expect("fix_case");

        // "Media" -> symlink "media", "Icon.png" -> symlink "icon.png"
        assert!(root.join("media").symlink_metadata().expect("media").is_symlink());
        assert!(root.join("Media/icon.png").symlink_metadata().expect("icon.png").is_symlink());

        // "scripts" should have no symlink
        assert!(!root.join("Scripts").exists());
    }

    #[test]
    fn test_dangling_cleanup() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();

        // Create a dangling symlink
        unix_fs::symlink("NonExistent", root.join("broken_link")).expect("symlink");
        assert!(root.join("broken_link").symlink_metadata().expect("meta").is_symlink());

        let result = fix_case(root).expect("fix_case");
        assert_eq!(result.symlinks_cleaned, 1);
        assert!(root.join("broken_link").symlink_metadata().is_err(), "dangling symlink should be removed");
    }

    #[test]
    fn test_collision_emits_warning() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();

        // Both Textures/ and textures/ exist as real directories
        fs::create_dir_all(root.join("Textures")).expect("mkdir");
        fs::create_dir_all(root.join("textures")).expect("mkdir");

        let result = fix_case(root).expect("fix_case");
        assert!(!result.warnings.is_empty(), "expected a collision warning");
    }

    #[test]
    fn test_idempotent() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();

        fs::create_dir_all(root.join("Media/AnimSets")).expect("mkdir");

        let first = fix_case(root).expect("fix_case first");
        assert!(first.symlinks_created >= 2);

        let second = fix_case(root).expect("fix_case second");
        assert_eq!(second.symlinks_created, 0, "second run should create nothing");
        assert_eq!(second.symlinks_cleaned, 0, "second run should clean nothing");
    }

    #[test]
    fn test_chain_resolution() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();

        // Create nested uppercase path with a file
        fs::create_dir_all(root.join("Media/AnimSets/Player")).expect("mkdir");
        fs::write(root.join("Media/AnimSets/Player/file.txt"), b"hello").expect("write");

        fix_case(root).expect("fix_case");

        // The full lowercase path should resolve through the symlink chain
        let content = fs::read(root.join("media/animsets/player/file.txt"))
            .expect("reading through symlink chain should work");
        assert_eq!(content, b"hello");
    }

    #[test]
    fn test_dangling_at_target_path() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();

        // Real directory with uppercase name
        fs::create_dir_all(root.join("ANIMSETS")).expect("mkdir");
        // Dangling symlink occupying the lowercase name
        unix_fs::symlink("NonExistent", root.join("animsets")).expect("symlink");

        let result = fix_case(root).expect("fix_case");

        // Dangling should be cleaned and replaced with correct symlink
        assert!(result.symlinks_cleaned >= 1);
        assert!(result.symlinks_created >= 1);

        // Verify the new symlink resolves correctly
        let meta = root.join("animsets").symlink_metadata().expect("meta");
        assert!(meta.is_symlink());
        assert!(root.join("animsets").is_dir(), "animsets should resolve to a directory");
    }

    #[test]
    fn test_files_not_just_dirs() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();

        fs::write(root.join("Icon.png"), b"img").expect("write");

        let result = fix_case(root).expect("fix_case");
        assert!(result.symlinks_created >= 1);

        let meta = root.join("icon.png").symlink_metadata().expect("meta");
        assert!(meta.is_symlink());
    }

    #[test]
    fn test_non_ascii_passthrough() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();

        // Non-ASCII directory — the Ä should not be lowercased to ä by
        // make_ascii_lowercase (it only affects ASCII bytes)
        fs::create_dir_all(root.join("Ärzte")).expect("mkdir");

        let result = fix_case(root).expect("fix_case");

        // "Ärzte" lowercased via make_ascii_lowercase gives "ärzte" (Ä is
        // multi-byte UTF-8: 0xC3 0x84 → unchanged, since neither byte is
        // ASCII uppercase). So the name is unchanged and no symlink is needed.
        // If the name contained ASCII uppercase (e.g., "ÄBC"), a symlink
        // "äbc" would be wrong — only "Äbc" (ASCII-only lowercase) is created.
        assert_eq!(result.symlinks_created, 0, "non-ASCII-only name should produce no symlink");
    }
}
```

**Step 2: Register the module**

Add `pub mod case_fix;` to `src/pz/mod.rs`:

```rust
pub mod case_fix;
pub mod detect;
pub mod ini;
pub mod logs;
pub mod mods;
pub mod rcon;
pub mod sandbox;
```

**Step 3: Run tests to verify they fail**

Run: `cargo test --lib pz::case_fix`
Expected: all tests FAIL (stub returns default result with 0 symlinks created)

**Step 4: Commit**

```bash
git add src/pz/case_fix.rs src/pz/mod.rs
git commit -m "test: add failing tests for case-sensitivity fix"
```

---

### Task 3: Implement `fix_case`

**Files:**

- Modify: `src/pz/case_fix.rs` (replace stub with real implementation)

**Step 1: Implement the function**

Replace the stub `fix_case` body with the real implementation:

```rust
pub fn fix_case(root: &Path) -> Result<CaseFixResult> {
    anyhow::ensure!(root.is_dir(), "case-fix root does not exist: {}", root.display());

    let mut result = CaseFixResult::default();

    // Single depth-first walk. Do NOT follow symlinks to avoid cycles.
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                // Permission denied or other per-entry error — skip.
                warn!("case-fix: skipping inaccessible path: {e}");
                result.failures += 1;
                continue;
            }
        };

        let path = entry.path();

        // Phase 1: Clean dangling symlinks.
        if entry.path_is_symlink() {
            // Check if target exists. symlink_metadata succeeded (walkdir gave
            // us the entry), but the *target* may not resolve.
            if !path.exists() {
                match std::fs::remove_file(path) {
                    Ok(()) => result.symlinks_cleaned += 1,
                    Err(e) => {
                        warn!("case-fix: failed to remove dangling symlink {}: {e}", path.display());
                        result.failures += 1;
                    }
                }
            }
            // Don't create symlinks for symlinks — only real entries.
            continue;
        }

        // Phase 2: Create lowercase symlinks for entries with uppercase ASCII.
        let file_name = entry.file_name();
        let name_bytes = file_name.as_encoded_bytes();

        // Skip if no ASCII uppercase characters.
        if !name_bytes.iter().any(|b| b.is_ascii_uppercase()) {
            continue;
        }

        let mut lower_bytes = name_bytes.to_vec();
        lower_bytes.make_ascii_lowercase();

        // Safety: make_ascii_lowercase only changes ASCII bytes, preserving
        // valid UTF-8 / OsStr encoding.
        let lower_name = unsafe {
            std::ffi::OsStr::from_encoded_bytes_unchecked(&lower_bytes)
        };

        // The symlink goes in the same parent directory.
        let parent = match path.parent() {
            Some(p) => p,
            None => continue,
        };
        let symlink_path = parent.join(lower_name);

        // Use symlink_metadata (lstat) — does NOT follow symlinks.
        // Path::exists() follows symlinks, making dangling symlinks invisible
        // while they still occupy the directory entry (causing EEXIST).
        match std::fs::symlink_metadata(&symlink_path) {
            Ok(_meta) => {
                // Something already exists at the lowercase path.
                // Could be a real file/dir (collision) or a valid symlink
                // from a previous run (idempotent — skip silently).
                // Only warn on collisions (non-symlinks).
                if !_meta.is_symlink() {
                    result.warnings.push(format!(
                        "collision: both '{}' and '{}' exist in {}",
                        file_name.to_string_lossy(),
                        lower_name.to_string_lossy(),
                        parent.display(),
                    ));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Target path is free — create the symlink.
                // Use relative target: symlink points to sibling entry by name.
                if let Err(e) = unix_fs::symlink(file_name, &symlink_path) {
                    warn!(
                        "case-fix: failed to create symlink {} -> {}: {e}",
                        symlink_path.display(),
                        file_name.to_string_lossy(),
                    );
                    result.failures += 1;
                } else {
                    result.symlinks_created += 1;
                }
            }
            Err(e) => {
                warn!(
                    "case-fix: failed to check {}: {e}",
                    symlink_path.display(),
                );
                result.failures += 1;
            }
        }
    }

    Ok(result)
}
```

Add the required import at the top of the file:

```rust
use std::os::unix::fs as unix_fs;
```

**Step 2: Run tests to verify they pass**

Run: `cargo test --lib pz::case_fix`
Expected: all 10 tests PASS

**Step 3: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: no new warnings (existing known warnings in `web/handlers/mods.rs` are OK)

**Step 4: Commit**

```bash
git add src/pz/case_fix.rs
git commit -m "feat: implement case-sensitivity fix for Linux PZ servers

Walk the server install directory, clean dangling symlinks, and create
lowercase symlinks for files/dirs with uppercase ASCII characters.
Uses lstat (symlink_metadata) to handle dangling symlinks at target
paths, making the algorithm readdir-order-independent."
```

---

### Task 4: Add CLI command `safehouse mods fix-case`

**Files:**

- Modify: `src/cli/mod.rs:158-181` (add `FixCase` variant to `ModAction`)
- Modify: `src/cli/mods.rs:9-21` (add match arm + handler)

**Step 1: Add the CLI variant**

In `src/cli/mod.rs`, add to the `ModAction` enum before the closing `}`:

```rust
    /// Fix case-sensitivity issues by creating lowercase symlinks
    FixCase,
```

**Step 2: Add the handler**

In `src/cli/mods.rs`, add the match arm in the `run` function:

```rust
        ModAction::FixCase => fix_case_cmd(ctx),
```

Add the import at the top of `src/cli/mods.rs`:

```rust
use crate::pz::case_fix::fix_case;
```

Add the handler function:

```rust
fn fix_case_cmd(ctx: &CliContext) -> Result<()> {
    println!("Scanning {} for case-sensitivity issues...", ctx.config.server_install_dir.display());

    let result = fix_case(&ctx.config.server_install_dir)?;

    println!(
        "Done: {} symlinks created, {} dangling cleaned, {} failures",
        result.symlinks_created, result.symlinks_cleaned, result.failures,
    );

    for w in &result.warnings {
        println!("  ⚠ {w}");
    }

    Ok(())
}
```

**Step 3: Verify it compiles**

Run: `cargo build`
Expected: compiles cleanly

**Step 4: Verify CLI help shows the new command**

Run: `cargo run -- mods --help`
Expected: `fix-case` appears in the subcommand list

**Step 5: Commit**

```bash
git add src/cli/mod.rs src/cli/mods.rs
git commit -m "feat: add 'safehouse mods fix-case' CLI command"
```

---

### Task 5: Integrate into server start and SteamCMD install

**Files:**

- Modify: `src/container.rs` (hook into `create_and_start` and `run_steamcmd_install`)

**Step 1: Add the import**

Add to the imports at the top of `src/container.rs`:

```rust
use crate::pz::case_fix::fix_case;
```

**Step 2: Hook into `create_and_start`**

In `create_and_start`, add the case-fix call immediately before the `// Clean up any leftover stopped container` comment (line 54). This ensures it runs before `create_container` triggers `:Z` SELinux relabeling.

```rust
    // Fix case-sensitivity issues: create lowercase symlinks so PZ's
    // lowercased path lookups resolve on Linux. Must run BEFORE
    // create_container because the :Z bind mount triggers SELinux
    // relabeling — symlinks created afterward would lack container_file_t.
    match fix_case(&config.server_install_dir) {
        Ok(result) => {
            if result.symlinks_created > 0 || result.symlinks_cleaned > 0 {
                tracing::info!(
                    created = result.symlinks_created,
                    cleaned = result.symlinks_cleaned,
                    failures = result.failures,
                    "case-fix scan complete",
                );
            }
            for w in &result.warnings {
                tracing::warn!("case-fix: {w}");
            }
        }
        Err(e) => tracing::warn!("case-fix scan failed, continuing: {e}"),
    }
```

**Step 3: Hook into `run_steamcmd_install`**

In `run_steamcmd_install`, add the case-fix call after the `println!("PZ server installed.");` line (near the end, before `Ok(())`):

```rust
    // Fix case-sensitivity issues on freshly downloaded files.
    match fix_case(&config.server_install_dir) {
        Ok(result) if result.symlinks_created > 0 => {
            println!(
                "Case-fix: created {} lowercase symlinks, cleaned {} dangling",
                result.symlinks_created, result.symlinks_cleaned,
            );
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("case-fix scan failed after install: {e}"),
    }
```

**Step 4: Verify it compiles**

Run: `cargo build`
Expected: compiles cleanly

**Step 5: Run full test suite**

Run: `cargo test`
Expected: all tests pass (existing 106 + 10 new case_fix tests)

**Step 6: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: no new warnings

**Step 7: Commit**

```bash
git add src/container.rs
git commit -m "feat: auto-run case-fix on server start and after SteamCMD install

Runs fix_case before container creation (required for SELinux :Z
relabeling order) and after SteamCMD downloads fresh files.
Both hooks are non-fatal — failures are logged and the server
continues starting."
```

---

### Task 6: Update documentation

**Files:**

- Modify: `docs/architecture.md` (add case_fix to module map and design decisions)

**Step 1: Add to module map**

In `docs/architecture.md`, add `case_fix.rs` to the `pz/` section of the module map:

```
├── pz/
│   ├── mod.rs           # PZ module declarations
│   ├── case_fix.rs      # Lowercase symlink fixer for case-sensitive Linux filesystems
│   ├── detect.rs        # Binary detection, PID utilities
```

**Step 2: Add design decision**

Add a new subsection under `## Key Design Decisions`:

```markdown
### Case-Sensitivity Fix

PZ lowercases file paths internally (Windows heritage). On Linux's case-sensitive filesystems, this breaks mod loading when files have uppercase names. `case_fix.rs` walks the server install directory and creates relative lowercase symlinks alongside any entry with uppercase ASCII characters — e.g., `animsets -> AnimSets`. The fix runs automatically before container creation and after SteamCMD installs, plus on-demand via `safehouse mods fix-case`. Uses `symlink_metadata` (lstat) instead of `Path::exists()` to correctly handle dangling symlinks at target paths.
```

**Step 3: Commit**

```bash
git add docs/architecture.md
git commit -m "docs: add case-sensitivity fix to architecture docs"
```
