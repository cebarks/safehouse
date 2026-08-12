use anyhow::Result;
use rusqlite::params;

use crate::db::Database;

#[derive(Debug, Clone)]
pub struct BackupSnapshot {
    pub id: i64,
    pub filename: String,
    pub label: Option<String>,
    pub size_bytes: Option<i64>,
    pub server_name: String,
    pub created_at: String,
    pub created_by: String,
}

impl Database {
    pub fn record_backup(
        &self,
        filename: &str,
        label: Option<&str>,
        size_bytes: Option<i64>,
        server_name: &str,
        created_by: &str,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO backup_snapshots (filename, label, size_bytes, server_name, created_by)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![filename, label, size_bytes, server_name, created_by],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_backups(&self) -> Result<Vec<BackupSnapshot>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, filename, label, size_bytes, server_name, created_at, created_by
             FROM backup_snapshots ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(BackupSnapshot {
                id: r.get(0)?,
                filename: r.get(1)?,
                label: r.get(2)?,
                size_bytes: r.get(3)?,
                server_name: r.get(4)?,
                created_at: r.get(5)?,
                created_by: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn delete_backup_record(&self, filename: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM backup_snapshots WHERE filename = ?1",
            params![filename],
        )?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_list_backups() {
        let db = Database::open_in_memory().unwrap();
        let id = db
            .record_backup("snap1.tar.gz", Some("label"), Some(1024), "server", "cli")
            .unwrap();
        assert!(id > 0);

        let backups = db.list_backups().unwrap();
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].filename, "snap1.tar.gz");
        assert_eq!(backups[0].label.as_deref(), Some("label"));
        assert_eq!(backups[0].size_bytes, Some(1024));
    }

    #[test]
    fn test_delete_backup_record() {
        let db = Database::open_in_memory().unwrap();
        db.record_backup("snap1.tar.gz", None, None, "server", "cli")
            .unwrap();
        db.delete_backup_record("snap1.tar.gz").unwrap();
        let backups = db.list_backups().unwrap();
        assert!(backups.is_empty());
    }
}
