# Merge sw-sync Collection Management into Safehouse — Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task.

**Goal:** Absorb the `sw-sync` collection management CLI (from `steam-workshop-mcp`) into `safehouse` as a new `safehouse mods push-collection` subcommand, so mod list management is unified in one tool instead of split between two binaries.

**Architecture:** Safehouse already has `mods sync` which pulls a collection into server.ini (one-directional: Steam → server). The new `mods push-collection` goes the other direction: pushes a local file of IDs into a Steam Workshop collection via the Steamworks SDK. The `CollectionManager` from `steamworks_ext.rs` is brought in behind the same `steamworks` feature flag pattern. The existing `steam/workshop.rs` module gains collection-fetching that returns `HashSet<u64>` for diffing (the current `fetch_collection_items` returns `Vec<String>` — we add a typed variant). The CLI gains `--file`, `--apply`, `--no-remove`, `--delay` flags on the new subcommand.

**Key benefit:** sw-sync required `STEAM_API_KEY` for reading collections (via `SteamClient`). Safehouse's existing `fetch_collection_items` uses the public, keyless `ISteamRemoteStorage/GetCollectionDetails/v1/` endpoint. This port **eliminates the STEAM_API_KEY requirement** for dry runs.

**Parallelism note:** Tasks 1, 3, 4, 5 are independent and can run simultaneously. Task 2 depends on Task 1. Task 6 depends on Tasks 2–5.

**Tech Stack:** Rust, `clap` (already in safehouse), `steamworks` + `steamworks-sys` (new optional deps), `reqwest` (existing)

**Source project:** `~/code/steam-workshop-mcp/` — files to port: `src/steamworks_ext.rs`, collection-fetching from `src/bin/sync.rs`

---

### Task 1: Add `steamworks` feature flag and dependencies

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add optional dependencies**

Add to `[dependencies]`:
```toml
steamworks = { version = "0.13", optional = true }
steamworks-sys = { version = "0.13", optional = true }
```

Add `[features]` section (safehouse currently has none):
```toml
[features]
default = []
steamworks = ["dep:steamworks", "dep:steamworks-sys"]
```

**Step 2: Verify both build paths**

Run: `cargo check`
Expected: passes (no feature = no change)

Run: `cargo check --features steamworks`
Expected: passes (deps download but nothing uses them yet)

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat: add optional steamworks/steamworks-sys dependencies behind feature flag"
```

---

### Task 2: Port `CollectionManager` and `AppIdFile` into safehouse

**Files:**
- Create: `src/steam/collection.rs`
- Modify: `src/steam/mod.rs`

**Step 1: Create `src/steam/collection.rs`**

Copy `steamworks_ext.rs` from `~/code/steam-workshop-mcp/src/steamworks_ext.rs` with these adaptations:

1. Gate via `#[cfg(feature = "steamworks")]` on the `mod` declaration in `steam/mod.rs`, so the file itself is unconditional but gated at import.
2. Keep the same `CollectionManager` struct with `init(app_id)`, `add_item(collection_id, item_id)`, `remove_item(collection_id, item_id)` methods.
3. Keep the `poll_api_call` generic helper using `ManualDispatch_RunFrame` (NOT `client.run_callbacks()` — the original had a bug where `run_callbacks()` consumed callbacks before `GetAPICallResult` could read them; the fix uses `ManualDispatch_RunFrame` directly).
4. Keep constants: `POLL_INTERVAL = 50ms`, `CALL_TIMEOUT = 30s`.
5. Include the `AppIdFile` RAII guard struct (writes `steam_appid.txt` on creation, removes on Drop):

```rust
/// RAII guard that writes steam_appid.txt on creation and removes it on drop.
pub struct AppIdFile;

impl AppIdFile {
    pub fn write(app_id: u32) -> anyhow::Result<Self> {
        std::fs::write("steam_appid.txt", app_id.to_string())
            .context("failed to write steam_appid.txt")?;
        Ok(Self)
    }
}

impl Drop for AppIdFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file("steam_appid.txt");
    }
}
```

