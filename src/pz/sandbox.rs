use std::fmt;
use std::path::Path;

use anyhow::Result;

/// Comment-preserving editor for SandboxVars.lua.
/// Supports dotted keys for nested tables: `get("Zombies.Speed")` returns the
/// value of `Speed` inside the `Zombies = { ... }` block.
pub struct SandboxEditor {
    lines: Vec<String>,
}

impl SandboxEditor {
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

    /// Find the line index for a (possibly dotted) key.
    /// Tracks nesting depth to resolve `"Table.Key"` to the correct line.
    /// PZ wraps everything in `SandboxVars = { ... }`, so flat keys live at
    /// depth 1 and nested-table keys at depth 2. Uses exact (case-sensitive)
    /// matching because Lua is case-sensitive.
    fn find_line(&self, dotted_key: &str) -> Option<usize> {
        let parts: Vec<&str> = dotted_key.splitn(2, '.').collect();
        let (target_parent, target_leaf) = if parts.len() == 2 {
            (Some(parts[0]), parts[1])
        } else {
            (None, parts[0])
        };

        let mut current_table: Option<String> = None;
        let mut depth = 0u32;

        for (i, line) in self.lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("--") {
                continue;
            }

            if let Some(eq) = trimmed.find('=') {
                let k = trimmed[..eq].trim();
                let v = trimmed[eq + 1..].trim();
                if v.starts_with('{') {
                    depth += 1;
                    // At depth 2 we're entering a named sub-table (e.g. Zombies = {)
                    if depth == 2 {
                        current_table = Some(k.to_string());
                    }
                    continue;
                }
                // Exact case-sensitive match (Lua is case-sensitive)
                let is_match = match target_parent {
                    Some(parent) => {
                        depth == 2 && current_table.as_deref() == Some(parent) && k == target_leaf
                    }
                    None => depth == 1 && k == target_leaf,
                };
                if is_match {
                    return Some(i);
                }
            }

            if trimmed.starts_with('}') && depth > 0 {
                depth -= 1;
                if depth == 1 {
                    current_table = None;
                }
            }
        }
        None
    }

    pub fn get(&self, dotted_key: &str) -> Option<&str> {
        let idx = self.find_line(dotted_key)?;
        let trimmed = self.lines[idx].trim();
        let eq = trimmed.find('=')?;
        let v = trimmed[eq + 1..].trim().trim_end_matches(',').trim();
        Some(v)
    }

    pub fn set(&mut self, dotted_key: &str, value: &str) {
        if let Some(idx) = self.find_line(dotted_key) {
            let line = &self.lines[idx];
            let trimmed = line.trim();
            // find_line only returns indices for lines containing '='
            if let Some(eq) = trimmed.find('=') {
                let original_key = trimmed[..eq].trim();
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                self.lines[idx] = format!("{indent}{original_key} = {value},");
            }
        }
        // If key not found, do nothing — we don't auto-create nested tables
    }
}

impl fmt::Display for SandboxEditor {
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

    const SAMPLE: &str = r#"SandboxVars = {
    -- Zombie options
    ZombieCount = 3,
    Zombies = {
        Speed = 3,
        Strength = 2,
    },
    Loot = 1,
}
"#;

    #[test]
    fn test_get_flat() {
        let s = SandboxEditor::parse(SAMPLE);
        assert_eq!(s.get("ZombieCount"), Some("3"));
    }

    #[test]
    fn test_get_nested() {
        let s = SandboxEditor::parse(SAMPLE);
        assert_eq!(s.get("Zombies.Speed"), Some("3"));
        assert_eq!(s.get("Zombies.Strength"), Some("2"));
    }

    #[test]
    fn test_get_table_returns_none() {
        let s = SandboxEditor::parse(SAMPLE);
        // Requesting a table key (not a leaf) returns None
        assert_eq!(s.get("Zombies"), None);
    }

    #[test]
    fn test_get_missing() {
        let s = SandboxEditor::parse(SAMPLE);
        assert_eq!(s.get("NonExistent"), None);
    }

    #[test]
    fn test_set_flat() {
        let mut s = SandboxEditor::parse(SAMPLE);
        s.set("ZombieCount", "5");
        assert_eq!(s.get("ZombieCount"), Some("5"));
    }

    #[test]
    fn test_set_nested() {
        let mut s = SandboxEditor::parse(SAMPLE);
        s.set("Zombies.Speed", "5");
        assert_eq!(s.get("Zombies.Speed"), Some("5"));
        // Other nested keys unchanged
        assert_eq!(s.get("Zombies.Strength"), Some("2"));
    }

    #[test]
    fn test_comments_preserved() {
        let mut s = SandboxEditor::parse(SAMPLE);
        s.set("Loot", "3");
        assert!(s.to_string().contains("-- Zombie options"));
    }

    #[test]
    fn test_round_trip_unchanged() {
        let s = SandboxEditor::parse(SAMPLE);
        assert_eq!(s.to_string(), SAMPLE);
    }
}
