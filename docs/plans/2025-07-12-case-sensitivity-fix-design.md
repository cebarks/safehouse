# Case-Sensitivity Fix for Linux PZ Server

**Date**: 2025-07-12
**Status**: Approved

## Problem

Project Zomboid was developed for Windows (case-insensitive NTFS). Its Java code lowercases file paths internally when resolving references — e.g., `AnimSets` becomes `animsets`. On Linux (case-sensitive ext4/xfs), these lookups fail with `FileNotFoundException`.

This primarily affects Workshop mods (authors develop on Windows and never notice casing issues), but can also affect vanilla PZ files when mods reference them with wrong casing.

Example error:

```
java.io.FileNotFoundException: steamapps/workshop/content/108600/2335368829/mods/authentic z - current/42/media/animsets/player/ext/ext01.xml (No such file or directory)
```

The real path on disk has `AnimSets` (capital A and S), but PZ looks up `animsets`.

## Solution

Automatically create lowercase symlinks alongside any file or directory with uppercase ASCII characters in its name. When PZ requests the lowercased path, the symlink resolves to the real file.

## Design Decisions

| Decision | Choice | Rationale |
| --- | --- | --- |
| When to run | Both: auto on server start + standalone CLI command | Auto prevents "forgot to run" failures; CLI useful for debugging |
| Scope | Full server install dir (mods + vanilla) | Mods referencing vanilla paths with wrong case is a real failure mode |
| Stale symlink handling | Clean-then-create (remove all dangling symlinks, then create fresh) | No manifest to maintain, dangling symlinks in a PZ install are never useful |
| Fatality | Non-fatal (best-effort) | A partially-working server is better than refusing to start |

## Alternatives Considered

| Alternative | Why rejected |
| --- | --- |
| **Case-insensitive ext4 (casefold)** — `tune2fs -O casefold` + per-directory `chattr +F` (kernel 5.2+) | Requires specific filesystem configuration the user may not control, can't be applied to existing populated directories, ext4-specific |
| **ciopfs (FUSE case-insensitive layer)** | Adds a FUSE runtime dependency, measurable I/O overhead for a game server, another service to manage |
| **overlayfs with casefold lower** | Complex setup, requires specific kernel support, overkill for the problem |

The symlink approach works on any Linux filesystem, has zero runtime overhead (symlink resolution is in-kernel), and requires no extra dependencies or system configuration.

## Core Algorithm

New module: `src/pz/case_fix.rs`

### Public API

```rust
pub struct CaseFixResult {
    pub symlinks_created: u32,
    pub symlinks_cleaned: u32,
    pub failures: u32,
    pub warnings: Vec<String>,
}

/// Scan `root` recursively, clean dangling symlinks, and create
/// lowercase symlinks for any file/directory with uppercase ASCII chars.
pub fn fix_case(root: &Path) -> Result<CaseFixResult>
```

### Walk Strategy

Single depth-first traversal of the directory tree under `root`:

1. **Clean dangling symlinks.** For every symlink encountered, check if its target exists. If dangling, remove it and increment `symlinks_cleaned`.