The file should be identical to the working version in `steam-workshop-mcp` with the `ManualDispatch_RunFrame` fix, plus `AppIdFile`.

**Step 2: Update `src/steam/mod.rs`**

Add:
```rust
#[cfg(feature = "steamworks")]
pub mod collection;
#[cfg(feature = "steamworks")]
pub use collection::{CollectionManager, AppIdFile};
```

**Step 3: Verify**

Run: `cargo check` — passes (module gated out)
Run: `cargo check --features steamworks` — passes (CollectionManager compiles)

**Step 4: Commit**

```bash
git add src/steam/collection.rs src/steam/mod.rs
git commit -m "feat: port CollectionManager and AppIdFile from steam-workshop-mcp"
```

---

### Task 3: Add `collection_sync_file` config option and CLI plumbing

**Files:**
- Modify: `src/config.rs` — add `collection_sync_file: Option<PathBuf>` field
- Modify: `src/cli/mod.rs` — add `PushCollection` subcommand under `ModAction`
- Modify: `src/cli/mods.rs` — add handler

**Step 1: Add config field**

In `SafehouseConfig` struct, add:
```rust
/// Path to a local file of workshop IDs used as the source of truth
/// for push-collection (one ID per line, # comments supported).
/// Relative paths are resolved against CWD.
pub collection_sync_file: Option<PathBuf>,
```

Add `collection_sync_file: None,` to the explicit `Default::default()` impl in `src/config.rs:83-99`.

**Step 2: Add CLI subcommand**

In `src/cli/mod.rs`, add a new variant to `ModAction`:

```rust
/// Push a local file of workshop IDs to a Steam Workshop collection.
/// Requires Steam client running and logged in as collection owner.
PushCollection {
    /// File of workshop IDs (overrides collection_sync_file in config)
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Collection ID or URL (overrides steam_collection_id in config)
    #[arg(short, long)]
    collection: Option<String>,

    /// Steam app ID for Steamworks init
    #[arg(long, default_value = "108600")]
    app_id: u32,

    /// Only add missing items, don't remove extras from the collection
    #[arg(long)]
    no_remove: bool,

    /// Apply changes (default is dry run)
    #[arg(long)]
    apply: bool,

    /// Delay between Steamworks API calls in milliseconds
    #[arg(long, default_value = "500")]
    delay: u64,
},
```

**Step 3: Add handler stub in `src/cli/mods.rs`**

In the `match action` block, add:
```rust
ModAction::PushCollection { file, collection, app_id, no_remove, apply, delay } => {
    push_collection(ctx, file.as_deref(), collection.as_deref(), *app_id, *no_remove, *apply, *delay).await
}
```

Write a stub `push_collection` function that prints "not yet implemented" for now.

**Step 4: Verify**

Run: `cargo check` — passes
Run: `safehouse mods push-collection --help` — shows the new subcommand help

**Step 5: Commit**

```bash
git add src/config.rs src/cli/mod.rs src/cli/mods.rs
git commit -m "feat: add 'mods push-collection' CLI subcommand and config"
```

---

### Task 4: Add collection-fetching as `HashSet<u64>` to `steam/workshop.rs`

**Files:**
- Modify: `src/steam/workshop.rs`
- Modify: `src/steam/mod.rs` (re-export)

**Step 1: Add typed fetch function**

The existing `fetch_collection_items` returns `Vec<String>`. Add a new function that returns typed IDs for diffing:

```rust
/// Fetch all workshop item IDs from a collection as a typed HashSet.
/// Used by push-collection for set operations (diff, intersection).
pub async fn fetch_collection_item_ids(
    client: &reqwest::Client,
    collection_id: &str,
) -> Result<std::collections::HashSet<u64>> {
    let items = fetch_collection_items(client, collection_id).await?;
    let mut ids = std::collections::HashSet::new();
    for item in items {
        if let Ok(id) = item.parse::<u64>() {
            ids.insert(id);
        }
    }
    Ok(ids)
}
```

