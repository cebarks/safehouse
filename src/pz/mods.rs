use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::pz::ini::IniEditor;
use crate::validate::{validate_mod_folder_name, validate_workshop_id};

/// Add a mod to both WorkshopItems= and Mods= lines. Idempotent.
///
/// Both fields are validated first: they're joined with `;` and written
/// into a `key=value` line in `server.ini`, so an unvalidated ID or folder
/// name could inject an extra `;`-separated entry, corrupt the line with a
/// stray `=`, or smuggle an unrelated key via an embedded newline.
pub fn add_mod_to_ini(ini: &mut IniEditor, workshop_id: &str, mod_folder_name: &str) -> Result<()> {
    validate_workshop_id(workshop_id)?;
    validate_mod_folder_name(mod_folder_name)?;
    ini.add_workshop_id(workshop_id);
    ini.add_mod_name(mod_folder_name);
    Ok(())
}

/// Remove a mod from both lists.
pub fn remove_mod_from_ini(ini: &mut IniEditor, workshop_id: &str, mod_folder_name: &str) {
    ini.remove_workshop_id(workshop_id);
    ini.remove_mod_name(mod_folder_name);
}

/// Return a paired list of (workshop_id, mod_folder_name) from the INI.
/// The lists must be the same length and in the same order.
pub fn list_mods(ini: &IniEditor) -> Vec<(String, String)> {
    ini.workshop_ids()
        .into_iter()
        .zip(ini.mod_names())
        .collect()
}

/// Scan the Steam Workshop download directory for mod.info files.
/// Returns a map of workshop_id → Vec<mod_folder_name>.
///
/// PZ Workshop mods are downloaded to:
///   <server_install_dir>/steamapps/workshop/content/108600/<workshop_id>/mods/<name>/mod.info
///
/// Each mod.info contains a line like `id=BritasWeaponPack`.
pub fn scan_workshop_mod_folders(install_dir: &Path) -> HashMap<String, Vec<String>> {
    let workshop_dir = install_dir
        .join("steamapps")
        .join("workshop")
        .join("content")
        .join("108600");
    let mut map: HashMap<String, Vec<String>> = HashMap::new();

    let entries = match std::fs::read_dir(&workshop_dir) {
        Ok(e) => e,
        Err(_) => return map,
    };

    for entry in entries.flatten() {
        let workshop_id = entry.file_name().to_string_lossy().to_string();
        if !workshop_id.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let mods_dir = entry.path().join("mods");
        if let Ok(mod_entries) = std::fs::read_dir(&mods_dir) {
            for mod_entry in mod_entries.flatten() {
                let mod_info_path = mod_entry.path().join("mod.info");
                if let Some(mod_id) = parse_mod_info_id(&mod_info_path) {
                    map.entry(workshop_id.clone()).or_default().push(mod_id);
                }
            }
        }
    }
    map
}

