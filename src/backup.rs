use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;

/// Create a .tar.gz snapshot of `source_dir` into `dest_dir`.
/// Returns the path of the created archive.
pub fn create_snapshot(
    source_dir: &Path,
    dest_dir: &Path,
    server_name: &str,
    label: Option<&str>,
) -> Result<PathBuf> {
    std::fs::create_dir_all(dest_dir)?;
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let label_part = label.map(|l| format!("_{l}")).unwrap_or_default();
    let filename = format!("{server_name}_{timestamp}{label_part}.tar.gz");
    let out_path = dest_dir.join(&filename);
    let tmp_path = dest_dir.join(format!(".{filename}.tmp"));

    let file = std::fs::File::create(&tmp_path)
        .with_context(|| format!("cannot create backup file {}", tmp_path.display()))?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut builder = tar::Builder::new(enc);
    builder
        .append_dir_all(".", source_dir)
        .context("failed to archive save directory")?;
    builder.finish()?;

    // Atomic rename: only the complete archive gets the final name
    std::fs::rename(&tmp_path, &out_path)
        .with_context(|| format!("cannot rename temp backup to {}", out_path.display()))?;

    Ok(out_path)
}

/// Extract a snapshot archive into `dest_dir` (overwrites contents).
pub fn restore_snapshot(archive: &Path, dest_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dest_dir)?;
    let file = std::fs::File::open(archive)
        .with_context(|| format!("cannot open backup {}", archive.display()))?;
    let dec = flate2::read::GzDecoder::new(file);
    let mut ar = tar::Archive::new(dec);
    ar.unpack(dest_dir)
        .context("failed to extract backup archive")?;
    Ok(())
}

/// List snapshot files in `backup_dir`, sorted newest first.
pub fn list_snapshots(backup_dir: &Path) -> Result<Vec<PathBuf>> {
    if !backup_dir.exists() {
        return Ok(vec![]);
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(backup_dir)?
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x == "gz")
        })
        .map(|e| e.path())
        .collect();
    entries.sort_by_key(|p| std::cmp::Reverse(p.metadata().ok().and_then(|m| m.modified().ok())));
    Ok(entries)
}

/// Delete snapshots older than `retain_days`, keeping at least `min_keep`.
pub fn prune_snapshots(
    backup_dir: &Path,
    retain_days: u32,
    min_keep: usize,
) -> Result<Vec<PathBuf>> {
    let all = list_snapshots(backup_dir)?;
    let cutoff = Utc::now() - chrono::Duration::days(i64::from(retain_days));
    let mut pruned = vec![];
    for path in all.iter().skip(min_keep) {
        let mtime = path.metadata()?.modified()?;
        let mtime_utc: chrono::DateTime<Utc> = mtime.into();
        if mtime_utc < cutoff {
            std::fs::remove_file(path)?;
            pruned.push(path.clone());
        }
    }
    Ok(pruned)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_and_list_snapshots() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        // Create fake save data
        std::fs::write(src.path().join("map_zone.bin"), b"fake data").unwrap();

        let snap_path = create_snapshot(src.path(), dst.path(), "testworld", None).unwrap();
        assert!(snap_path.exists());
        assert_eq!(snap_path.extension().unwrap(), "gz");

        let snaps = list_snapshots(dst.path()).unwrap();
        assert_eq!(snaps.len(), 1);
    }

    #[test]
    fn test_restore_snapshot() {
        let src = tempdir().unwrap();
        let snap_dir = tempdir().unwrap();
        let restore_dir = tempdir().unwrap();

        std::fs::write(src.path().join("map_zone.bin"), b"original").unwrap();
        let snap = create_snapshot(src.path(), snap_dir.path(), "testworld", None).unwrap();

        restore_snapshot(&snap, restore_dir.path()).unwrap();
        assert!(restore_dir.path().join("map_zone.bin").exists());
        let content = std::fs::read_to_string(restore_dir.path().join("map_zone.bin")).unwrap();
        assert_eq!(content, "original");
    }

    #[test]
    fn test_create_snapshot_with_label() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        std::fs::write(src.path().join("data.bin"), b"data").unwrap();

        let snap = create_snapshot(src.path(), dst.path(), "testworld", Some("pre-wipe")).unwrap();
        let filename = snap.file_name().unwrap().to_string_lossy();
        assert!(filename.contains("_pre-wipe"));
        assert!(filename.ends_with(".tar.gz"));
    }

    #[test]
    fn test_list_snapshots_empty_dir() {
        let dir = tempdir().unwrap();
        let snaps = list_snapshots(dir.path()).unwrap();
        assert!(snaps.is_empty());
    }

    #[test]
    fn test_list_snapshots_missing_dir() {
        let snaps = list_snapshots(Path::new("/nonexistent/path")).unwrap();
        assert!(snaps.is_empty());
    }
}
