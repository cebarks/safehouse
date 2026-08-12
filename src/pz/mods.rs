use crate::pz::ini::IniEditor;

/// Add a mod to both WorkshopItems= and Mods= lines. Idempotent.
pub fn add_mod_to_ini(ini: &mut IniEditor, workshop_id: &str, mod_folder_name: &str) {
    ini.add_workshop_id(workshop_id);
    ini.add_mod_name(mod_folder_name);
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_add_mod_updates_both_lists() {
        let content = "WorkshopItems=\nMods=\n";
        let mut ini = IniEditor::parse(content);
        add_mod_to_ini(&mut ini, "2392987220", "BritasWeaponPack");
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
        add_mod_to_ini(&mut ini, "2392987220", "BritasWeaponPack");
        assert_eq!(ini.workshop_ids().len(), 1);
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
}
