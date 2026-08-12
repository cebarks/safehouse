use anyhow::Result;
use rusqlite::params;

use crate::db::Database;
use crate::steam::WorkshopModInfo;

#[derive(Debug, Clone)]
pub struct CachedMod {
    pub workshop_id: String,
    pub mod_folder_name: Option<String>,
    pub title: String,
    pub author: Option<String>,
}

impl Database {
    pub fn upsert_workshop_mod(&self, info: &WorkshopModInfo, folder: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO workshop_mods (workshop_id, mod_folder_name, title, author, description)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(workshop_id) DO UPDATE SET
               mod_folder_name = excluded.mod_folder_name,
               title = excluded.title,
               author = excluded.author,
               description = excluded.description,
               fetched_at = datetime('now')",
            params![
                info.workshop_id,
                folder,
                info.title,
                info.author,
                info.description
            ],
        )?;
        Ok(())
    }

    pub fn get_cached_mod(&self, workshop_id: &str) -> Result<Option<CachedMod>> {
        let mut stmt = self.conn.prepare(
            "SELECT workshop_id, mod_folder_name, title, author
             FROM workshop_mods WHERE workshop_id = ?1",
        )?;
        let mut rows = stmt.query(params![workshop_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(CachedMod {
                workshop_id: row.get(0)?,
                mod_folder_name: row.get(1)?,
                title: row.get(2)?,
                author: row.get(3)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn save_mod_profile(
        &self,
        name: &str,
        desc: Option<&str>,
        ids: &[String],
        names: &[String],
    ) -> Result<()> {
        let ids_json = serde_json::to_string(ids)?;
        let names_json = serde_json::to_string(names)?;
        self.conn.execute(
            "INSERT INTO mod_profiles (name, description, workshop_ids, mod_names)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(name) DO UPDATE SET
               description = excluded.description,
               workshop_ids = excluded.workshop_ids,
               mod_names = excluded.mod_names,
               updated_at = datetime('now')",
            params![name, desc, ids_json, names_json],
        )?;
        Ok(())
    }

    pub fn get_mod_profile(&self, name: &str) -> Result<Option<(Vec<String>, Vec<String>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT workshop_ids, mod_names FROM mod_profiles WHERE name = ?1")?;
        let mut rows = stmt.query(params![name])?;
        if let Some(row) = rows.next()? {
            let ids: Vec<String> = serde_json::from_str(&row.get::<_, String>(0)?)?;
            let names: Vec<String> = serde_json::from_str(&row.get::<_, String>(1)?)?;
            Ok(Some((ids, names)))
        } else {
            Ok(None)
        }
    }

    pub fn list_mod_profiles(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM mod_profiles ORDER BY name")?;
        let names = stmt
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(names)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_upsert_and_get_cached_mod() {
        let db = Database::open_in_memory().unwrap();
        let info = WorkshopModInfo {
            workshop_id: "111".to_string(),
            title: "TestMod".to_string(),
            author: Some("Author".to_string()),
            description: None,
        };
        db.upsert_workshop_mod(&info, Some("TestModFolder"))
            .unwrap();
        let cached = db.get_cached_mod("111").unwrap().unwrap();
        assert_eq!(cached.title, "TestMod");
        assert_eq!(cached.mod_folder_name.as_deref(), Some("TestModFolder"));
    }

    #[test]
    fn test_upsert_updates_on_conflict() {
        let db = Database::open_in_memory().unwrap();
        let info1 = WorkshopModInfo {
            workshop_id: "111".to_string(),
            title: "OldTitle".to_string(),
            author: None,
            description: None,
        };
        db.upsert_workshop_mod(&info1, None).unwrap();
        let info2 = WorkshopModInfo {
            workshop_id: "111".to_string(),
            title: "NewTitle".to_string(),
            author: Some("NewAuthor".to_string()),
            description: None,
        };
        db.upsert_workshop_mod(&info2, Some("folder")).unwrap();
        let cached = db.get_cached_mod("111").unwrap().unwrap();
        assert_eq!(cached.title, "NewTitle");
        assert_eq!(cached.author.as_deref(), Some("NewAuthor"));
    }

    #[test]
    fn test_get_cached_mod_missing() {
        let db = Database::open_in_memory().unwrap();
        assert!(db.get_cached_mod("999").unwrap().is_none());
    }

    #[test]
    fn test_mod_profiles_round_trip() {
        let db = Database::open_in_memory().unwrap();
        let ids = vec!["111".to_string(), "222".to_string()];
        let names = vec!["ModA".to_string(), "ModB".to_string()];
        db.save_mod_profile("test_profile", Some("desc"), &ids, &names)
            .unwrap();

        let (loaded_ids, loaded_names) = db.get_mod_profile("test_profile").unwrap().unwrap();
        assert_eq!(loaded_ids, ids);
        assert_eq!(loaded_names, names);
    }

    #[test]
    fn test_list_mod_profiles() {
        let db = Database::open_in_memory().unwrap();
        db.save_mod_profile("beta", None, &[], &[]).unwrap();
        db.save_mod_profile("alpha", None, &[], &[]).unwrap();
        let profiles = db.list_mod_profiles().unwrap();
        assert_eq!(profiles, vec!["alpha", "beta"]);
    }
}
