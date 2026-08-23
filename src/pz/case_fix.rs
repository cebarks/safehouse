use std::path::Path;

use anyhow::Result;

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