/// Parse the `id=` field from a PZ mod.info file.
fn parse_mod_info_id(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("id=") {
            let id = value.trim();
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}

/// Result of a collection sync operation.
#[derive(Debug)]
pub struct SyncResult {
    /// Mods added to server.ini.
    pub added: Vec<(String, Vec<String>)>,
    /// Mods removed from server.ini.
    pub removed: Vec<(String, String)>,
    /// Workshop IDs where the mod folder name couldn't be discovered
    /// (not yet downloaded — will appear after a server restart).
    pub pending: Vec<String>,
    /// Total mods now in server.ini.
    pub total: usize,
}

/// Sync server.ini mod lists to match the given collection workshop IDs.
///
/// For each workshop ID in `collection_ids`:
///   - If already in server.ini → keep
///   - If not in server.ini → add (using scanned mod folder names)
///
/// Any mod in server.ini NOT in `collection_ids` → remove.
///
/// `known_folders` maps workshop_id → mod folder names (from scan or DB).
pub fn sync_mods_to_collection(
    ini: &mut IniEditor,
    collection_ids: &[String],
    known_folders: &HashMap<String, Vec<String>>,
) -> SyncResult {
    let current_ids = ini.workshop_ids();
    let current_names = ini.mod_names();
    let current_pairs: Vec<(String, String)> = current_ids.into_iter().zip(current_names).collect();

    let mut new_ids: Vec<String> = Vec::new();
    let mut new_names: Vec<String> = Vec::new();
    let mut added: Vec<(String, Vec<String>)> = Vec::new();
    let mut pending: Vec<String> = Vec::new();

    for cid in collection_ids {
        if let Some((_, existing_name)) = current_pairs.iter().find(|(id, _)| id == cid) {
            // Already present — keep as-is
            new_ids.push(cid.clone());
            new_names.push(existing_name.clone());
        } else if let Some(folders) = known_folders.get(cid.as_str()) {
            // New mod with discovered folder names
            for folder in folders {
                new_ids.push(cid.clone());
                new_names.push(folder.clone());
            }
            added.push((cid.clone(), folders.clone()));
        } else {
            // New mod but no folder name discovered yet
            // Still add to WorkshopItems so SteamCMD downloads it
            new_ids.push(cid.clone());
            new_names.push(String::new()); // placeholder
            pending.push(cid.clone());
        }
    }

    // Track removed mods
    let removed: Vec<(String, String)> = current_pairs
        .into_iter()
        .filter(|(id, _)| !collection_ids.contains(id))
        .collect();

    // Clean out empty placeholder names before writing
    let (final_ids, final_names): (Vec<String>, Vec<String>) = new_ids
        .into_iter()
        .zip(new_names)
        .filter(|(_, name)| !name.is_empty())
        .unzip();

    // Also set WorkshopItems to include pending (even without mod names)
    let mut all_workshop_ids = final_ids.clone();
    for pid in &pending {
        if !all_workshop_ids.contains(pid) {
            all_workshop_ids.push(pid.clone());
        }
    }

    let total = final_ids.len();
    ini.set_workshop_ids(&all_workshop_ids);
    ini.set_mod_names(&final_names);

    SyncResult {
        added,
        removed,
        pending,
        total,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_add_mod_updates_both_lists() {
        let content = "WorkshopItems=\nMods=\n";
        let mut ini = IniEditor::parse(content);
        add_mod_to_ini(&mut ini, "2392987220", "BritasWeaponPack").unwrap();
        assert!(ini.workshop_ids().contains(&"2392987220".to_string()));
        assert!(ini.mod_names().contains(&"BritasWeaponPack".to_string()));
    }

    #[test]
    fn test_remove_mod_removes_from_both_lists() {
        let content = "WorkshopItems=2392987220;999\nMods=BritasWeaponPack;OtherMod\n";
        let mut ini = IniEditor::parse(content);
        remove_mod_from_ini(&mut ini, "2392987220", "BritasWeaponPack");
        assert!(!ini.workshop_ids().contains(&"2392987220".to_string()));
        assert!(ini.workshop_ids().contains(&"999".to_string()));
    }

    #[test]
    fn test_no_duplicate_add() {
        let content = "WorkshopItems=2392987220\nMods=BritasWeaponPack\n";
        let mut ini = IniEditor::parse(content);
        add_mod_to_ini(&mut ini, "2392987220", "BritasWeaponPack").unwrap();
        assert_eq!(ini.workshop_ids().len(), 1);
    }

    #[test]
    fn test_add_mod_rejects_invalid_workshop_id() {
        let mut ini = IniEditor::parse("WorkshopItems=\nMods=\n");
        assert!(add_mod_to_ini(&mut ini, "123;RCONPassword=hacked", "Mod").is_err());
        assert!(ini.workshop_ids().is_empty());
    }

    #[test]
    fn test_add_mod_rejects_invalid_folder_name() {
        let mut ini = IniEditor::parse("WorkshopItems=\nMods=\n");
        assert!(add_mod_to_ini(&mut ini, "123", "Evil\nRCONPassword=hacked").is_err());
        assert!(ini.mod_names().is_empty());
    }

    #[test]
    fn test_list_mods_zips_correctly() {
        let content = "WorkshopItems=111;222\nMods=ModA;ModB\n";
        let ini = IniEditor::parse(content);
        let mods = list_mods(&ini);
        assert_eq!(
            mods,
            vec![
                ("111".to_string(), "ModA".to_string()),
                ("222".to_string(), "ModB".to_string()),
            ]
        );
    }

    // --- sync_mods_to_collection tests ---

    #[test]
    fn test_sync_adds_new_mods_with_known_folders() {
        let mut ini = IniEditor::parse("WorkshopItems=\nMods=\n");
        let collection = vec!["111".to_string(), "222".to_string()];
        let mut known = HashMap::new();
        known.insert("111".to_string(), vec!["ModA".to_string()]);
        known.insert("222".to_string(), vec!["ModB".to_string()]);
        let result = sync_mods_to_collection(&mut ini, &collection, &known);
        assert_eq!(result.added.len(), 2);
        assert_eq!(result.removed.len(), 0);
        assert_eq!(result.pending.len(), 0);
        assert_eq!(result.total, 2);
        assert_eq!(ini.mod_names(), vec!["ModA", "ModB"]);
    }

    #[test]
    fn test_sync_removes_mods_not_in_collection() {
        let mut ini = IniEditor::parse("WorkshopItems=111;222\nMods=ModA;ModB\n");
        let collection = vec!["111".to_string()]; // 222 removed from collection
        let known = HashMap::new();
        let result = sync_mods_to_collection(&mut ini, &collection, &known);
        assert_eq!(result.removed.len(), 1);
        assert_eq!(result.removed[0].0, "222");
        assert_eq!(ini.mod_names(), vec!["ModA"]);
    }

    #[test]
    fn test_sync_keeps_existing_mods() {
        let mut ini = IniEditor::parse("WorkshopItems=111\nMods=ModA\n");
        let collection = vec!["111".to_string()];
        let known = HashMap::new();
        let result = sync_mods_to_collection(&mut ini, &collection, &known);
        assert_eq!(result.added.len(), 0);
        assert_eq!(result.removed.len(), 0);
        assert_eq!(result.total, 1);
        assert_eq!(ini.mod_names(), vec!["ModA"]);
    }

    #[test]
    fn test_sync_pending_mods_added_to_workshop_ids_only() {
        let mut ini = IniEditor::parse("WorkshopItems=\nMods=\n");
        let collection = vec!["111".to_string(), "222".to_string()];
        let mut known = HashMap::new();
        known.insert("111".to_string(), vec!["ModA".to_string()]);
        // 222 has no known folder name
        let result = sync_mods_to_collection(&mut ini, &collection, &known);
        assert_eq!(result.pending, vec!["222"]);
        assert_eq!(result.total, 1); // only ModA is "active"
        assert!(ini.workshop_ids().contains(&"222".to_string())); // but 222 is in WorkshopItems
        assert_eq!(ini.mod_names(), vec!["ModA"]); // not in Mods
    }

    #[test]
    fn test_sync_handles_multi_mod_packs() {
        let mut ini = IniEditor::parse("WorkshopItems=\nMods=\n");
        let collection = vec!["111".to_string()];
        let mut known = HashMap::new();
        known.insert("111".to_string(), vec!["SubModA".to_string(), "SubModB".to_string()]);
        let result = sync_mods_to_collection(&mut ini, &collection, &known);
        assert_eq!(result.added.len(), 1);
        assert_eq!(result.added[0].1, vec!["SubModA", "SubModB"]);
        assert_eq!(ini.mod_names(), vec!["SubModA", "SubModB"]);
    }

    #[test]
    fn test_sync_idempotent() {
        let mut ini = IniEditor::parse("WorkshopItems=111;222\nMods=ModA;ModB\n");
        let collection = vec!["111".to_string(), "222".to_string()];
        let known = HashMap::new();
        let result = sync_mods_to_collection(&mut ini, &collection, &known);
        assert_eq!(result.added.len(), 0);
        assert_eq!(result.removed.len(), 0);
        assert_eq!(ini.workshop_ids(), vec!["111", "222"]);
        assert_eq!(ini.mod_names(), vec!["ModA", "ModB"]);
    }

    // --- parse_mod_info_id tests ---

    #[test]
    fn test_parse_mod_info_id_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("mod.info");
        std::fs::write(&path, "name=Brita's Weapon Pack\nid=BritasWeaponPack\n").unwrap();
        assert_eq!(parse_mod_info_id(&path), Some("BritasWeaponPack".to_string()));
    }

    #[test]
    fn test_parse_mod_info_id_with_spaces() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("mod.info");
        std::fs::write(&path, "id= MyMod \nname=My Mod\n").unwrap();
        assert_eq!(parse_mod_info_id(&path), Some("MyMod".to_string()));
    }

    #[test]
    fn test_parse_mod_info_id_missing_id_line() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("mod.info");
        std::fs::write(&path, "name=SomeMod\nversion=1.0\n").unwrap();
        assert_eq!(parse_mod_info_id(&path), None);
    }

    #[test]
    fn test_parse_mod_info_id_missing_file() {
        let path = std::path::PathBuf::from("/nonexistent/mod.info");
        assert_eq!(parse_mod_info_id(&path), None);
    }

    #[test]
    fn test_parse_mod_info_id_empty_value() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("mod.info");
        std::fs::write(&path, "id=\nname=SomeMod\n").unwrap();
        assert_eq!(parse_mod_info_id(&path), None);
    }

    // --- scan_workshop_mod_folders tests ---

    #[test]
    fn test_scan_workshop_mod_folders_discovers_mods() {
        let tmp = tempfile::tempdir().unwrap();
        let mod_dir = tmp.path()
            .join("steamapps/workshop/content/108600/12345/mods/TestMod");
        std::fs::create_dir_all(&mod_dir).unwrap();
        std::fs::write(mod_dir.join("mod.info"), "id=TestMod\n").unwrap();
        let result = scan_workshop_mod_folders(tmp.path());
        assert_eq!(result.get("12345").unwrap(), &vec!["TestMod".to_string()]);
    }

    #[test]
    fn test_scan_workshop_mod_folders_missing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let result = scan_workshop_mod_folders(tmp.path());
        assert!(result.is_empty());
    }
}