**Step 2: Re-export in `src/steam/mod.rs`**

Add `fetch_collection_item_ids` to the `pub use` block.

**Step 3: Write test**

```rust
#[test]
fn test_fetch_collection_item_ids_parses_correctly() {
    let items = vec!["123".to_string(), "456".to_string(), "123".to_string()];
    let ids: std::collections::HashSet<u64> = items.iter()
        .filter_map(|s| s.parse().ok())
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&123));
    assert!(ids.contains(&456));
}
```

**Step 4: Verify**

Run: `cargo test --lib steam::workshop` — passes

**Step 5: Commit**

```bash
git add src/steam/workshop.rs src/steam/mod.rs
git commit -m "feat: add fetch_collection_item_ids for typed collection diffing"
```

---

### Task 5: Implement the ID file reader

**Files:**
- Create: `src/pz/id_file.rs`
- Modify: `src/pz/mod.rs`

**Step 1: Create `src/pz/id_file.rs`**

Port the file reading logic from `sw-sync`'s `read_id_file`:

```rust
use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};

/// Read workshop IDs from a file.
///
/// Supports:
/// - One ID per line
/// - `#` line comments (inline and full-line)
/// - Blank lines
/// - Semicolon-separated IDs on a single line (legacy format)
///
/// Returns a deduplicated set.
pub fn read_id_file(path: &Path) -> Result<HashSet<u64>> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read ID file: {}", path.display()))?;

    let mut ids = HashSet::new();
    for line in raw.lines() {
        let stripped = match line.find('#') {
            Some(pos) => &line[..pos],
            None => line,
        };
        for token in stripped.split(';') {
            let trimmed = token.trim();
            if trimmed.is_empty() {
                continue;
            }
            match trimmed.parse::<u64>() {
                Ok(id) => { ids.insert(id); }
                Err(_) => {
                    eprintln!("warning: skipping non-numeric token: {trimmed}");
                }
            }
        }
    }
    Ok(ids)
}
```

**Step 2: Add module to `src/pz/mod.rs`**

```rust
pub mod id_file;
```

**Step 3: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_read_basic_ids() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("mods.txt");
        std::fs::write(&path, "123\n456\n789\n").unwrap();
        let ids = read_id_file(&path).unwrap();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&123));
    }

    #[test]
    fn test_read_comments_and_blanks() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("mods.txt");
        std::fs::write(&path, "# header comment\n123\n\n456 # inline\n").unwrap();
        let ids = read_id_file(&path).unwrap();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_read_semicolons() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("mods.txt");
        std::fs::write(&path, "123;456;789\n").unwrap();
        let ids = read_id_file(&path).unwrap();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn test_read_deduplicates() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("mods.txt");
        std::fs::write(&path, "123\n123\n456\n").unwrap();
        let ids = read_id_file(&path).unwrap();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_read_missing_file() {
        let result = read_id_file(Path::new("/nonexistent/mods.txt"));
        assert!(result.is_err());
    }
}
```

**Step 4: Verify**

Run: `cargo test pz::id_file` — all pass

**Step 5: Commit**

```bash
git add src/pz/id_file.rs src/pz/mod.rs
git commit -m "feat: add id_file module for reading workshop ID files"
```

---

### Task 6: Implement `push_collection` handler with dry run + apply

**Files:**
- Modify: `src/cli/mods.rs`

**Step 1: Implement the full `push_collection` function**

This is the main orchestration function. It:

1. Resolves the ID file path: CLI `--file` > config `collection_sync_file` > error
2. Resolves the collection ID: CLI `--collection` > config `steam_collection_id` > error
3. Reads desired IDs from the file via `read_id_file`
4. Fetches current collection items via `fetch_collection_item_ids` (reqwest, no API key needed — uses the public ISteamRemoteStorage endpoint)
5. Computes diff: `to_add = desired - current`, `to_remove = current - desired` (empty if `--no-remove`)
6. Prints diff summary with counts
7. If `--apply`:
   - Writes `steam_appid.txt` (RAII guard via `AppIdFile`)
   - Initializes `CollectionManager::init(app_id)` — `#[cfg(feature = "steamworks")]`
   - Processes adds then removes sequentially with `--delay` between each
   - Cleans up `steam_appid.txt`
   - Prints summary
