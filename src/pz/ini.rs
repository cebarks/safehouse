use std::fmt;
use std::path::Path;

use anyhow::Result;

/// A comment-preserving INI editor for Project Zomboid server.ini files.
/// The format is a flat key=value file with an optional [ServerConfig] section header.
pub struct IniEditor {
    lines: Vec<String>,
}

impl IniEditor {
    pub fn parse(content: &str) -> Self {
        Self {
            lines: content.lines().map(str::to_owned).collect(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::parse(&content))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.to_string())?;
        Ok(())
    }

    /// Get the value for a key, or None if not present.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.lines.iter().find_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.starts_with(';') {
                return None;
            }
            let (k, v) = trimmed.split_once('=')?;
            if k.trim().eq_ignore_ascii_case(key) {
                Some(v.trim())
            } else {
                None
            }
        })
    }

    /// Set a key to a new value. Adds the key if not present.
    pub fn set(&mut self, key: &str, value: &str) {
        for line in &mut self.lines {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.starts_with(';') {
                continue;
            }
            if let Some((k, _)) = trimmed.split_once('=') {
                if k.trim().eq_ignore_ascii_case(key) {
                    let original_key = k.trim();
                    *line = format!("{original_key}={value}");
                    return;
                }
            }
        }
        // Key not found — append
        self.lines.push(format!("{key}={value}"));
    }

    /// Return the WorkshopItems= list as individual IDs.
    pub fn workshop_ids(&self) -> Vec<String> {
        self.get("WorkshopItems")
            .map(|v| {
                v.split(';')
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn set_workshop_ids(&mut self, ids: &[String]) {
        self.set("WorkshopItems", &ids.join(";"));
    }

    pub fn add_workshop_id(&mut self, id: &str) {
        let mut ids = self.workshop_ids();
        if !ids.iter().any(|x| x == id) {
            ids.push(id.to_owned());
            self.set_workshop_ids(&ids);
        }
    }

    pub fn remove_workshop_id(&mut self, id: &str) {
        let ids: Vec<String> = self
            .workshop_ids()
            .into_iter()
            .filter(|x| x != id)
            .collect();
        self.set_workshop_ids(&ids);
    }

    /// Return the Mods= list as individual folder names.
    pub fn mod_names(&self) -> Vec<String> {
        self.get("Mods")
            .map(|v| {
                v.split(';')
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn set_mod_names(&mut self, names: &[String]) {
        self.set("Mods", &names.join(";"));
    }

    pub fn add_mod_name(&mut self, name: &str) {
        let mut names = self.mod_names();
        if !names.iter().any(|x| x == name) {
            names.push(name.to_owned());
            self.set_mod_names(&names);
        }
    }

    pub fn remove_mod_name(&mut self, name: &str) {
        let names: Vec<String> = self.mod_names().into_iter().filter(|x| x != name).collect();
        self.set_mod_names(&names);
    }
}

impl fmt::Display for IniEditor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = self.lines.join("\n");
        if !out.ends_with('\n') {
            out.push('\n');
        }
        f.write_str(&out)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[ServerConfig]
# This is a comment
ServerName=MyServer
MaxPlayers=20
PVP=false
WorkshopItems=111;222;333
Mods=ModA;ModB;ModC
"#;

    #[test]
    fn test_get_existing_key() {
        let ini = IniEditor::parse(SAMPLE);
        assert_eq!(ini.get("MaxPlayers"), Some("20"));
    }

    #[test]
    fn test_get_missing_key() {
        let ini = IniEditor::parse(SAMPLE);
        assert_eq!(ini.get("NonExistent"), None);
    }

    #[test]
    fn test_set_existing_key() {
        let mut ini = IniEditor::parse(SAMPLE);
        ini.set("MaxPlayers", "32");
        assert_eq!(ini.get("MaxPlayers"), Some("32"));
    }

    #[test]
    fn test_set_preserves_comments() {
        let mut ini = IniEditor::parse(SAMPLE);
        ini.set("MaxPlayers", "32");
        let out = ini.to_string();
        assert!(out.contains("# This is a comment"), "comment was stripped");
    }

    #[test]
    fn test_round_trip_identical_when_unchanged() {
        let ini = IniEditor::parse(SAMPLE);
        assert_eq!(ini.to_string(), SAMPLE);
    }

    #[test]
    fn test_workshop_ids() {
        let ini = IniEditor::parse(SAMPLE);
        assert_eq!(ini.workshop_ids(), vec!["111", "222", "333"]);
    }

    #[test]
    fn test_add_workshop_id() {
        let mut ini = IniEditor::parse(SAMPLE);
        ini.add_workshop_id("444");
        assert!(ini.workshop_ids().contains(&"444".to_string()));
    }

    #[test]
    fn test_remove_workshop_id() {
        let mut ini = IniEditor::parse(SAMPLE);
        ini.remove_workshop_id("222");
        assert!(!ini.workshop_ids().contains(&"222".to_string()));
        assert!(ini.workshop_ids().contains(&"111".to_string()));
    }

    #[test]
    fn test_mod_names() {
        let ini = IniEditor::parse(SAMPLE);
        assert_eq!(ini.mod_names(), vec!["ModA", "ModB", "ModC"]);
    }
}
