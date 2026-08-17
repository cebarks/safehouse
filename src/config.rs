use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

fn default_web_bind() -> String {
    "0.0.0.0".to_string()
}
fn default_web_port() -> u16 {
    9292
}
fn default_rcon_port() -> u16 {
    27015
}
fn default_backup_retention_days() -> u32 {
    7
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafehouseConfig {
    /// Path to the PZ server install directory (contains ProjectZomboid64)
    pub server_install_dir: PathBuf,

    /// Server instance name — matches prefix of files in ~/Zomboid/server/
    #[serde(default = "default_server_name")]
    pub server_name: String,

    /// Path to Zomboid data directory. Defaults to $HOME/Zomboid if absent.
    pub zomboid_data_dir: Option<PathBuf>,

    /// RCON password — must match RCONPassword in server.ini
    #[serde(default)]
    pub rcon_password: String,

    /// RCON port — must match RCONPort in server.ini
    #[serde(default = "default_rcon_port")]
    pub rcon_port: u16,

    /// Web UI bind address
    #[serde(default = "default_web_bind")]
    pub web_bind: String,

    /// Web UI port
    #[serde(default = "default_web_port")]
    pub web_port: u16,

    /// Discord webhook URL for event notifications
    pub discord_webhook_url: Option<String>,

    /// Number of days to retain backup snapshots
    #[serde(default = "default_backup_retention_days")]
    pub backup_retention_days: u32,

    /// Send RCON save before taking a snapshot
    #[serde(default = "default_true")]
    pub backup_rcon_save: bool,

    /// Steam Workshop collection ID to sync mods from
    pub steam_collection_id: Option<String>,

    /// Session secret for web UI cookie signing (auto-generated on first serve)
    #[serde(default)]
    pub session_secret: String,
}

fn default_server_name() -> String {
    "servertest".to_string()
}

impl Default for SafehouseConfig {
    fn default() -> Self {
        Self {
            server_install_dir: PathBuf::from("/opt/pzserver"),
            server_name: default_server_name(),
            zomboid_data_dir: None,
            rcon_password: String::new(),
            rcon_port: default_rcon_port(),
            web_bind: default_web_bind(),
            web_port: default_web_port(),
            discord_webhook_url: None,
            backup_retention_days: default_backup_retention_days(),
            backup_rcon_save: true,
            steam_collection_id: None,
            session_secret: String::new(),
        }
    }
}

impl SafehouseConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read config at {}", path.display()))?;
        toml::from_str(&content).context("invalid safehouse.toml")
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self).context("failed to serialize config")?;
        std::fs::write(path, content)
            .with_context(|| format!("cannot write config to {}", path.display()))
    }

    /// Resolve the Zomboid data directory ($HOME/Zomboid by default).
    pub fn zomboid_dir(&self) -> PathBuf {
        self.zomboid_data_dir.clone().unwrap_or_else(|| {
            dirs_next::home_dir()
                .unwrap_or_else(|| PathBuf::from("/root"))
                .join("Zomboid")
        })
    }

    pub fn ensure_session_secret(&mut self) {
        if self.session_secret.is_empty() {
            use rand::RngCore;
            let mut bytes = [0u8; 64];
            rand::rngs::OsRng.fill_bytes(&mut bytes);
            self.session_secret = hex::encode(bytes);
        }
    }

    /// Decode the hex session secret into raw bytes for cookie signing.
    /// Panics if the secret is invalid hex (setup ensures this never happens).
    #[allow(clippy::unwrap_used)] // deliberate panic on invariant violation
    pub fn session_key_bytes(&self) -> Vec<u8> {
        hex::decode(&self.session_secret)
            .expect("session_secret must be valid hex — run `safehouse setup` to regenerate")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_defaults() {
        let cfg = SafehouseConfig::default();
        assert_eq!(cfg.web_port, 9292);
        assert_eq!(cfg.rcon_port, 27015);
        assert_eq!(cfg.backup_retention_days, 7);
        assert!(cfg.rcon_password.is_empty());
    }

    #[test]
    fn test_round_trip_toml() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("safehouse.toml");
        let mut cfg = SafehouseConfig::default();
        cfg.server_name = "testworld".to_string();
        cfg.save(&path).unwrap();
        let loaded = SafehouseConfig::load(&path).unwrap();
        assert_eq!(loaded.server_name, "testworld");
    }
}