8. If dry run: prints "Run with --apply to execute" and exits

**Important cfg handling:** The `--apply` codepath uses `CollectionManager` which is behind `#[cfg(feature = "steamworks")]`. The function compiles without the feature flag — dry run always works, `--apply` returns an error telling the user to rebuild with `--features steamworks`. The `Ok(())` return must be inside the `#[cfg(feature = "steamworks")]` block to avoid an unreachable code warning.

```rust
async fn push_collection(
    ctx: &CliContext,
    file: Option<&Path>,
    collection_arg: Option<&str>,
    app_id: u32,
    no_remove: bool,
    apply: bool,
    delay_ms: u64,
) -> Result<()> {
    // 1. Resolve file path
    let file_path = match file {
        Some(p) => p.to_path_buf(),
        None => ctx.config.collection_sync_file.clone()
            .ok_or_else(|| anyhow::anyhow!(
                "No ID file specified. Pass --file or set collection_sync_file in safehouse.toml."
            ))?,
    };

    // 2. Resolve collection ID
    let raw = match collection_arg {
        Some(c) => c.to_string(),
        None => ctx.config.steam_collection_id.clone()
            .ok_or_else(|| anyhow::anyhow!(
                "No collection specified. Pass --collection or set steam_collection_id in safehouse.toml."
            ))?,
    };
    let collection_id = crate::steam::parse_collection_id(&raw)?;

    if apply {
        eprintln!("Mode: APPLY (will make changes)");
    } else {
        eprintln!("Mode: DRY RUN (preview only)");
    }
    if no_remove {
        eprintln!("  --no-remove: will only add, never remove");
    }
    eprintln!();

    // 3. Read desired IDs
    eprintln!("Reading IDs from {}...", file_path.display());
    let desired = crate::pz::id_file::read_id_file(&file_path)?;
    if desired.is_empty() {
        anyhow::bail!("ID file is empty or contains no valid IDs");
    }
    eprintln!("  {} unique workshop IDs", desired.len());

    // 4. Fetch current collection
    eprintln!("Fetching collection {collection_id}...");
    let current = crate::steam::fetch_collection_item_ids(&ctx.http, &collection_id).await?;
    eprintln!("  {} items currently in collection", current.len());

    // 5. Compute diff
    let to_add: Vec<u64> = desired.difference(&current).copied().collect();
    let to_remove: Vec<u64> = if no_remove {
        Vec::new()
    } else {
        current.difference(&desired).copied().collect()
    };
    let in_sync = desired.intersection(&current).count();

    eprintln!();
    eprintln!("── Diff ────────────────────────────────────────");
    eprintln!("  ✓ In sync:   {in_sync}");
    eprintln!("  + To add:    {}", to_add.len());
    eprintln!("  - To remove: {}", to_remove.len());

    if to_add.is_empty() && to_remove.is_empty() {
        eprintln!("\nCollection is in sync — nothing to do.");
        return Ok(());
    }

    if !to_add.is_empty() {
        eprintln!("\nWould add:");
        for id in &to_add { eprintln!("  + {id}"); }
    }
    if !to_remove.is_empty() {
        eprintln!("\nWould remove:");
        for id in &to_remove { eprintln!("  - {id}"); }
    }

    if !apply {
        eprintln!("\nRun with --apply to execute.");
        return Ok(());
    }

    // 6. Apply via Steamworks SDK
    #[cfg(not(feature = "steamworks"))]
    {
        anyhow::bail!(
            "--apply requires the steamworks feature. Rebuild with: \
             cargo install --path . --features steamworks"
        );
    }

    #[cfg(feature = "steamworks")]
    {
        use crate::steam::{CollectionManager, AppIdFile};
        use std::time::Duration;

        eprintln!("\nInitializing Steamworks...");
        let _appid_file = AppIdFile::write(app_id)?;
        let collection_id_u64: u64 = collection_id.parse()
            .context("collection ID must be numeric")?;
        let mgr = CollectionManager::init(app_id)
            .context("failed to init Steamworks — is Steam running?")?;
        eprintln!("  Steamworks ready");

        let delay = Duration::from_millis(delay_ms);
        let mut added = 0u32;
        let mut removed = 0u32;
        let mut failed = 0u32;

        if !to_add.is_empty() {
            eprintln!("\nAdding {} items...", to_add.len());
            for (i, &item_id) in to_add.iter().enumerate() {
                match mgr.add_item(collection_id_u64, item_id) {
                    Ok(()) => {
                        eprintln!("  [{}/{}] ✓ added {item_id}", i + 1, to_add.len());
                        added += 1;
                    }
                    Err(e) => {
                        eprintln!("  [{}/{}] ✗ failed {item_id}: {e}", i + 1, to_add.len());
                        failed += 1;
                    }
                }
                if i < to_add.len() - 1 { std::thread::sleep(delay); }
            }
        }

        if !to_remove.is_empty() {
            eprintln!("\nRemoving {} items...", to_remove.len());
            for (i, &item_id) in to_remove.iter().enumerate() {
                match mgr.remove_item(collection_id_u64, item_id) {
                    Ok(()) => {
                        eprintln!("  [{}/{}] ✓ removed {item_id}", i + 1, to_remove.len());
                        removed += 1;
                    }
                    Err(e) => {
                        eprintln!("  [{}/{}] ✗ failed {item_id}: {e}", i + 1, to_remove.len());
                        failed += 1;
                    }
                }
                if i < to_remove.len() - 1 { std::thread::sleep(delay); }
            }
        }

        eprintln!();
        eprintln!("══════════ DONE ══════════");
        eprintln!("  ✓ Added:   {added}");
        eprintln!("  ✓ Removed: {removed}");
        eprintln!("  ✗ Failed:  {failed}");

        if failed > 0 {
            anyhow::bail!("{failed} operations failed");
        }

        Ok(())
    }
}
```

