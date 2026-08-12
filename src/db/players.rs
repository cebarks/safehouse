use anyhow::Result;
use rusqlite::params;

use crate::db::Database;

impl Database {
    pub fn record_player_join(&self, name: &str, steam_id: Option<&str>) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO player_sessions (player_name, steam_id) VALUES (?1, ?2)",
            params![name, steam_id],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn record_player_leave(&self, name: &str) -> Result<()> {
        // Update the most recent open session for this player.
        // Uses a subquery because UPDATE...ORDER BY...LIMIT requires
        // SQLITE_ENABLE_UPDATE_DELETE_LIMIT which rusqlite's bundled
        // SQLite does not enable by default.
        self.conn.execute(
            "UPDATE player_sessions SET left_at = datetime('now')
             WHERE id = (
               SELECT id FROM player_sessions
               WHERE player_name = ?1 AND left_at IS NULL
               ORDER BY joined_at DESC LIMIT 1
             )",
            params![name],
        )?;
        Ok(())
    }

    pub fn recent_sessions(&self, limit: usize) -> Result<Vec<(String, String, Option<String>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT player_name, joined_at, left_at FROM player_sessions
             ORDER BY joined_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_record_join_and_recent_sessions() {
        let db = Database::open_in_memory().unwrap();
        let id = db.record_player_join("Alice", Some("steam123")).unwrap();
        assert!(id > 0);

        let sessions = db.recent_sessions(10).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].0, "Alice");
        assert!(sessions[0].2.is_none()); // not left yet
    }

    #[test]
    fn test_record_leave_closes_session() {
        let db = Database::open_in_memory().unwrap();
        db.record_player_join("Bob", None).unwrap();
        db.record_player_leave("Bob").unwrap();

        let sessions = db.recent_sessions(10).unwrap();
        assert_eq!(sessions.len(), 1);
        assert!(sessions[0].2.is_some()); // left_at is set
    }

    #[test]
    fn test_leave_closes_most_recent_open_session() {
        let db = Database::open_in_memory().unwrap();
        db.record_player_join("Charlie", None).unwrap();
        db.record_player_leave("Charlie").unwrap();
        db.record_player_join("Charlie", None).unwrap();
        db.record_player_leave("Charlie").unwrap();

        let sessions = db.recent_sessions(10).unwrap();
        assert_eq!(sessions.len(), 2);
        // Both sessions should have left_at set
        assert!(sessions[0].2.is_some());
        assert!(sessions[1].2.is_some());
    }

    #[test]
    fn test_recent_sessions_limit() {
        let db = Database::open_in_memory().unwrap();
        for i in 0..5 {
            db.record_player_join(&format!("Player{i}"), None).unwrap();
        }
        let sessions = db.recent_sessions(3).unwrap();
        assert_eq!(sessions.len(), 3);
    }
}
