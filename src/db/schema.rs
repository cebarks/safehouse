use rusqlite::Connection;

const MIGRATIONS: &[&str] = &[include_str!("../../migrations/001_initial.sql")];

pub fn run_migrations(conn: &Connection) -> rusqlite::Result<()> {
    let current: i32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let target = (i + 1) as i32;
        if current < target {
            conn.execute_batch("BEGIN EXCLUSIVE")?;
            match (|| -> rusqlite::Result<()> {
                conn.execute_batch(sql)?;
                conn.pragma_update(None, "user_version", target)?;
                conn.execute_batch("COMMIT")?;
                Ok(())
            })() {
                Ok(()) => {}
                Err(e) => {
                    let _ = conn.execute_batch("ROLLBACK");
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}
