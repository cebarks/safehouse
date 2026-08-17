//! Input validation for values that flow from the web UI (or mod metadata)
//! into files safehouse writes on disk: `server.ini`, `SandboxVars.lua`, and
//! backup archive filenames.
//!
//! These files are line-oriented and, in the case of `SandboxVars.lua`,
//! interpreted as Lua by the PZ server on boot. Without validation, a
//! crafted value can smuggle a newline (injecting an unrelated key such as
//! `RCONPassword=...`) or, worse, break out of a Lua assignment and inject
//! arbitrary Lua statements. All validation here is deliberately
//! conservative: reject anything that isn't obviously safe rather than try
//! to enumerate every dangerous pattern.

use anyhow::{bail, Result};

fn reject_control_chars(s: &str, field: &str) -> Result<()> {
    if s.contains(['\n', '\r', '\0']) {
        bail!("{field} must not contain newlines or control characters");
    }
    Ok(())
}

/// Validate a `server.ini` key. Keys are matched case-insensitively against
/// `key = value` lines, so a key containing '=' or a newline could corrupt
/// the file structure or smuggle an unrelated key.
pub fn validate_ini_key(key: &str) -> Result<()> {
    if key.trim().is_empty() {
        bail!("config key must not be empty");
    }
    reject_control_chars(key, "config key")?;
    if key.contains('=') {
        bail!("config key must not contain '='");
    }
    Ok(())
}

/// Validate a `server.ini` value. A newline in the value would be written
/// verbatim into the file and parsed back as one or more additional
/// `key=value` lines, letting a crafted value silently overwrite unrelated
/// keys (e.g. `RCONPassword`).
pub fn validate_ini_value(value: &str) -> Result<()> {
    reject_control_chars(value, "config value")
}

/// Validate a `SandboxVars.lua` dotted key path (e.g. `Zombies.Speed`).
pub fn validate_lua_key(key: &str) -> Result<()> {
    if key.trim().is_empty() {
        bail!("sandbox key must not be empty");
    }
    if !key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
    {
        bail!("sandbox key may only contain letters, digits, '.', and '_'");
    }
    Ok(())
}

/// Validate a `SandboxVars.lua` value. PZ sandbox values are always one of:
/// an integer, a float, a boolean, or a simple double-quoted string
/// literal. `SandboxEditor::set` writes the value verbatim into a Lua table
/// literal that the PZ server `loadstring`s at boot, so anything outside
/// this grammar (e.g. `3, }; os.execute("...")`) would be interpreted as
/// Lua and executed with the server process's privileges.
pub fn validate_lua_value(value: &str) -> Result<()> {
    let v = value.trim();
    if v.is_empty() {
        bail!("sandbox value must not be empty");
    }
    if v == "true" || v == "false" {
        return Ok(());
    }
    if v.parse::<f64>().is_ok() {
        return Ok(());
    }
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
        let inner = &v[1..v.len() - 1];
        if !inner.contains(['"', '\\', '\n', '\r']) {
            return Ok(());
        }
    }
    bail!(
        "sandbox value must be a number, true/false, or a simple double-quoted \
         string with no embedded quotes (got: {value:?})"
    );
}

/// Validate a Steam Workshop item ID. Published file IDs are always decimal
/// digit strings; anything else can't be a real Workshop ID and, if
/// written unchecked into the `WorkshopItems=` list, could inject a `;`-
/// separated extra entry, an `=` that corrupts the line, or a newline that
/// smuggles another INI key.
pub fn validate_workshop_id(id: &str) -> Result<()> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        bail!("workshop ID must contain only digits (got: {id:?})");
    }
    Ok(())
}

/// Validate a mod folder name as written into the `Mods=` list in
/// `server.ini`. Mod folder names are filesystem directory names chosen by
/// Workshop mod authors. Real PZ mods use characters like `()`, `&`, `+`,
/// spaces, dots, and brackets in their folder names. We allow those but
/// reject the three characters that would corrupt the INI format:
///   - `;` — would inject an extra mod entry
///   - `=` — would corrupt the key=value line
///   - newlines/control chars — would smuggle another INI key
pub fn validate_mod_folder_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("mod folder name must not be empty");
    }
    reject_control_chars(name, "mod folder name")?;
    if name.contains(';') {
        bail!("mod folder name must not contain ';' (got: {name:?})");
    }
    if name.contains('=') {
        bail!("mod folder name must not contain '=' (got: {name:?})");
    }
    Ok(())
}