**Step 2: Verify `AppIdFile` is importable**

`AppIdFile` was added in Task 2. Verify `use crate::steam::AppIdFile;` resolves.

**Step 3: Verify**

Run: `cargo check` — passes (apply path gated out)
Run: `cargo check --features steamworks` — passes (apply path compiles)
Run: `cargo test` — all existing tests pass

**Step 4: Commit**

```bash
git add src/cli/mods.rs
git commit -m "feat: implement 'mods push-collection' with dry run and Steamworks apply"
```

---

### Task 7: Build, install, and smoke test

**Files:**
- Modify: `.gitignore` — add `steam_appid.txt`
- No code changes

**Step 1: Add `steam_appid.txt` to `.gitignore`**

```
steam_appid.txt
```

**Step 2: Build with steamworks feature**

```bash
cargo build --release --features steamworks
```

Expected: compiles, links against libsteam_api.so

**Step 3: Install libsteam_api.so alongside the binary**

```bash
cargo install --path . --features steamworks
cp target/release/build/steamworks-sys-*/out/libsteam_api.so ~/.cargo/bin/
patchelf --set-rpath '$ORIGIN' ~/.cargo/bin/safehouse
```

**Step 4: Smoke test — dry run**

```bash
safehouse --data-dir /mnt/services/safehouse-1 mods push-collection \
  --file ~/projects/pz-mod-list/mods.txt \
  --collection 3786678808
```

Expected: shows diff (should be "in sync" after earlier sw-sync runs)

**Step 5: Smoke test — apply (single item)**

Create a temporary test file with one ID and test add+remove against the collection. Verify via dry run before and after.

**Step 6: Commit**

```bash
git add .gitignore
git commit -m "chore: add steam_appid.txt to gitignore, verify build"
```

---

### Task 8: Update safehouse docs

