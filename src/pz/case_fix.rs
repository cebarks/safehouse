use std::os::unix::fs as unix_fs;
use std::path::Path;

use anyhow::Result;
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
    anyhow::ensure!(root.is_dir(), "case-fix root does not exist: {}", root.display());

    let mut result = CaseFixResult::default();

    // Single depth-first walk. Do NOT follow symlinks to avoid cycles.
    // min_depth(1) skips the root entry itself — we only fix contents.
    for entry in WalkDir::new(root).min_depth(1).follow_links(false) {
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

        fs::create_dir_all(root.join("Media/AnimSets/Player")).expect("mkdir");

        let result = fix_case(root).expect("fix_case");
        assert!(result.symlinks_created >= 3, "expected at least 3 symlinks, got {}", result.symlinks_created);

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

        fs::create_dir_all(root.join("Media")).expect("mkdir");
        fs::create_dir_all(root.join("scripts")).expect("mkdir");
        fs::write(root.join("Media/Icon.png"), b"img").expect("write");

        let result = fix_case(root).expect("fix_case");

        assert!(root.join("media").symlink_metadata().expect("media").is_symlink());
        assert!(root.join("Media/icon.png").symlink_metadata().expect("icon.png").is_symlink());
        assert!(!root.join("Scripts").exists());
    }

    #[test]
    fn test_dangling_cleanup() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();

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

        fs::create_dir_all(root.join("Media/AnimSets/Player")).expect("mkdir");
        fs::write(root.join("Media/AnimSets/Player/file.txt"), b"hello").expect("write");

        fix_case(root).expect("fix_case");

        let content = fs::read(root.join("media/animsets/player/file.txt"))
            .expect("reading through symlink chain should work");
        assert_eq!(content, b"hello");
    }

    #[test]
    fn test_dangling_at_target_path() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();

        fs::create_dir_all(root.join("ANIMSETS")).expect("mkdir");
        unix_fs::symlink("NonExistent", root.join("animsets")).expect("symlink");

        let result = fix_case(root).expect("fix_case");

        assert!(result.symlinks_cleaned >= 1);
        assert!(result.symlinks_created >= 1);

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

        fs::create_dir_all(root.join("Ärzte")).expect("mkdir");

        let result = fix_case(root).expect("fix_case");
        assert_eq!(result.symlinks_created, 0, "non-ASCII-only name should produce no symlink");
    }
}