2. **Create lowercase symlinks.** For every file and directory whose name contains at least one uppercase ASCII character:
   - Compute the lowercase name via `make_ascii_lowercase` (ASCII-only — PZ's case-folding is ASCII-only)
   - If the lowercase name equals the actual name, skip
   - If something already exists at the lowercase path (real file/dir or valid symlink), skip and push a warning
   - Otherwise, create a **relative symlink**: `lowercase_name -> actual_name` in the same parent directory
   - Increment `symlinks_created`

3. **Before creating a symlink**, use `std::fs::symlink_metadata()` (lstat — does NOT follow symlinks) to check whether anything already exists at the target path. This is critical because `Path::exists()` follows symlinks, making dangling symlinks invisible — but `symlink()` still fails with EEXIST on the occupied directory entry. If `symlink_metadata` finds a dangling symlink at the target path, remove it first, then create the new symlink. This makes the algorithm order-independent within a directory (readdir order is non-deterministic on ext4).

4. **Do not follow symlinks** during traversal — `walkdir` with `follow_links(false)`. Inspect symlinks to check for dangling, but only recurse into real directories. This avoids symlink cycles.

### Depth-First Matters

Symlinks must be created at every directory level. If the real path is:

```
media/AnimSets/Player/ext/Ext01.xml
```

PZ may look up:

```
media/animsets/player/ext/ext01.xml
```

Required symlinks:

- `media/animsets` → `AnimSets`
- `media/AnimSets/player` → `Player`
- `media/AnimSets/Player/ext/ext01.xml` → `Ext01.xml`

Note: symlinks are created inside the real (cased) directory, pointing from the lowercase name to the real entry. We don't need to also create symlinks inside the symlinked lowercase paths because the filesystem resolves the chain — accessing `media/animsets/player/` follows `animsets` → `AnimSets`, then inside `AnimSets/` finds `player` → `Player`.

## Integration Points

### CLI Command: `safehouse mods fix-case`

New `FixCase` variant on `ModAction` in `src/cli/mod.rs`. Handler in `src/cli/mods.rs`:

1. Resolve `config.server_install_dir`
2. Call `fix_case(&config.server_install_dir)`
3. Print human-readable summary: symlinks created, cleaned, failures, and any collision warnings

### Auto-Run on Server Start

In `container.rs::create_and_start`, before `docker.create_container`:

```rust
match fix_case(&config.server_install_dir) {
    Ok(result) => {
        info!(created = result.symlinks_created, cleaned = result.symlinks_cleaned, "case-fix scan complete");
        for w in &result.warnings { warn!("{w}"); }
    }
    Err(e) => warn!("case-fix scan failed, continuing: {e}"),
}
```

Non-fatal — if the scan fails entirely, log and continue. The container will either work (most files were fine) or fail with a clearer PZ-level error.

> **Ordering constraint:** `fix_case` MUST run before `create_container` because the `:Z` bind mount flag triggers a recursive SELinux relabel (`chcon -R`) during container creation. Symlinks created after this point would lack the `container_file_t` context, causing access denials when PZ tries to follow them inside the container.

### After SteamCMD Install

Also call `fix_case` at the end of `run_steamcmd_install` in `container.rs`. SteamCMD downloads fresh mod files that may have uppercase paths — fixing them immediately avoids a "must restart to pick up symlinks" gap.

### Host-Side Execution

The scan runs on the **host filesystem**, not inside the container. Since `server_install_dir` is bind-mounted into the container at `/server`, symlinks created on the host are visible inside the container. Relative symlink targets resolve correctly in both contexts because they reference sibling entries in the same directory.

## Error Handling

| Scenario | Behavior |
| --- | --- |
| Root path doesn't exist | Return `Err`. Caller logs warning and continues — container will fail with a clearer error anyway |
| Permission denied on subdirectory | Log warning for that path, skip, continue |
| Symlink creation fails (read-only FS, SELinux) | Log warning, increment `failures`, continue |
| Symlink cycles | `walkdir` with `follow_links(false)` prevents recursion into symlinks |
| Name collision (both `Textures/` and `textures/` exist) | Skip, push warning — this is a mod packaging bug that can't be auto-fixed |
| Non-UTF8 filenames | `OsStr` comparison works. `make_ascii_lowercase` only affects ASCII bytes — non-ASCII names are left alone |

## Known Limitations

- **Runtime mod downloads.** PZ can auto-download Workshop mods during server runtime (when processing its WorkshopItems list on startup). These newly downloaded files won't have lowercase symlinks until the next server restart triggers `fix_case`. A server restart is required after mod downloads to pick up symlinks. A future enhancement could use inotify/fanotify to auto-create symlinks on the workshop directory.

## Dependencies

Add `walkdir` crate for recursive traversal. It handles symlink cycles, per-entry permission errors, and depth-first ordering. Widely used, minimal transitive dependencies (`same-file` on Linux).

## Testing

Unit tests in `src/pz/case_fix.rs` using `tempdir` (already a dev-dependency). Each test creates a synthetic directory tree, runs `fix_case`, asserts on result + filesystem state.

| Test | Setup | Assertion |
| --- | --- | --- |
| Basic fix | `Media/AnimSets/Player/` | Symlinks `media`, `animsets`, `player` created |
| Already lowercase | `media/scripts/` | Zero symlinks created |
| Mixed content | Some dirs uppercase, some lowercase, some files uppercase | Only uppercase entries get symlinks |
| Dangling cleanup | Pre-create dangling symlink | Removed, `symlinks_cleaned == 1` |
| Collision | Both `Textures/` and `textures/` as real dirs | No symlink, warning emitted |
| Idempotent | Run `fix_case` twice | Second run: zero created, zero cleaned |
| Chain resolution | `Media/AnimSets/Player/file.txt` | `std::fs::read(root.join("media/animsets/player/file.txt"))` succeeds and returns correct content |
| Dangling at target path | `ANIMSETS/` dir + dangling `animsets -> NonExistent` symlink | Dangling removed, new `animsets -> ANIMSETS` created, path resolves |
| Files not just dirs | `Icon.png` | `icon.png` symlink exists |
| Non-ASCII passthrough | `Ärzte/` | No ASCII-lowercase symlink to itself |

## Performance

A heavily-modded PZ install (50+ mods) has ~50k files. A single `walkdir` traversal + symlink creation completes well under a second. Negligible compared to PZ's 30-60 second Java startup.