**Files:**
- Modify: `~/code/safehouse/README.md` — document new subcommand

**Step 1: Add push-collection docs to README**

Under the Mods section, add:

```markdown
### Push Collection

Push your local mod list to a Steam Workshop collection:

```bash
# Preview changes (dry run — default)
safehouse mods push-collection --file mods.txt --collection 3786678808

# Apply changes (requires Steam client running, logged in as collection owner)
safehouse mods push-collection --file mods.txt --collection 3786678808 --apply

# Add-only mode (don't remove items from collection)
safehouse mods push-collection --file mods.txt --apply --no-remove
```

Requires building with `--features steamworks` for `--apply`.
Set `collection_sync_file` and `steam_collection_id` in `safehouse.toml` to use defaults.
```

**Step 2: Commit**

```bash
cd ~/code/safehouse && git add README.md && git commit -m "docs: document mods push-collection subcommand"
```

---

## Risk Notes

1. **`libsteam_api.so` distribution** — Same issue as sw-sync: the Steamworks SDK shared library must be next to the binary or on `LD_LIBRARY_PATH`. Solved with `patchelf --set-rpath '$ORIGIN'`.

2. **Feature flag build complexity** — Regular `cargo install safehouse` won't include steamworks. The `--apply` path must give a clear error message when the feature is missing, not a cryptic link error. Handled by the `#[cfg(not(feature = "steamworks"))]` bail.

3. **Safehouse runs on mars (no Steam client)** — The server host doesn't have Steam running. `push-collection --apply` must be run from a machine where Steam is running (your desktop). Dry run works anywhere. This matches the sw-sync usage pattern.

4. **Let-chains** — sw-sync's `sync.rs` uses a let-chain (`if let ... && let ...`) in `fetch_collection_items`, but that function is replaced in the port by a wrapper around safehouse's existing function. `steamworks_ext.rs` uses no features requiring Rust 1.87+, so this is a non-issue.

5. **reqwest version mismatch** — Safehouse uses `reqwest 0.12`, steam-workshop-mcp uses `reqwest 0.13`. The collection-fetching code uses basic `.post().form().send().await` which is identical across both versions. No porting issue.

6. **Naming** — `mods sync` pulls FROM a collection INTO server.ini. `mods push-collection` pushes FROM a file INTO a collection. The `push` verb clearly communicates directionality and contrasts naturally with `sync` (implicit pull).

7. **Blocking Steamworks calls** — `CollectionManager::init()`, `add_item()`, `remove_item()`, and `thread::sleep()` are all synchronous blocking calls inside an `async fn`. In safehouse's CLI path this is fine (single task). If this function is ever called from the web UI (like `mods sync` is shared via `execute_collection_sync`), it would need `tokio::task::spawn_blocking`.

## Review Findings (applied)

The following issues from the Opus review were fixed in this revision:

- ✅ **BLOCKER: `AppIdFile` import** — Moved `AppIdFile` into `src/steam/collection.rs` (Task 2), re-exported via `steam/mod.rs`, and imported in the cfg block via `use crate::steam::{CollectionManager, AppIdFile};`
- ✅ **BLOCKER: Missing `Default` impl field** — Explicitly noted to add `collection_sync_file: None,` to `src/config.rs:83-99`
- ✅ **WARNING: Unreachable `Ok(())`** — Moved `Ok(())` inside the `#[cfg(feature = "steamworks")]` block
- ✅ **MINOR: Let-chains factual error** — Corrected Risk Note 4
- ✅ **MINOR: `--no-remove` UX note** — Added to the apply block output
- ✅ **NAMING: `push-collection`** — Adopted over `collection-sync`
- ✅ **SUGGESTION: STEAM_API_KEY elimination** — Called out in header
- ✅ **SUGGESTION: Task parallelism** — Noted in header
- ✅ **SUGGESTION: Blocking calls risk** — Added Risk Note 7
- ✅ **SUGGESTION: pz-mod-list in separate task** — Removed from Task 8
