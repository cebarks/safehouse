use anyhow::Result;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use rand::rngs::OsRng;
use rusqlite::params;

use super::Database;

trait OptionalExt<T> {
    fn optional(self) -> rusqlite::Result<Option<T>>;
}

impl<T> OptionalExt<T> for rusqlite::Result<T> {
    fn optional(self) -> rusqlite::Result<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

impl Database {
    pub fn create_user(&self, username: &str, password: &str) -> Result<()> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("hash error: {e}"))?
            .to_string();
        self.conn.execute(
            "INSERT INTO users (username, password_hash) VALUES (?1, ?2)",
            params![username, hash],
        )?;
        Ok(())
    }

    pub fn verify_password(&self, username: &str, password: &str) -> Result<bool> {
        let hash: Option<String> = self
            .conn
            .query_row(
                "SELECT password_hash FROM users WHERE username = ?1",
                params![username],
                |r| r.get(0),
            )
            .optional()?;

        let Some(hash) = hash else {
            return Ok(false);
        };
        let parsed = PasswordHash::new(&hash).map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    }

    pub fn user_exists(&self, username: &str) -> Result<bool> {
        let count: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM users WHERE username = ?1",
            params![username],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_create_user_and_verify() {
        let db = Database::open_in_memory().unwrap();
        db.create_user("admin", "secret123").unwrap();
        assert!(db.verify_password("admin", "secret123").unwrap());
        assert!(!db.verify_password("admin", "wrong").unwrap());
    }

    #[test]
    fn test_user_exists() {
        let db = Database::open_in_memory().unwrap();
        assert!(!db.user_exists("admin").unwrap());
        db.create_user("admin", "pass").unwrap();
        assert!(db.user_exists("admin").unwrap());
    }

    #[test]
    fn test_verify_nonexistent_user() {
        let db = Database::open_in_memory().unwrap();
        assert!(!db.verify_password("nobody", "pass").unwrap());
    }
}