/// Validate a backup label used to build a filename on disk
/// (`{server}_{timestamp}_{label}.tar.gz`). Must not contain path
/// separators or `..` so a crafted label can't write the backup archive
/// outside the backups directory.
pub fn validate_backup_label(label: &str) -> Result<()> {
    if label.is_empty() {
        bail!("backup label must not be empty");
    }
    if !label
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        bail!("backup label may only contain letters, digits, '_', '-', and '.' (got: {label:?})");
    }
    if label.contains("..") {
        bail!("backup label must not contain '..'");
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_ini_key_rejects_newline() {
        assert!(validate_ini_key("Foo\nRCONPassword").is_err());
    }

    #[test]
    fn test_ini_key_rejects_equals() {
        assert!(validate_ini_key("Foo=Bar").is_err());
    }

    #[test]
    fn test_ini_key_rejects_empty() {
        assert!(validate_ini_key("").is_err());
        assert!(validate_ini_key("   ").is_err());
    }

    #[test]
    fn test_ini_key_accepts_normal_key() {
        assert!(validate_ini_key("MaxPlayers").is_ok());
    }

    #[test]
    fn test_ini_value_rejects_newline_injection() {
        assert!(validate_ini_value("20\nRCONPassword=owned").is_err());
    }

    #[test]
    fn test_ini_value_accepts_normal_value() {
        assert!(validate_ini_value("20").is_ok());
        assert!(validate_ini_value("a strong password!").is_ok());
    }

    #[test]
    fn test_lua_value_accepts_number() {
        assert!(validate_lua_value("3").is_ok());
        assert!(validate_lua_value("3.5").is_ok());
        assert!(validate_lua_value("-1").is_ok());
    }

    #[test]
    fn test_lua_value_accepts_bool() {
        assert!(validate_lua_value("true").is_ok());
        assert!(validate_lua_value("false").is_ok());
    }

    #[test]
    fn test_lua_value_accepts_simple_string() {
        assert!(validate_lua_value("\"hello world\"").is_ok());
    }

    #[test]
    fn test_lua_value_rejects_injection() {
        assert!(validate_lua_value("3, }; os.execute(\"evil\"); local x = {").is_err());
        assert!(validate_lua_value("os.execute(\"evil\")").is_err());
    }

    #[test]
    fn test_lua_value_rejects_string_breakout() {
        assert!(validate_lua_value("\"a\" .. os.execute(\"evil\") .. \"b\"").is_err());
    }

    #[test]
    fn test_lua_value_rejects_empty() {
        assert!(validate_lua_value("").is_err());
    }

    #[test]
    fn test_lua_key_accepts_dotted() {
        assert!(validate_lua_key("Zombies.Speed").is_ok());
    }

    #[test]
    fn test_lua_key_rejects_bad_chars() {
        assert!(validate_lua_key("Zombies.Speed}; os.execute()").is_err());
    }

    #[test]
    fn test_workshop_id_accepts_digits() {
        assert!(validate_workshop_id("2392987220").is_ok());
    }

    #[test]
    fn test_workshop_id_rejects_non_numeric() {
        assert!(validate_workshop_id("123;RCONPassword=hacked").is_err());
        assert!(validate_workshop_id("").is_err());
        assert!(validate_workshop_id("abc").is_err());
    }

    #[test]
    fn test_mod_folder_name_accepts_safe_chars() {
        assert!(validate_mod_folder_name("BritasWeaponPack").is_ok());
        assert!(validate_mod_folder_name("Mod-Name_2").is_ok());
    }

    #[test]
    fn test_mod_folder_name_accepts_real_pz_names() {
        assert!(validate_mod_folder_name("Arsenal(26)GunFighter").is_ok());
        assert!(validate_mod_folder_name("ScrapWeapons(new version)").is_ok());
        assert!(validate_mod_folder_name("Literature&Magazines").is_ok());
        assert!(validate_mod_folder_name("Authentic Z - Current").is_ok());
        assert!(validate_mod_folder_name("AuthenticZBackpacks+").is_ok());
        assert!(validate_mod_folder_name("LitSortOGSN_chocolate").is_ok());
    }

    #[test]
    fn test_mod_folder_name_rejects_dangerous_chars() {
        assert!(validate_mod_folder_name("Evil;Other").is_err());
        assert!(validate_mod_folder_name("Evil\nRCONPassword=owned").is_err());
        assert!(validate_mod_folder_name("Evil=Owned").is_err());
        assert!(validate_mod_folder_name("").is_err());
    }

    #[test]
    fn test_backup_label_accepts_safe_chars() {
        assert!(validate_backup_label("pre-wipe").is_ok());
        assert!(validate_backup_label("nightly_2024.01.01").is_ok());
    }

    #[test]
    fn test_backup_label_rejects_traversal() {
        assert!(validate_backup_label("../../../../tmp/pwned").is_err());
        assert!(validate_backup_label("a/b").is_err());
        assert!(validate_backup_label("..").is_err());
        assert!(validate_backup_label("").is_err());
    }
}
