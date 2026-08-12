# Safehouse Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task.

**Goal:** Build `safehouse`, a single-binary Project Zomboid dedicated server manager with CLI and embedded web UI, following the quartermaster/quma architecture pattern.

**Architecture:** One Rust binary (`safehouse`) combining a clap-based CLI with an embedded actix-web HTTP server, askama HTML templates, and a rusqlite SQLite database. Server management targets a native Linux PZ process (no containers) via PID-file tracking, tokio process spawning, and a custom Source RCON TCP client for live admin commands. Static assets are baked into the binary via rust-embed; templates compile at build time via askama.

**Tech Stack:** Rust 2021 · clap 4 (derive) · actix-web 4 · askama 0.12 · rusqlite 0.32 (bundled) · tokio 1 (full) · reqwest 0.12 (rustls-tls) · argon2 0.5 · flate2 + tar 0.4 (backups) · parking_lot 0.12 · tracing + tracing-subscriber 0.3 · HTMX 2 (web interactivity) · rust-embed 8 · dirs-next 2 · hex 0.4

---

## Reference: Key Project Zomboid Server Facts

- **Install:** SteamCMD `app_update 380870 validate` (anonymous login, free dedicated server)
- **Binary:** `<install_dir>/ProjectZomboid64` on Linux
- **Config files** (all in `$HOME/Zomboid/server/<name>.ini`, etc.):
  - `<name>.ini` — networking, players, PvP, passwords, mod lists
  - `<name>_SandboxVars.lua` — zombie behavior, loot, XP, weather
  - `<name>_spawnpoints.lua` — player spawn coordinates
  - `<name>_spawnregions.lua` — which map regions are enabled
- **Mod format in .ini:** `WorkshopItems=id1;id2;id3` and `Mods=FolderName1;FolderName2`
  (two synchronized semicolon-delimited lists — Workshop IDs + internal mod folder names)
- **RCON:** Source RCON protocol (TCP), port in `RCONPort=` in server.ini
- **World saves:** `$HOME/Zomboid/Saves/<server_name>/`
- **Logs:** `$HOME/Zomboid/server/<name>_[timestamp].txt` (latest via glob)
- **Startup:** `./ProjectZomboid64 -servername <name> -adminpassword <pass> -rconpassword <pass>`

---

## Task 1: Project Scaffold

**Files:**

- Create: `Cargo.toml`
- Create: `build.rs`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `.gitignore`

**Step 1: Write `Cargo.toml`**

```toml
[package]
name = "safehouse"
version = "0.1.0"
edition = "2021"
description = "CLI and web UI for managing a Project Zomboid dedicated server"
license = "AGPL-3.0-only"

[[bin]]
name = "safehouse"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
rusqlite = { version = "0.32", features = ["bundled"] }
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
tokio = { version = "1", features = ["full"] }
anyhow = "1"
chrono = { version = "0.4", features = ["serde"] }
actix-web = "4"
actix-session = { version = "0.10", features = ["cookie-session"] }
askama = "0.12"
rust-embed = "8"
argon2 = "0.5"
rpassword = "7"
flate2 = "1"
tar = "0.4"
parking_lot = "0.12"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
regex = "1"
rand = "0.8"
dirs-next = "2"
hex = "0.4"
glob = "0.3"
libc = "0.2"

[dev-dependencies]
tempfile = "3"

[profile.release]
strip = true
codegen-units = 1
```

**Step 2: Write `build.rs`**

```rust
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");

    let version = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().trim_start_matches('v').to_string())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

    println!("cargo:rustc-env=SAFEHOUSE_VERSION={version}");
}
```

**Step 3: Write `src/lib.rs`** (empty for now — module declarations added in Task 2 after stubs exist)

```rust
#![deny(clippy::unwrap_used)]
```

> **Convention:** `#![deny(clippy::unwrap_used)]` forbids `.unwrap()` and `.expect()` in production code.
>
> - All `#[cfg(test)]` modules must add `#[allow(clippy::unwrap_used)]` at the top.
> - Functions that deliberately panic on invariant violations (static regex compilation, session secret decode, signal registration) must add `#[allow(clippy::unwrap_used)]` on the function.
>
> ```rust
> #[cfg(test)]
> #[allow(clippy::unwrap_used)]
> mod tests { ... }
>
> #[allow(clippy::unwrap_used)] // static regex, always valid
> pub fn parse_log_line(line: &str) -> Option<PlayerEvent> { ... }
> ```

**Step 4: Write `src/main.rs`** (minimal — just proves it compiles)

```rust
fn main() {
    println!("safehouse {}", env!("SAFEHOUSE_VERSION"));
}
```

**Step 5: Write `.gitignore`**

```
/target
*.db
*.db-wal
*.db-shm
/dist
```

**Step 6: Verify it compiles**

```bash
cargo build 2>&1 | head -30
```

Expected: `Finished dev` (will warn about unused modules — that's fine)

**Step 7: Commit**

```bash
git add -A
git commit -m "chore: project scaffold — Cargo.toml, main.rs, lib.rs, build.rs"
```

---

## Task 2: Config + Dirs Modules

**Files:**

- Create: `src/config.rs`
- Create: `src/dirs.rs`
- Create stub files: `src/backup.rs`, `src/notify.rs`, `src/pz/mod.rs`, `src/steam/mod.rs`, `src/web/mod.rs`, `src/db/mod.rs`, `src/cli/mod.rs`
- Modify: `src/lib.rs` (add module declarations now that stubs exist)

**Step 1: Write the failing test**

Add to bottom of `src/config.rs` (file doesn't exist yet — test acts as spec):

```rust
// write src/config.rs with this test first, impl below
#[cfg(test)]
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
```

**Step 2: Run failing test**

```bash
cargo test test_defaults 2>&1 | tail -5
```

Expected: FAIL — `SafehouseConfig` not defined

**Step 3: Implement `src/config.rs`**

```rust
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

fn default_web_bind() -> String { "0.0.0.0".to_string() }
fn default_web_port() -> u16 { 9292 }
fn default_rcon_port() -> u16 { 27015 }
fn default_backup_retention_days() -> u32 { 7 }
fn default_true() -> bool { true }

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

    /// Session secret for web UI cookie signing (auto-generated on first serve)
    #[serde(default)]
    pub session_secret: String,
}

fn default_server_name() -> String { "servertest".to_string() }

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
```

> **Note:** `dirs-next` and `hex` are already declared in `Cargo.toml` (Task 1).

**Step 4: Implement `src/dirs.rs`**

```rust
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::config::SafehouseConfig;

/// All disk locations safehouse owns or reads.
#[derive(Debug, Clone)]
pub struct SafehouseDirs {
    /// Root safehouse data dir (where safehouse.toml and safehouse.db live)
    pub root: PathBuf,
}

impl SafehouseDirs {
    pub fn from_root(root: PathBuf) -> Self {
        Self { root }
    }

    /// Auto-detect: explicit arg → SAFEHOUSE_DIR env → ~/.local/share/safehouse
    pub fn detect(explicit: Option<&Path>) -> Result<Self> {
        if let Some(p) = explicit {
            return Ok(Self::from_root(p.to_path_buf()));
        }
        if let Ok(env) = std::env::var("SAFEHOUSE_DIR") {
            return Ok(Self::from_root(PathBuf::from(env)));
        }
        let default = dirs_next::data_local_dir()
            .context("cannot resolve local data dir")?
            .join("safehouse");
        Ok(Self::from_root(default))
    }

    pub fn config_path(&self) -> PathBuf { self.root.join("safehouse.toml") }
    pub fn db_path(&self)     -> PathBuf { self.root.join("safehouse.db") }
    pub fn backups_dir(&self) -> PathBuf { self.root.join("backups") }
    pub fn run_dir(&self)     -> PathBuf { self.root.join("run") }
    pub fn pid_file(&self)    -> PathBuf { self.run_dir().join("server.pid") }
    pub fn log_dir(&self)     -> PathBuf { self.root.join("logs") }

    pub fn ensure_dirs(&self) -> Result<()> {
        for dir in [&self.root, &self.backups_dir(), &self.run_dir(), &self.log_dir()] {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("cannot create directory {}", dir.display()))?;
        }
        Ok(())
    }

    /// Resolve the server config file path: <zomboid_dir>/server/<name>.ini
    pub fn server_ini(&self, config: &SafehouseConfig) -> PathBuf {
        config.zomboid_dir().join("server").join(format!("{}.ini", config.server_name))
    }

    pub fn sandbox_lua(&self, config: &SafehouseConfig) -> PathBuf {
        config.zomboid_dir().join("server").join(format!("{}_SandboxVars.lua", config.server_name))
    }

    pub fn saves_dir(&self, config: &SafehouseConfig) -> PathBuf {
        config.zomboid_dir().join("Saves").join("Multiplayer").join(&config.server_name)
    }

    /// Find the most recent PZ log file for this server.
    pub fn latest_log(&self, config: &SafehouseConfig) -> Option<PathBuf> {
        let pattern = config.zomboid_dir()
            .join("server")
            .join(format!("{}_*.txt", config.server_name))
            .to_string_lossy()
            .to_string();
        glob::glob(&pattern).ok()?.flatten().max_by_key(|p| {
            p.metadata().ok().and_then(|m| m.modified().ok())
        })
    }
}
```

**Step 5: Add stub files and populate `lib.rs`** (so the crate compiles)

```bash
# Create all module stubs
for f in src/backup.rs src/notify.rs; do echo "// TODO" > $f; done
mkdir -p src/logging src/pz src/steam src/web src/db src/cli
for f in src/pz/mod.rs src/steam/mod.rs src/web/mod.rs src/db/mod.rs src/cli/mod.rs; do
  echo "// TODO" > $f
done
```

Now add module declarations to `src/lib.rs` (stubs exist, so this compiles):

```rust
#![deny(clippy::unwrap_used)]

pub mod backup;
pub mod cli;
pub mod config;
pub mod db;
pub mod dirs;
pub mod logging;
pub mod notify;
pub mod pz;
pub mod steam;
pub mod web;
```

Write `src/logging/mod.rs` with actual tracing initialization (not a stub):

```rust
use tracing_subscriber::EnvFilter;

/// Initialize tracing. Call once from main before any tracing macros.
pub fn init(verbosity: u8) {
    let filter = match verbosity {
        0 => "safehouse=info",
        1 => "safehouse=debug",
        _ => "safehouse=trace",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(filter)),
        )
        .init();
}
```

**Step 6: Run tests**

```bash
cargo test config:: -- --nocapture
```

Expected: 2 tests pass

**Step 7: Commit**

```bash
git add -A
git commit -m "feat: add config and dirs modules"
```

---

## Task 3: Database

**Files:**

- Create: `migrations/001_initial.sql`
- Create: `src/db/mod.rs`
- Create: `src/db/schema.rs`
- Create: `src/db/mods.rs`
- Create: `src/db/backups.rs`
- Create: `src/db/players.rs`
- Create: `src/db/users.rs`

**Step 1: Write the failing test**

```rust
// In src/db/mod.rs — write this test first
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_in_memory_and_migrate() {
        let db = Database::open_in_memory().unwrap();
        // If migrations succeed without panic, schema is valid
        let count: i32 = db.conn
            .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
```

**Step 2: Run failing test**

```bash
cargo test test_open_in_memory -- --nocapture
```

Expected: FAIL — `Database` not defined

**Step 3: Write `migrations/001_initial.sql`**

```sql
-- Safehouse schema v1

CREATE TABLE IF NOT EXISTS users (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    username        TEXT    NOT NULL UNIQUE,
    password_hash   TEXT    NOT NULL,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now'))
);

-- Steam Workshop mod cache
CREATE TABLE IF NOT EXISTS workshop_mods (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    workshop_id     TEXT    NOT NULL UNIQUE,
    mod_folder_name TEXT,           -- internal name used in Mods= line
    title           TEXT    NOT NULL,
    author          TEXT,
    description     TEXT,
    fetched_at      TEXT    NOT NULL DEFAULT (datetime('now'))
);

-- Named mod collection presets
CREATE TABLE IF NOT EXISTS mod_profiles (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT    NOT NULL UNIQUE,
    description     TEXT,
    workshop_ids    TEXT    NOT NULL DEFAULT '[]',  -- JSON array of workshop_id strings
    mod_names       TEXT    NOT NULL DEFAULT '[]',  -- JSON array of mod_folder_names in order
    created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT
);

-- Backup snapshot records
CREATE TABLE IF NOT EXISTS backup_snapshots (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    filename        TEXT    NOT NULL UNIQUE,
    label           TEXT,
    size_bytes      INTEGER,
    server_name     TEXT    NOT NULL,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    created_by      TEXT    NOT NULL DEFAULT 'cli'  -- 'cli', 'web', 'auto'
);

-- Player session log (parsed from PZ logs)
CREATE TABLE IF NOT EXISTS player_sessions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    player_name     TEXT    NOT NULL,
    steam_id        TEXT,
    joined_at       TEXT    NOT NULL DEFAULT (datetime('now')),
    left_at         TEXT
);

CREATE INDEX IF NOT EXISTS idx_player_sessions_name ON player_sessions(player_name);
CREATE INDEX IF NOT EXISTS idx_player_sessions_joined ON player_sessions(joined_at);
```

**Step 4: Implement `src/db/schema.rs`**

```rust
use rusqlite::Connection;

const MIGRATIONS: &[&str] = &[
    include_str!("../../migrations/001_initial.sql"),
];

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
                Ok(()) => {},
                Err(e) => {
                    let _ = conn.execute_batch("ROLLBACK");
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}
```

**Step 5: Implement `src/db/mod.rs`**

```rust
pub mod backups;
pub mod mods;
pub mod players;
pub mod schema;
pub mod users;

use std::path::Path;
use rusqlite::Connection;

pub struct Database {
    pub(crate) conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        schema::run_migrations(&conn)?;
        Ok(Self { conn })
    }
}
```

**Step 6: Add stub sub-modules** (so it compiles)

```rust
// src/db/mods.rs, src/db/backups.rs, src/db/players.rs, src/db/users.rs
// each just:  use super::Database;
```

**Step 7: Run tests**

```bash
cargo test test_open_in_memory -- --nocapture
```

Expected: PASS

**Step 8: Commit**

```bash
git add -A
git commit -m "feat: database scaffold — schema, migrations, Database struct"
```

---

## Task 4: PZ Server Detection

**Files:**

- Create: `src/pz/detect.rs`
- Modify: `src/pz/mod.rs`

**Step 1: Write the failing test**

```rust
// In src/pz/detect.rs
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_binary_found_when_present() {
        let tmp = tempdir().unwrap();
        let bin = tmp.path().join("ProjectZomboid64");
        std::fs::write(&bin, "").unwrap();
        let result = find_server_binary(tmp.path());
        assert!(result.is_some());
    }

    #[test]
    fn test_binary_absent() {
        let tmp = tempdir().unwrap();
        assert!(find_server_binary(tmp.path()).is_none());
    }

    #[test]
    fn test_read_pid_file_missing() {
        let tmp = tempdir().unwrap();
        let pid_file = tmp.path().join("server.pid");
        assert!(read_pid(&pid_file).is_none());
    }

    #[test]
    fn test_read_pid_file_valid() {
        let tmp = tempdir().unwrap();
        let pid_file = tmp.path().join("server.pid");
        std::fs::write(&pid_file, "12345\n").unwrap();
        assert_eq!(read_pid(&pid_file), Some(12345));
    }
}
```

**Step 2: Run failing tests**

```bash
cargo test pz::detect -- --nocapture
```

Expected: FAIL — module not found

**Step 3: Implement `src/pz/detect.rs`**

```rust
use std::path::{Path, PathBuf};

/// Find the PZ server binary in the given install directory.
pub fn find_server_binary(install_dir: &Path) -> Option<PathBuf> {
    let bin = install_dir.join("ProjectZomboid64");
    if bin.exists() { Some(bin) } else { None }
}

/// Read a PID from a PID file. Returns None if file is missing or unparseable.
pub fn read_pid(pid_file: &Path) -> Option<u32> {
    std::fs::read_to_string(pid_file).ok()?.trim().parse().ok()
}

/// Check whether a PID is alive by probing /proc/<pid>.
pub fn pid_is_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// Check if the PZ server is currently running via PID file.
pub fn is_server_running(pid_file: &Path) -> bool {
    read_pid(pid_file).is_some_and(pid_is_alive)
}

/// Acquire an exclusive advisory lock on the PID file.
/// Returns the locked File handle (caller must keep it alive while server runs).
/// Fails if another process already holds the lock.
pub fn lock_pid_file(pid_file: &Path) -> Result<std::fs::File, anyhow::Error> {
    use std::os::unix::io::AsRawFd;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(pid_file)?;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        anyhow::bail!("Another safehouse instance is already managing this server (PID file locked)");
    }
    Ok(file)
}
```

**Step 4: Update `src/pz/mod.rs`**

```rust
pub mod detect;
pub mod ini;      // next task
pub mod sandbox;  // next task
pub mod rcon;     // task 7
pub mod mods;     // task 9
pub mod logs;     // task 11
```

> Add `// TODO` stub files for each sub-module so it compiles.

**Step 5: Run tests**

```bash
cargo test pz::detect -- --nocapture
```

Expected: 4 tests pass

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: pz server detection — binary locate, PID file, is_running check"
```

---

## Task 5: server.ini Parser

The parser must **preserve comments and blank lines** on round-trip (server admins annotate their configs heavily).

**Files:**

- Create: `src/pz/ini.rs`

**Step 1: Write the failing test**

```rust
// In src/pz/ini.rs
#[cfg(test)]
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
```

**Step 2: Run failing tests**

```bash
cargo test pz::ini -- --nocapture
```

Expected: FAIL — module not found

**Step 3: Implement `src/pz/ini.rs`**

```rust
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

    pub fn to_string(&self) -> String {
        let mut out = self.lines.join("\n");
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out
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
                    // Preserve the original key casing from the file
                    let original_key = k.trim();
                    *line = format!("{original_key}={value}");
                    return;
                }
            }
        }
        // Key not found — append before end
        self.lines.push(format!("{key}={value}"));
    }

    /// Return the WorkshopItems= list as individual IDs.
    pub fn workshop_ids(&self) -> Vec<String> {
        self.get("WorkshopItems")
            .map(|v| v.split(';').filter(|s| !s.is_empty()).map(str::to_owned).collect())
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
        let ids: Vec<String> = self.workshop_ids().into_iter().filter(|x| x != id).collect();
        self.set_workshop_ids(&ids);
    }

    /// Return the Mods= list as individual folder names.
    pub fn mod_names(&self) -> Vec<String> {
        self.get("Mods")
            .map(|v| v.split(';').filter(|s| !s.is_empty()).map(str::to_owned).collect())
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
```

**Step 4: Run tests**

```bash
cargo test pz::ini -- --nocapture
```

Expected: All tests pass

**Step 5: Commit**

```bash
git add src/pz/ini.rs
git commit -m "feat: comment-preserving server.ini parser with mod list management"
```

---

## Task 6: SandboxVars.lua Parser

PZ's SandboxVars.lua format (Build 41+ uses nested tables):

```lua
SandboxVars = {
    MaxPlayers = 16,
    Zombies = {
        Speed = 3,
        Strength = 2,
    },
}
```

The parser supports dotted keys for nested values: `get("Zombies.Speed")` returns `"3"`.

**Files:**

- Create: `src/pz/sandbox.rs`

**Step 1: Write the failing test**

```rust
// In src/pz/sandbox.rs
#[cfg(test)]
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
```

**Step 2: Run failing tests**

```bash
cargo test pz::sandbox -- --nocapture
```

Expected: FAIL

**Step 3: Implement `src/pz/sandbox.rs`**

```rust
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

    pub fn to_string(&self) -> String {
        let mut out = self.lines.join("\n");
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out
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
            if trimmed.starts_with("--") { continue; }

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
                        depth == 2
                            && current_table.as_deref() == Some(parent)
                            && k == target_leaf
                    }
                    None => depth == 1 && k == target_leaf,
                };
                if is_match {
                    return Some(i);
                }
            }

            if trimmed.starts_with('}') && depth > 0 {
                depth -= 1;
                if depth == 1 { current_table = None; }
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
            // Safety: find_line only returns indices for lines containing '='
            #[allow(clippy::unwrap_used)]
            let eq = trimmed.find('=').unwrap();
            let original_key = trimmed[..eq].trim();
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            self.lines[idx] = format!("{indent}{original_key} = {value},");
        }
        // If key not found, do nothing — we don't auto-create nested tables
    }
}
```

**Step 4: Run tests**

```bash
cargo test pz::sandbox -- --nocapture
```

Expected: All pass

**Step 5: Commit**

```bash
git add src/pz/sandbox.rs
git commit -m "feat: SandboxVars.lua parser with dotted-key support"
```

---

## Task 7: RCON Client

Source RCON protocol implementation for sending admin commands to the PZ server.

**Files:**

- Create: `src/pz/rcon.rs`

**Step 1: Write the failing test**

```rust
// In src/pz/rcon.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_encode_decode() {
        let pkt = RconPacket { id: 1, pkt_type: SERVERDATA_AUTH, body: "mypassword".to_string() };
        let encoded = pkt.encode();
        let decoded = RconPacket::decode(&encoded).unwrap();
        assert_eq!(decoded.id, 1);
        assert_eq!(decoded.pkt_type, SERVERDATA_AUTH);
        assert_eq!(decoded.body, "mypassword");
    }

    #[test]
    fn test_packet_encode_decode() {
        let pkt = RconPacket { id: 1, pkt_type: SERVERDATA_AUTH, body: "mypassword".to_string() };
        let encoded = pkt.encode();
        let decoded = RconPacket::decode(&encoded).unwrap();
        assert_eq!(decoded.id, 1);
        assert_eq!(decoded.pkt_type, SERVERDATA_AUTH);
        assert_eq!(decoded.body, "mypassword");
    }

    #[test]
    fn test_empty_body_encode_decode() {
        let pkt = RconPacket { id: 42, pkt_type: SERVERDATA_EXECCOMMAND, body: String::new() };
        let encoded = pkt.encode();
        let decoded = RconPacket::decode(&encoded).unwrap();
        assert_eq!(decoded.body, "");
    }
}
```

**Step 2: Run failing tests**

```bash
cargo test pz::rcon -- --nocapture
```

Expected: FAIL

**Step 3: Implement `src/pz/rcon.rs`**

```rust
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use anyhow::{bail, Context, Result};

pub const SERVERDATA_AUTH: i32 = 3;
pub const SERVERDATA_AUTH_RESPONSE: i32 = 2;
pub const SERVERDATA_EXECCOMMAND: i32 = 2;
pub const SERVERDATA_RESPONSE_VALUE: i32 = 0;

#[derive(Debug, Clone)]
pub struct RconPacket {
    pub id: i32,
    pub pkt_type: i32,
    pub body: String,
}

impl RconPacket {
    /// Encode to wire format.
    pub fn encode(&self) -> Vec<u8> {
        let body = self.body.as_bytes();
        // length field = id(4) + type(4) + body + null(1) + padding_null(1)
        let length = (4 + 4 + body.len() + 2) as i32;
        let mut buf = Vec::with_capacity(4 + length as usize);
        buf.extend_from_slice(&length.to_le_bytes());
        buf.extend_from_slice(&self.id.to_le_bytes());
        buf.extend_from_slice(&self.pkt_type.to_le_bytes());
        buf.extend_from_slice(body);
        buf.push(0); // body null terminator
        buf.push(0); // packet null terminator
        buf
    }

    /// Decode from a complete raw buffer (including the leading 4-byte length).
    pub fn decode(buf: &[u8]) -> Result<Self> {
        if buf.len() < 14 {
            bail!("RCON packet too short: {} bytes", buf.len());
        }
        let length = i32::from_le_bytes(buf[0..4].try_into()?) as usize;
        if buf.len() < 4 + length {
            bail!("RCON buffer too short for declared length");
        }
        let id = i32::from_le_bytes(buf[4..8].try_into()?);
        let pkt_type = i32::from_le_bytes(buf[8..12].try_into()?);
        // body is between byte 12 and the null terminator
        let body_end = (4 + length).saturating_sub(2); // strip two trailing nulls
        let body_bytes = &buf[12..body_end.max(12)];
        let body = String::from_utf8_lossy(body_bytes).into_owned();
        Ok(Self { id, pkt_type, body })
    }
}

pub struct RconClient {
    stream: TcpStream,
    next_id: i32,
}

impl RconClient {
    /// Connect and authenticate. Errors if authentication fails.
    pub fn connect(host: &str, port: u16, password: &str) -> Result<Self> {
        let addr = format!("{host}:{port}");
        let stream = TcpStream::connect_timeout(
            &addr.parse().context("invalid RCON address")?,
            Duration::from_secs(5),
        )
        .with_context(|| format!("cannot connect to RCON at {addr}"))?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;

        let mut client = Self { stream, next_id: 1 };
        client.authenticate(password)?;
        Ok(client)
    }

    fn next_id(&mut self) -> i32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn send(&mut self, pkt: &RconPacket) -> Result<()> {
        self.stream.write_all(&pkt.encode())?;
        Ok(())
    }

    fn recv(&mut self) -> Result<RconPacket> {
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf)?;
        let length = i32::from_le_bytes(len_buf) as usize;
        let mut rest = vec![0u8; length];
        self.stream.read_exact(&mut rest)?;
        // Re-assemble with length prefix for decode
        let mut buf = Vec::with_capacity(4 + length);
        buf.extend_from_slice(&len_buf);
        buf.extend_from_slice(&rest);
        RconPacket::decode(&buf)
    }

    fn authenticate(&mut self, password: &str) -> Result<()> {
        let id = self.next_id();
        let auth_pkt = RconPacket { id, pkt_type: SERVERDATA_AUTH, body: password.to_string() };
        self.send(&auth_pkt)?;
        // Source RCON sends TWO responses to auth:
        //   1. An empty SERVERDATA_RESPONSE_VALUE (type 0)
        //   2. The actual SERVERDATA_AUTH_RESPONSE (type 2) with id=-1 on failure
        let first = self.recv()?;
        // If the first response is already the auth response (some implementations skip the empty one)
        let auth_response = if first.pkt_type == SERVERDATA_AUTH_RESPONSE || first.id == -1 {
            first
        } else {
            // Consume the second packet which is the real auth response
            self.recv()?
        };
        if auth_response.id == -1 {
            bail!("RCON authentication failed — check rcon_password in safehouse.toml");
        }
        Ok(())
    }

    /// Send a command and return the response body.
    pub fn send_command(&mut self, command: &str) -> Result<String> {
        let id = self.next_id();
        let pkt = RconPacket { id, pkt_type: SERVERDATA_EXECCOMMAND, body: command.to_string() };
        self.send(&pkt)?;
        let resp = self.recv()?;
        Ok(resp.body)
    }
}
```

**Step 4: Run tests**

```bash
cargo test pz::rcon -- --nocapture
```

Expected: 2 tests pass (encode/decode tests; TCP tests need a real server)

**Step 5: Commit**

```bash
git add src/pz/rcon.rs
git commit -m "feat: Source RCON TCP client — connect, authenticate, send_command"
```

---

## Task 8: Steam Workshop Mod Metadata

PZ requires two identifiers per mod: the Workshop ID (numeric, on Steam) and the mod's internal folder name (declared inside the mod itself). We fetch metadata from Steam Web API — no API key required for `GetPublishedFileDetails`.

**Files:**

- Create: `src/steam/workshop.rs`
- Modify: `src/steam/mod.rs`

**Step 1: Write the failing test**

```rust
// In src/steam/workshop.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workshop_info_struct() {
        let info = WorkshopModInfo {
            workshop_id: "2392987220".to_string(),
            title: "Brita's Weapon Pack".to_string(),
            author: Some("Brita".to_string()),
            description: None,
        };
        assert_eq!(info.workshop_id, "2392987220");
    }

    #[test]
    fn test_parse_api_response() {
        let json = serde_json::json!({
            "response": {
                "publishedfiledetails": [{
                    "publishedfileid": "2392987220",
                    "title": "Brita's Weapon Pack",
                    "creator": "76561198XXXXX",
                    "description": "Adds weapons"
                }]
            }
        });
        let info = parse_file_details(&json["response"]["publishedfiledetails"][0]).unwrap();
        assert_eq!(info.title, "Brita's Weapon Pack");
    }
}
```

**Step 2: Implement `src/steam/workshop.rs`**

```rust
use anyhow::{Context, Result};
use serde_json::Value;

const DETAILS_URL: &str =
    "https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkshopModInfo {
    pub workshop_id: String,
    pub title: String,
    pub author: Option<String>,
    pub description: Option<String>,
}

pub fn parse_file_details(detail: &Value) -> Option<WorkshopModInfo> {
    Some(WorkshopModInfo {
        workshop_id: detail["publishedfileid"].as_str()?.to_owned(),
        title: detail["title"].as_str().unwrap_or("Unknown").to_owned(),
        author: detail["creator"].as_str().map(str::to_owned),
        description: detail["description"].as_str().map(str::to_owned),
    })
}

/// Fetch metadata for a single Workshop item. Blocking wrapper around async fetch.
pub async fn fetch_mod_info(
    client: &reqwest::Client,
    workshop_id: &str,
) -> Result<WorkshopModInfo> {
    let params = [
        ("itemcount", "1"),
        ("publishedfileids[0]", workshop_id),
    ];
    let resp: Value = client
        .post(DETAILS_URL)
        .form(&params)
        .send()
        .await
        .context("Steam API request failed")?
        .json()
        .await
        .context("Steam API response parse failed")?;

    let detail = &resp["response"]["publishedfiledetails"][0];
    parse_file_details(detail)
        .with_context(|| format!("no details returned for workshop ID {workshop_id}"))
}
```

**Step 3: Update `src/steam/mod.rs`**

```rust
pub mod workshop;
pub use workshop::{fetch_mod_info, WorkshopModInfo};
```

**Step 4: Run tests**

```bash
cargo test steam:: -- --nocapture
```

Expected: 2 tests pass (no network needed for unit tests)

**Step 5: Commit**

```bash
git add src/steam/
git commit -m "feat: Steam Workshop metadata fetcher — GetPublishedFileDetails API"
```

---

## Task 9: Mod Management (DB + server.ini Integration)

**Files:**

- Create: `src/db/mods.rs`
- Create: `src/pz/mods.rs`

**Step 1: Write the failing test**

```rust
// In src/pz/mods.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::pz::ini::IniEditor;

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
}
```

**Step 2: Implement `src/pz/mods.rs`**

```rust
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
        .zip(ini.mod_names().into_iter())
        .collect()
}
```

**Step 3: Implement `src/db/mods.rs`**

```rust
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
            params![info.workshop_id, folder, info.title, info.author, info.description],
        )?;
        Ok(())
    }

    pub fn get_cached_mod(&self, workshop_id: &str) -> Result<Option<CachedMod>> {
        let mut stmt = self.conn.prepare(
            "SELECT workshop_id, mod_folder_name, title, author FROM workshop_mods WHERE workshop_id = ?1"
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

    pub fn save_mod_profile(&self, name: &str, desc: Option<&str>, ids: &[String], names: &[String]) -> Result<()> {
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
        let mut stmt = self.conn.prepare(
            "SELECT workshop_ids, mod_names FROM mod_profiles WHERE name = ?1"
        )?;
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
        let mut stmt = self.conn.prepare("SELECT name FROM mod_profiles ORDER BY name")?;
        let names = stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(names)
    }
}
```

**Step 4: Run tests**

```bash
cargo test pz::mods -- --nocapture
```

Expected: 3 tests pass

**Step 5: Commit**

```bash
git add src/pz/mods.rs src/db/mods.rs
git commit -m "feat: mod management — ini integration, DB cache, named profiles"
```

---

## Task 10: Backup Engine

**Files:**

- Create: `src/backup.rs`
- Create: `src/db/backups.rs`

**Step 1: Write the failing test**

```rust
// In src/backup.rs
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_and_list_snapshots() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        // Create fake save data
        std::fs::write(src.path().join("map_zone.bin"), b"fake data").unwrap();

        let snap_path = create_snapshot(src.path(), dst.path(), "testworld", None).unwrap();
        assert!(snap_path.exists());
        assert!(snap_path.extension().unwrap() == "gz");
    }

    #[test]
    fn test_restore_snapshot() {
        let src = tempdir().unwrap();
        let snap_dir = tempdir().unwrap();
        let restore_dir = tempdir().unwrap();

        std::fs::write(src.path().join("map_zone.bin"), b"original").unwrap();
        let snap = create_snapshot(src.path(), snap_dir.path(), "testworld", None).unwrap();

        restore_snapshot(&snap, restore_dir.path()).unwrap();
        assert!(restore_dir.path().join("map_zone.bin").exists());
    }
}
```

**Step 2: Implement `src/backup.rs`**

```rust
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;

/// Create a .tar.gz snapshot of `source_dir` into `dest_dir`.
/// Returns the path of the created archive.
pub fn create_snapshot(
    source_dir: &Path,
    dest_dir: &Path,
    server_name: &str,
    label: Option<&str>,
) -> Result<PathBuf> {
    std::fs::create_dir_all(dest_dir)?;
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let label_part = label.map(|l| format!("_{l}")).unwrap_or_default();
    let filename = format!("{server_name}_{timestamp}{label_part}.tar.gz");
    let out_path = dest_dir.join(&filename);
    let tmp_path = dest_dir.join(format!(".{filename}.tmp"));

    let file = std::fs::File::create(&tmp_path)
        .with_context(|| format!("cannot create backup file {}", tmp_path.display()))?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut builder = tar::Builder::new(enc);
    builder
        .append_dir_all(".", source_dir)
        .context("failed to archive save directory")?;
    builder.finish()?;

    // Atomic rename: only the complete archive gets the final name
    std::fs::rename(&tmp_path, &out_path)
        .with_context(|| format!("cannot rename temp backup to {}", out_path.display()))?;

    Ok(out_path)
}

/// Extract a snapshot archive into `dest_dir` (overwrites contents).
pub fn restore_snapshot(archive: &Path, dest_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dest_dir)?;
    let file = std::fs::File::open(archive)
        .with_context(|| format!("cannot open backup {}", archive.display()))?;
    let dec = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);
    archive
        .unpack(dest_dir)
        .context("failed to extract backup archive")?;
    Ok(())
}

/// List snapshot files in `backup_dir`, sorted newest first.
pub fn list_snapshots(backup_dir: &Path) -> Result<Vec<PathBuf>> {
    if !backup_dir.exists() {
        return Ok(vec![]);
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(backup_dir)?
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x == "gz")
        })
        .map(|e| e.path())
        .collect();
    entries.sort_by_key(|p| {
        std::cmp::Reverse(
            p.metadata().ok().and_then(|m| m.modified().ok()),
        )
    });
    Ok(entries)
}

/// Delete snapshots older than `retain_days`, keeping at least `min_keep`.
pub fn prune_snapshots(backup_dir: &Path, retain_days: u32, min_keep: usize) -> Result<Vec<PathBuf>> {
    let all = list_snapshots(backup_dir)?;
    let cutoff = Utc::now() - chrono::Duration::days(retain_days as i64);
    let mut pruned = vec![];
    for path in all.iter().skip(min_keep) {
        let mtime = path
            .metadata()?
            .modified()?;
        let mtime_utc: chrono::DateTime<Utc> = mtime.into();
        if mtime_utc < cutoff {
            std::fs::remove_file(path)?;
            pruned.push(path.clone());
        }
    }
    Ok(pruned)
}

// Re-export std::cmp::Reverse since we use it in sort
use std::cmp::Reverse;
```

**Step 3: Implement `src/db/backups.rs`**

```rust
use anyhow::Result;
use rusqlite::params;
use crate::db::Database;

#[derive(Debug, Clone)]
pub struct BackupSnapshot {
    pub id: i64,
    pub filename: String,
    pub label: Option<String>,
    pub size_bytes: Option<i64>,
    pub server_name: String,
    pub created_at: String,
    pub created_by: String,
}

impl Database {
    pub fn record_backup(&self, filename: &str, label: Option<&str>, size_bytes: Option<i64>, server_name: &str, created_by: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO backup_snapshots (filename, label, size_bytes, server_name, created_by)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![filename, label, size_bytes, server_name, created_by],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_backups(&self) -> Result<Vec<BackupSnapshot>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, filename, label, size_bytes, server_name, created_at, created_by
             FROM backup_snapshots ORDER BY created_at DESC"
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(BackupSnapshot {
                id: r.get(0)?,
                filename: r.get(1)?,
                label: r.get(2)?,
                size_bytes: r.get(3)?,
                server_name: r.get(4)?,
                created_at: r.get(5)?,
                created_by: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn delete_backup_record(&self, filename: &str) -> Result<()> {
        self.conn.execute("DELETE FROM backup_snapshots WHERE filename = ?1", params![filename])?;
        Ok(())
    }
}
```

**Step 4: Run tests**

```bash
cargo test backup:: -- --nocapture
```

Expected: 2 tests pass

**Step 5: Commit**

```bash
git add src/backup.rs src/db/backups.rs
git commit -m "feat: backup engine — tar.gz snapshots, restore, prune, DB records"
```

---

## Task 11: Log Parser + Player Session Tracking

PZ log lines look like:

```
1723165432783 LOG  : General     , 1723165432783> user 'Alice' disconnected
1723165432000 LOG  : General     , 1723165432000> user 'Bob' connected
```

**Files:**

- Create: `src/pz/logs.rs`
- Create: `src/db/players.rs`

**Step 1: Write the failing test**

```rust
// In src/pz/logs.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_connect() {
        let line = "1723165432000 LOG  : General     , 1723165432000> user 'Alice' connected";
        let event = parse_log_line(line);
        assert_eq!(event, Some(PlayerEvent::Connected { name: "Alice".to_string() }));
    }

    #[test]
    fn test_parse_disconnect() {
        let line = "1723165432783 LOG  : General     , 1723165432783> user 'Bob' disconnected";
        let event = parse_log_line(line);
        assert_eq!(event, Some(PlayerEvent::Disconnected { name: "Bob".to_string() }));
    }

    #[test]
    fn test_parse_irrelevant_line() {
        let line = "1723165432000 LOG  : General     , something else happened";
        assert_eq!(parse_log_line(line), None);
    }
}
```

**Step 2: Implement `src/pz/logs.rs`**

```rust
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    Connected { name: String },
    Disconnected { name: String },
}

static CONNECT_RE: OnceLock<Regex> = OnceLock::new();
static DISCONNECT_RE: OnceLock<Regex> = OnceLock::new();

pub fn parse_log_line(line: &str) -> Option<PlayerEvent> {
    let connect_re = CONNECT_RE.get_or_init(|| {
        // Static regexes — unwrap is safe, they never change
        #[allow(clippy::unwrap_used)]
        Regex::new(r"user '([^']+)' connected").unwrap()
    });
    let disconnect_re = DISCONNECT_RE.get_or_init(|| {
        #[allow(clippy::unwrap_used)]
        Regex::new(r"user '([^']+)' disconnected").unwrap()
    });

    if let Some(cap) = connect_re.captures(line) {
        return Some(PlayerEvent::Connected { name: cap[1].to_owned() });
    }
    if let Some(cap) = disconnect_re.captures(line) {
        return Some(PlayerEvent::Disconnected { name: cap[1].to_owned() });
    }
    None
}

/// Tail `n` lines from a file path.
pub fn tail_lines(path: &std::path::Path, n: usize) -> anyhow::Result<Vec<String>> {
    use std::io::{BufRead, BufReader};
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().collect::<std::io::Result<_>>()?;
    let start = lines.len().saturating_sub(n);
    Ok(lines[start..].to_vec())
}
```

**Step 3: Implement `src/db/players.rs`**

```rust
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
             ORDER BY joined_at DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit as i64], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}
```

**Step 4: Run tests**

```bash
cargo test pz::logs -- --nocapture
```

Expected: 3 tests pass

**Step 5: Commit**

```bash
git add src/pz/logs.rs src/db/players.rs
git commit -m "feat: PZ log parser — player connect/disconnect events + session DB"
```

---

## Task 12: Discord Webhook Notifications

**Files:**

- Create: `src/notify.rs`

**Step 1: Write the failing test**

```rust
// In src/notify.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_embed_payload() {
        let payload = build_webhook_payload("🟢 Server started", "safehouse", "#00ff00");
        let obj = payload.as_object().unwrap();
        assert!(obj.contains_key("embeds"));
    }
}
```

**Step 2: Implement `src/notify.rs`**

```rust
use anyhow::Result;
use serde_json::{json, Value};

pub enum NotifyEvent {
    ServerStarted,
    ServerStopped,
    PlayerJoined(String),
    PlayerLeft(String),
    BackupComplete { filename: String },
    UpdateAvailable { version: String },
}

impl NotifyEvent {
    /// Returns (title, hex_color). Title is owned because dynamic variants
    /// interpolate runtime strings.
    pub fn title_and_color(&self) -> (String, &'static str) {
        match self {
            NotifyEvent::ServerStarted => ("🟢 Server started".to_string(), "00b300"),
            NotifyEvent::ServerStopped => ("🔴 Server stopped".to_string(), "cc0000"),
            NotifyEvent::PlayerJoined(n) => (format!("👤 {n} joined"), "0099cc"),
            NotifyEvent::PlayerLeft(n)  => (format!("👋 {n} left"), "888888"),
            NotifyEvent::BackupComplete { filename } => (format!("💾 Backup: {filename}"), "ffaa00"),
            NotifyEvent::UpdateAvailable { version } => (format!("⬆️ Update available: v{version}"), "aa00ff"),
        }
    }
}

pub fn build_webhook_payload(title: &str, server_name: &str, hex_color: &str) -> Value {
    let color = i64::from_str_radix(hex_color, 16).unwrap_or(0);
    json!({
        "username": format!("Safehouse | {server_name}"),
        "embeds": [{
            "title": title,
            "color": color
        }]
    })
}

pub async fn send_webhook(client: &reqwest::Client, url: &str, payload: Value) -> Result<()> {
    client
        .post(url)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

pub async fn notify(client: &reqwest::Client, webhook_url: Option<&str>, server_name: &str, event: NotifyEvent) -> Result<()> {
    let Some(url) = webhook_url else { return Ok(()) };
    let (title, color) = event.title_and_color();
    let payload = build_webhook_payload(&title, server_name, color);
    send_webhook(client, url, payload).await
}
```

**Step 3: Run tests**

```bash
cargo test notify:: -- --nocapture
```

Expected: 1 test passes

**Step 4: Commit**

```bash
git add src/notify.rs
git commit -m "feat: Discord webhook notifications — server events, player activity"
```

---

## Task 13: CLI Scaffold

**Files:**

- Create: `src/cli/mod.rs` (full Command enum + Cli struct)
- Create: `src/cli/common.rs` (CliContext)
- Create stubs: `src/cli/setup.rs`, `src/cli/server.rs`, `src/cli/config.rs`, `src/cli/mods.rs`, `src/cli/backup.rs`, `src/cli/console.rs`, `src/cli/serve.rs`, `src/cli/webhook.rs`
- Modify: `src/main.rs`

**Step 1: Implement `src/cli/mod.rs`**

```rust
use std::path::PathBuf;
use clap::{Parser, Subcommand};

pub mod backup;
pub mod common;
pub mod config;
pub mod console;
pub mod mods;
pub mod serve;
pub mod server;
pub mod setup;
pub mod webhook;

#[derive(Parser)]
#[command(name = "safehouse", version = env!("SAFEHOUSE_VERSION"),
          about = "Project Zomboid dedicated server manager")]
pub struct Cli {
    /// Safehouse data directory (default: ~/.local/share/safehouse)
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,

    /// Config file path override
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Increase verbosity (-v debug, -vv trace)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize safehouse and install the PZ dedicated server
    Setup {
        /// Where to install the PZ server (default: ~/pzserver)
        #[arg(long)]
        install_dir: Option<PathBuf>,
        /// Admin password for the PZ server
        #[arg(long)]
        admin_password: Option<String>,
    },

    /// Manage the PZ server process
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },

    /// Edit server configuration files
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Manage Steam Workshop mods
    Mods {
        #[command(subcommand)]
        action: ModAction,
    },

    /// Manage world backups
    Backup {
        #[command(subcommand)]
        action: BackupAction,
    },

    /// Send RCON admin commands
    Console {
        #[command(subcommand)]
        action: ConsoleAction,
    },

    /// Configure Discord webhook notifications
    Webhook {
        /// Discord webhook URL
        #[arg(long)]
        url: Option<String>,
        /// Send a test notification
        #[arg(long)]
        test: bool,
    },

    /// Start the web management UI
    Serve {
        #[arg(long)]
        bind: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
}

#[derive(Subcommand)]
pub enum ServerAction {
    /// Start the server
    Start {
        #[arg(long, default_value = "60")]
        timeout: u64,
    },
    /// Stop the server gracefully (RCON save + shutdown)
    Stop,
    /// Restart the server
    Restart,
    /// Stream server logs to stdout
    Logs {
        #[arg(short, long)]
        follow: bool,
        #[arg(long, default_value = "100")]
        lines: usize,
    },
    /// Show server status (running, player count, uptime)
    Status,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show all current server.ini values
    Show,
    /// Set a key in server.ini
    Set { key: String, value: String },
    /// Edit SandboxVars.lua
    Sandbox {
        #[command(subcommand)]
        action: SandboxAction,
    },
    /// Manage named config presets
    Preset {
        #[command(subcommand)]
        action: PresetAction,
    },
}

#[derive(Subcommand)]
pub enum SandboxAction {
    Show,
    Set { key: String, value: String },
}

#[derive(Subcommand)]
pub enum PresetAction {
    List,
    Save { name: String },
    Apply { name: String },
}

#[derive(Subcommand)]
pub enum ModAction {
    /// List installed Workshop mods
    List,
    /// Add a mod by Workshop ID (you must also provide the mod's folder name)
    Add {
        workshop_id: String,
        /// Internal mod folder name (shown in the mod's README or on Workshop page)
        mod_name: String,
    },
    /// Remove a mod by Workshop ID
    Remove { workshop_id: String },
    /// Fetch and display Workshop metadata for an ID
    Info { workshop_id: String },
    /// Manage named mod collection profiles
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
}

#[derive(Subcommand)]
pub enum ProfileAction {
    List,
    Save { name: String },
    Load { name: String },
}

#[derive(Subcommand)]
pub enum BackupAction {
    /// Create a snapshot of the world save + configs
    Create {
        #[arg(long)]
        label: Option<String>,
    },
    /// List available snapshots
    List,
    /// Restore a snapshot (stops server first)
    Restore { filename: String },
    /// Delete snapshots older than retention policy
    Prune {
        #[arg(long, default_value = "2")]
        min_keep: usize,
    },
}

#[derive(Subcommand)]
pub enum ConsoleAction {
    /// Broadcast a message to all players
    Chat { message: String },
    /// List connected players
    Players,
    /// Kick a player by name
    Kick { player: String },
    /// Ban a player by name
    Ban { player: String },
    /// Give an item to a player
    Give { player: String, item: String },
    /// Trigger an in-game world save
    Save,
}
```

**Step 2: Implement `src/cli/common.rs`**

```rust
use anyhow::{Context, Result};

use crate::config::SafehouseConfig;
use crate::db::Database;
use crate::dirs::SafehouseDirs;

pub struct CliContext {
    pub dirs: SafehouseDirs,
    pub config: SafehouseConfig,
    pub db: Database,
    pub http: reqwest::Client,
}

pub fn resolve_context(cli: &super::Cli) -> Result<CliContext> {
    let dirs = SafehouseDirs::detect(cli.data_dir.as_deref())?;
    dirs.ensure_dirs()?;

    let config_path = cli.config.clone().unwrap_or_else(|| dirs.config_path());
    let config = if config_path.exists() {
        SafehouseConfig::load(&config_path)?
    } else {
        anyhow::bail!(
            "No config found at {}. Run `safehouse setup` first.",
            config_path.display()
        )
    };

    let db = Database::open(&dirs.db_path())
        .context("failed to open database")?;

    let http = reqwest::Client::builder()
        .user_agent(concat!("safehouse/", env!("SAFEHOUSE_VERSION")))
        .build()?;

    Ok(CliContext { dirs, config, db, http })
}
```

**Step 3: Update `src/main.rs`**

```rust
use anyhow::Result;
use clap::Parser;
use safehouse::cli::{self, Cli, Command};
use safehouse::logging;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    logging::init(cli.verbose);

    match &cli.command {
        Command::Setup { install_dir, admin_password } => {
            cli::setup::run(install_dir.as_deref(), admin_password.as_deref()).await
        }
        Command::Server { action } => {
            let ctx = cli::common::resolve_context(&cli)?;
            cli::server::run(action, &ctx).await
        }
        Command::Config { action } => {
            let ctx = cli::common::resolve_context(&cli)?;
            cli::config::run(action, &ctx)
        }
        Command::Mods { action } => {
            let ctx = cli::common::resolve_context(&cli)?;
            cli::mods::run(action, &ctx).await
        }
        Command::Backup { action } => {
            let ctx = cli::common::resolve_context(&cli)?;
            cli::backup::run(action, &ctx).await
        }
        Command::Console { action } => {
            let ctx = cli::common::resolve_context(&cli)?;
            cli::console::run(action, &ctx).await
        }
        Command::Webhook { url, test } => {
            let ctx = cli::common::resolve_context(&cli)?;
            cli::webhook::run(url.as_deref(), *test, &ctx).await
        }
        Command::Serve { bind, port } => {
            cli::serve::run(bind.as_deref(), *port, &cli).await
        }
    }
}
```

**Step 4: Create stub files for each CLI module**

Each stub must match the signature used in `main.rs` dispatch. Create these files:

```rust
// src/cli/server.rs
use anyhow::Result;
use super::common::CliContext;
use super::ServerAction;
pub async fn run(_action: &ServerAction, _ctx: &CliContext) -> Result<()> {
    todo!("server commands")
}
```

```rust
// src/cli/config.rs — note: sync fn, not async
use anyhow::Result;
use super::common::CliContext;
use super::ConfigAction;
pub fn run(_action: &ConfigAction, _ctx: &CliContext) -> Result<()> {
    todo!("config commands")
}
```

```rust
// src/cli/mods.rs
use anyhow::Result;
use super::common::CliContext;
use super::ModAction;
pub async fn run(_action: &ModAction, _ctx: &CliContext) -> Result<()> {
    todo!("mod commands")
}
```

```rust
// src/cli/backup.rs
use anyhow::Result;
use super::common::CliContext;
use super::BackupAction;
pub async fn run(_action: &BackupAction, _ctx: &CliContext) -> Result<()> {
    todo!("backup commands")
}
```

```rust
// src/cli/console.rs
use anyhow::Result;
use super::common::CliContext;
use super::ConsoleAction;
pub async fn run(_action: &ConsoleAction, _ctx: &CliContext) -> Result<()> {
    todo!("console commands")
}
```

```rust
// src/cli/setup.rs
use std::path::Path;
use anyhow::Result;
pub async fn run(_install_dir: Option<&Path>, _admin_password: Option<&str>) -> Result<()> {
    todo!("setup")
}
```

```rust
// src/cli/serve.rs
use anyhow::Result;
pub async fn run(_bind: Option<&str>, _port: Option<u16>, _cli: &super::Cli) -> Result<()> {
    todo!("serve")
}
```

```rust
// src/cli/webhook.rs
use anyhow::Result;
use super::common::CliContext;
pub async fn run(_url: Option<&str>, _test: bool, _ctx: &CliContext) -> Result<()> {
    todo!("webhook")
}
```

**Step 5: Verify CLI skeleton compiles and shows help**

```bash
cargo build 2>&1 | tail -5
cargo run -- --help
cargo run -- server --help
cargo run -- mods --help
```

Expected: help text shows all subcommands correctly

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: CLI scaffold — all subcommands defined, CliContext, main dispatch"
```

---

## Task 14: CLI — Server Lifecycle Commands

**Files:**

- Modify: `src/cli/server.rs`

**Step 1: Implement `src/cli/server.rs`**

```rust
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tokio::process::Command;
use std::process::Stdio;

use super::common::CliContext;
use super::ServerAction;
use crate::pz::detect::{find_server_binary, is_server_running, pid_is_alive, read_pid};

pub async fn run(action: &ServerAction, ctx: &CliContext) -> Result<()> {
    match action {
        ServerAction::Start { timeout } => start(ctx, *timeout).await,
        ServerAction::Stop => stop(ctx).await,
        ServerAction::Restart => restart(ctx).await,
        ServerAction::Logs { follow, lines } => logs(ctx, *follow, *lines).await,
        ServerAction::Status => status(ctx).await,
    }
}

async fn start(ctx: &CliContext, timeout_secs: u64) -> Result<()> {
    if is_server_running(&ctx.dirs.pid_file()) {
        bail!("Server is already running. Use `safehouse server status` to check.");
    }

    // Acquire exclusive lock on PID file to prevent concurrent starts
    let pid_lock = crate::pz::detect::lock_pid_file(&ctx.dirs.pid_file())
        .context("cannot acquire PID file lock")?;

    let install_dir = &ctx.config.server_install_dir;
    let binary = find_server_binary(install_dir)
        .with_context(|| format!("PZ binary not found in {}", install_dir.display()))?;

    println!("Starting PZ server '{}'...", ctx.config.server_name);

    let child = Command::new(&binary)
        .arg("-servername")
        .arg(&ctx.config.server_name)
        .arg("-rconpassword")
        .arg(&ctx.config.rcon_password)
        .current_dir(install_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to spawn server process")?;

    let pid = child.id().context("failed to get server PID")?;
    // Write PID to the locked file
    use std::io::Write;
    let mut pid_lock = pid_lock;
    write!(pid_lock, "{}", pid)?;

    // Detach — let the process run independently
    tokio::spawn(async move { let _ = child.wait_with_output().await; });

    // Wait for RCON to become available (indicates server is ready)
    println!("Waiting for server to become ready (timeout: {timeout_secs}s)...");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        if tokio::time::Instant::now() > deadline {
            bail!("Server did not respond within {timeout_secs}s — check logs with `safehouse server logs`");
        }
        if let Ok(mut rcon) = crate::pz::rcon::RconClient::connect(
            "127.0.0.1",
            ctx.config.rcon_port,
            &ctx.config.rcon_password,
        ) {
            let _ = rcon.send_command("help");
            println!("Server is ready (PID {pid}).");
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

async fn stop(ctx: &CliContext) -> Result<()> {
    if !is_server_running(&ctx.dirs.pid_file()) {
        println!("Server is not running.");
        return Ok(());
    }

    // Graceful: RCON save then quit
    println!("Saving world and stopping server...");
    if let Ok(mut rcon) = crate::pz::rcon::RconClient::connect(
        "127.0.0.1",
        ctx.config.rcon_port,
        &ctx.config.rcon_password,
    ) {
        let _ = rcon.send_command("save");
        tokio::time::sleep(Duration::from_secs(3)).await;
        let _ = rcon.send_command("quit");
    } else {
        tracing::warn!("Could not connect to RCON; sending SIGTERM directly");
        if let Some(pid) = read_pid(&ctx.dirs.pid_file()) {
            unsafe { libc::kill(pid as i32, libc::SIGTERM); }
        }
    }

    // Wait for process to exit
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_secs(2)).await;
        if !is_server_running(&ctx.dirs.pid_file()) {
            let _ = std::fs::remove_file(ctx.dirs.pid_file());
            println!("Server stopped.");
            return Ok(());
        }
    }

    // Force kill after 40s
    if let Some(pid) = read_pid(&ctx.dirs.pid_file()) {
        unsafe { libc::kill(pid as i32, libc::SIGKILL); }
    }
    let _ = std::fs::remove_file(ctx.dirs.pid_file());
    println!("Server force-killed.");
    Ok(())
}

async fn restart(ctx: &CliContext) -> Result<()> {
    stop(ctx).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    start(ctx, 60).await
}

async fn logs(ctx: &CliContext, follow: bool, lines: usize) -> Result<()> {
    let log_path = ctx.dirs.latest_log(&ctx.config)
        .context("No PZ log file found. Has the server been started at least once?")?;

    if follow {
        println!("Following {} (Ctrl+C to stop)...", log_path.display());
        let mut pos = std::fs::metadata(&log_path)?.len();
        loop {
            let meta = std::fs::metadata(&log_path)?;
            if meta.len() > pos {
                use std::io::{Read, Seek, SeekFrom};
                let mut f = std::fs::File::open(&log_path)?;
                f.seek(SeekFrom::Start(pos))?;
                let mut buf = String::new();
                f.read_to_string(&mut buf)?;
                print!("{buf}");
                pos = meta.len();
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    } else {
        let tail = crate::pz::logs::tail_lines(&log_path, lines)?;
        for line in tail {
            println!("{line}");
        }
    }
    Ok(())
}

async fn status(ctx: &CliContext) -> Result<()> {
    let running = is_server_running(&ctx.dirs.pid_file());
    let pid = read_pid(&ctx.dirs.pid_file());

    println!("Server:  {}", if running { "🟢 Running" } else { "🔴 Stopped" });
    if let Some(p) = pid {
        println!("PID:     {p}");
    }
    println!("Name:    {}", ctx.config.server_name);
    println!("Install: {}", ctx.config.server_install_dir.display());

    if running {
        if let Ok(mut rcon) = crate::pz::rcon::RconClient::connect(
            "127.0.0.1",
            ctx.config.rcon_port,
            &ctx.config.rcon_password,
        ) {
            let players = rcon.send_command("players").unwrap_or_else(|_| "?".to_string());
            println!("Players: {players}");
        }
    }
    Ok(())
}
```

**Step 2: Verify it compiles**

```bash
cargo build 2>&1 | tail -5
```

Expected: `Finished dev`

**Step 3: Manual smoke test** (requires a PZ server directory)

```bash
cargo run -- server status
```

Expected: Shows server as stopped

**Step 4: Commit**

```bash
git add src/cli/server.rs
git commit -m "feat: CLI server commands — start, stop, restart, logs, status"
```

---

## Task 15: CLI — Config & Mod Commands

**Files:**

- Modify: `src/cli/config.rs`
- Modify: `src/cli/mods.rs`

**Step 1: Implement `src/cli/config.rs`**

```rust
use anyhow::Result;
use super::common::CliContext;
use super::{ConfigAction, SandboxAction, PresetAction};
use crate::pz::ini::IniEditor;
use crate::pz::sandbox::SandboxEditor;

pub fn run(action: &ConfigAction, ctx: &CliContext) -> Result<()> {
    match action {
        ConfigAction::Show => show_ini(ctx),
        ConfigAction::Set { key, value } => set_ini(ctx, key, value),
        ConfigAction::Sandbox { action } => sandbox(ctx, action),
        ConfigAction::Preset { action } => preset(ctx, action),
    }
}

fn show_ini(ctx: &CliContext) -> Result<()> {
    let path = ctx.dirs.server_ini(&ctx.config);
    let ini = IniEditor::load(&path)?;
    print!("{}", ini.to_string());
    Ok(())
}

fn set_ini(ctx: &CliContext, key: &str, value: &str) -> Result<()> {
    let path = ctx.dirs.server_ini(&ctx.config);
    let mut ini = IniEditor::load(&path)?;
    let old = ini.get(key).map(str::to_owned);
    ini.set(key, value);
    ini.save(&path)?;
    if let Some(old) = old {
        println!("Updated {key}: {old} → {value}");
    } else {
        println!("Set {key}={value}");
    }
    Ok(())
}

fn sandbox(ctx: &CliContext, action: &SandboxAction) -> Result<()> {
    let path = ctx.dirs.sandbox_lua(&ctx.config);
    match action {
        SandboxAction::Show => {
            let s = SandboxEditor::load(&path)?;
            print!("{}", s.to_string());
        }
        SandboxAction::Set { key, value } => {
            let mut s = SandboxEditor::load(&path)?;
            s.set(key, value);
            s.save(&path)?;
            println!("Set {key} = {value}");
        }
    }
    Ok(())
}

fn preset(ctx: &CliContext, action: &PresetAction) -> Result<()> {
    match action {
        PresetAction::List => {
            let profiles = ctx.db.list_mod_profiles()?;
            if profiles.is_empty() {
                println!("No presets saved.");
            }
            for p in profiles {
                println!("  {p}");
            }
        }
        PresetAction::Save { name } => {
            let ini_path = ctx.dirs.server_ini(&ctx.config);
            let ini = IniEditor::load(&ini_path)?;
            let ids = ini.workshop_ids();
            let names = ini.mod_names();
            ctx.db.save_mod_profile(name, None, &ids, &names)?;
            println!("Saved preset '{name}' with {} mods.", ids.len());
        }
        PresetAction::Apply { name } => {
            if let Some((ids, names)) = ctx.db.get_mod_profile(name)? {
                let ini_path = ctx.dirs.server_ini(&ctx.config);
                let mut ini = IniEditor::load(&ini_path)?;
                ini.set_workshop_ids(&ids);
                ini.set_mod_names(&names);
                ini.save(&ini_path)?;
                println!("Applied preset '{name}' ({} mods). Restart the server to load.", ids.len());
            } else {
                anyhow::bail!("Preset '{name}' not found. Use `safehouse config preset list`.");
            }
        }
    }
    Ok(())
}
```

**Step 2: Implement `src/cli/mods.rs`**

```rust
use anyhow::Result;
use super::common::CliContext;
use super::{ModAction, ProfileAction};
use crate::pz::ini::IniEditor;
use crate::pz::mods::{add_mod_to_ini, remove_mod_from_ini, list_mods};
use crate::steam::fetch_mod_info;

pub async fn run(action: &ModAction, ctx: &CliContext) -> Result<()> {
    match action {
        ModAction::List => list(ctx),
        ModAction::Add { workshop_id, mod_name } => add(ctx, workshop_id, mod_name).await,
        ModAction::Remove { workshop_id } => remove(ctx, workshop_id),
        ModAction::Info { workshop_id } => info(ctx, workshop_id).await,
        ModAction::Profile { action } => profile(ctx, action),
    }
}

fn list(ctx: &CliContext) -> Result<()> {
    let ini_path = ctx.dirs.server_ini(&ctx.config);
    let ini = IniEditor::load(&ini_path)?;
    let mods = list_mods(&ini);
    if mods.is_empty() {
        println!("No mods installed.");
        return Ok(());
    }
    println!("{:<20} {}", "Workshop ID", "Mod Folder");
    println!("{}", "-".repeat(40));
    for (id, name) in &mods {
        // Try to show cached title
        let title = ctx.db.get_cached_mod(id)
            .ok()
            .flatten()
            .map(|m| m.title)
            .unwrap_or_else(|| name.clone());
        println!("{:<20} {} ({})", id, title, name);
    }
    Ok(())
}

async fn add(ctx: &CliContext, workshop_id: &str, mod_name: &str) -> Result<()> {
    let ini_path = ctx.dirs.server_ini(&ctx.config);
    let mut ini = IniEditor::load(&ini_path)?;
    add_mod_to_ini(&mut ini, workshop_id, mod_name);
    ini.save(&ini_path)?;

    // Fetch and cache metadata in background — best effort
    if let Ok(info) = fetch_mod_info(&ctx.http, workshop_id).await {
        println!("Added: {} ({})", info.title, workshop_id);
        let _ = ctx.db.upsert_workshop_mod(&info, Some(mod_name));
    } else {
        println!("Added: {workshop_id} ({mod_name}) [metadata fetch failed, will retry on next list]");
    }
    println!("Restart the server to load the new mod.");
    Ok(())
}

fn remove(ctx: &CliContext, workshop_id: &str) -> Result<()> {
    let ini_path = ctx.dirs.server_ini(&ctx.config);
    let mut ini = IniEditor::load(&ini_path)?;
    let existing_name = ini.workshop_ids()
        .into_iter()
        .zip(ini.mod_names().into_iter())
        .find(|(id, _)| id == workshop_id)
        .map(|(_, name)| name)
        .unwrap_or_default();
    remove_mod_from_ini(&mut ini, workshop_id, &existing_name);
    ini.save(&ini_path)?;
    println!("Removed {workshop_id} from mod list. Restart the server to apply.");
    Ok(())
}

async fn info(ctx: &CliContext, workshop_id: &str) -> Result<()> {
    let info = fetch_mod_info(&ctx.http, workshop_id).await?;
    println!("Title:   {}", info.title);
    println!("ID:      {}", info.workshop_id);
    if let Some(a) = &info.author { println!("Author:  {a}"); }
    if let Some(d) = &info.description {
        let preview: String = d.chars().take(200).collect();
        println!("Description: {preview}...");
    }
    Ok(())
}

fn profile(ctx: &CliContext, action: &ProfileAction) -> Result<()> {
    match action {
        ProfileAction::List => {
            for name in ctx.db.list_mod_profiles()? {
                println!("  {name}");
            }
        }
        ProfileAction::Save { name } => {
            let ini = IniEditor::load(&ctx.dirs.server_ini(&ctx.config))?;
            let ids = ini.workshop_ids();
            let names = ini.mod_names();
            ctx.db.save_mod_profile(name, None, &ids, &names)?;
            println!("Profile '{name}' saved ({} mods).", ids.len());
        }
        ProfileAction::Load { name } => {
            if let Some((ids, names)) = ctx.db.get_mod_profile(name)? {
                let mut ini = IniEditor::load(&ctx.dirs.server_ini(&ctx.config))?;
                ini.set_workshop_ids(&ids);
                ini.set_mod_names(&names);
                ini.save(&ctx.dirs.server_ini(&ctx.config))?;
                println!("Loaded profile '{name}' ({} mods). Restart server to apply.", ids.len());
            } else {
                anyhow::bail!("No profile named '{name}'.");
            }
        }
    }
    Ok(())
}
```

**Step 3: Build and smoke-test**

```bash
cargo build 2>&1 | tail -5
cargo run -- mods --help
cargo run -- config --help
```

**Step 4: Commit**

```bash
git add src/cli/config.rs src/cli/mods.rs
git commit -m "feat: CLI config and mods commands — view, set, presets, add/remove/list/profile"
```

---

## Task 16: CLI — Backup, Console, Setup, and Webhook Commands

**Files:**

- Modify: `src/cli/backup.rs`
- Modify: `src/cli/console.rs`
- Modify: `src/cli/setup.rs`
- Create: `src/cli/webhook.rs`

**Step 1: Implement `src/cli/backup.rs`**

```rust
use anyhow::{bail, Context, Result};
use super::common::CliContext;
use super::BackupAction;
use crate::backup::{create_snapshot, list_snapshots, restore_snapshot, prune_snapshots};
use crate::pz::rcon::RconClient;

pub async fn run(action: &BackupAction, ctx: &CliContext) -> Result<()> {
    match action {
        BackupAction::Create { label } => create(ctx, label.as_deref()).await,
        BackupAction::List => list(ctx),
        BackupAction::Restore { filename } => restore(ctx, filename).await,
        BackupAction::Prune { min_keep } => prune(ctx, *min_keep),
    }
}

async fn create(ctx: &CliContext, label: Option<&str>) -> Result<()> {
    // RCON save first if configured
    if ctx.config.backup_rcon_save {
        if let Ok(mut rcon) = RconClient::connect("127.0.0.1", ctx.config.rcon_port, &ctx.config.rcon_password) {
            println!("Saving world before backup...");
            let _ = rcon.send_command("save");
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    }

    let saves_dir = ctx.dirs.saves_dir(&ctx.config);
    if !saves_dir.exists() {
        bail!("World save directory not found: {}", saves_dir.display());
    }

    let snap = create_snapshot(&saves_dir, &ctx.dirs.backups_dir(), &ctx.config.server_name, label)?;
    let size = snap.metadata().map(|m| m.len()).unwrap_or(0);
    let filename = snap.file_name().unwrap_or_default().to_string_lossy().to_string();

    ctx.db.record_backup(&filename, label, Some(size as i64), &ctx.config.server_name, "cli")?;
    println!("Backup created: {filename} ({:.1} MB)", size as f64 / 1_048_576.0);
    Ok(())
}

fn list(ctx: &CliContext) -> Result<()> {
    let snaps = list_snapshots(&ctx.dirs.backups_dir())?;
    if snaps.is_empty() {
        println!("No backups found.");
        return Ok(());
    }
    for snap in snaps {
        let size = snap.metadata().map(|m| m.len()).unwrap_or(0);
        println!("{:<50} {:.1} MB", snap.file_name().unwrap_or_default().to_string_lossy(), size as f64 / 1_048_576.0);
    }
    Ok(())
}

async fn restore(ctx: &CliContext, filename: &str) -> Result<()> {
    let snap_path = ctx.dirs.backups_dir().join(filename);
    if !snap_path.exists() {
        bail!("Backup not found: {filename}");
    }
    // Ensure server is stopped first
    if crate::pz::detect::is_server_running(&ctx.dirs.pid_file()) {
        println!("Stopping server before restore...");
        crate::cli::server::run(&super::ServerAction::Stop, ctx).await?;
    }
    let saves_dir = ctx.dirs.saves_dir(&ctx.config);
    restore_snapshot(&snap_path, &saves_dir).context("restore failed")?;
    println!("Restored from {filename}. Start the server when ready.");
    Ok(())
}

fn prune(ctx: &CliContext, min_keep: usize) -> Result<()> {
    let pruned = prune_snapshots(&ctx.dirs.backups_dir(), ctx.config.backup_retention_days, min_keep)?;
    if pruned.is_empty() {
        println!("Nothing to prune.");
    } else {
        for p in &pruned {
            let fname = p.file_name().unwrap_or_default().to_string_lossy();
            println!("Deleted: {fname}");
            let _ = ctx.db.delete_backup_record(&fname);
        }
        println!("Pruned {} backup(s).", pruned.len());
    }
    Ok(())
}
```

**Step 2: Implement `src/cli/console.rs`**

```rust
use anyhow::{Context, Result};
use super::common::CliContext;
use super::ConsoleAction;
use crate::pz::rcon::RconClient;

pub async fn run(action: &ConsoleAction, ctx: &CliContext) -> Result<()> {
    let mut rcon = RconClient::connect("127.0.0.1", ctx.config.rcon_port, &ctx.config.rcon_password)
        .context("Cannot connect to RCON — is the server running?")?;
    match action {
        ConsoleAction::Chat { message } => {
            let r = rcon.send_command(&format!("servermsg \"{message}\""))?;
            println!("{r}");
        }
        ConsoleAction::Players => {
            let r = rcon.send_command("players")?;
            println!("{r}");
        }
        ConsoleAction::Kick { player } => {
            let r = rcon.send_command(&format!("kickuser \"{player}\""))?;
            println!("{r}");
        }
        ConsoleAction::Ban { player } => {
            let r = rcon.send_command(&format!("banuser \"{player}\""))?;
            println!("{r}");
        }
        ConsoleAction::Give { player, item } => {
            let r = rcon.send_command(&format!("additem \"{player}\" \"{item}\""))?;
            println!("{r}");
        }
        ConsoleAction::Save => {
            rcon.send_command("save")?;
            println!("World save triggered.");
        }
    }
    Ok(())
}
```

**Step 3: Implement `src/cli/setup.rs`**

```rust
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use crate::config::SafehouseConfig;
use crate::dirs::SafehouseDirs;

pub async fn run(install_dir: Option<&Path>, admin_password: Option<&str>) -> Result<()> {
    println!("=== Safehouse Setup ===");
    let dirs = SafehouseDirs::detect(None)?;
    dirs.ensure_dirs()?;

    let install = install_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs_next::home_dir().unwrap_or_default().join("pzserver"));

    println!("PZ install directory: {}", install.display());
    println!("Safehouse data dir:   {}", dirs.root.display());

    // Download PZ via SteamCMD
    install_pz_steamcmd(&install).await?;

    let password = admin_password.unwrap_or("changeme");
    let mut cfg = SafehouseConfig::default();
    cfg.server_install_dir = install.clone();

    let config_path = dirs.config_path();
    cfg.save(&config_path)?;

    println!("\nSetup complete!");
    println!("Edit {} to configure RCON password, server name, etc.", config_path.display());
    println!("Then run: safehouse server start");
    Ok(())
}

async fn install_pz_steamcmd(install_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(install_dir)?;
    println!("Installing Project Zomboid dedicated server via SteamCMD...");

    // Check steamcmd is available
    let steamcmd = which_steamcmd().context(
        "steamcmd not found. Install it: https://developer.valvesoftware.com/wiki/SteamCMD"
    )?;

    let status = tokio::process::Command::new(&steamcmd)
        .args([
            "+force_install_dir", &install_dir.to_string_lossy(),
            "+login", "anonymous",
            "+app_update", "380870", "validate",
            "+quit",
        ])
        .status()
        .await
        .context("steamcmd failed")?;

    if !status.success() {
        anyhow::bail!("steamcmd exited with status: {status}");
    }
    println!("PZ server installed.");
    Ok(())
}

fn which_steamcmd() -> Option<PathBuf> {
    for candidate in ["/usr/games/steamcmd", "/usr/bin/steamcmd", "steamcmd"] {
        let p = PathBuf::from(candidate);
        if p.exists() { return Some(p); }
    }
    // Try PATH
    std::process::Command::new("which").arg("steamcmd")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim().to_string()))
}
```

**Step 4: Implement `src/cli/webhook.rs`**

```rust
use anyhow::Result;
use super::common::CliContext;
use crate::notify::{notify, NotifyEvent};

pub async fn run(url: Option<&str>, test: bool, ctx: &CliContext) -> Result<()> {
    if let Some(u) = url {
        // Save to config
        let mut cfg = ctx.config.clone();
        cfg.discord_webhook_url = Some(u.to_string());
        cfg.save(&ctx.dirs.config_path())?;
        println!("Webhook URL saved.");
    }

    if test {
        // Use the newly-saved URL if one was provided, otherwise fall back to config
        let effective_url = url.or(ctx.config.discord_webhook_url.as_deref());
        notify(&ctx.http, effective_url, &ctx.config.server_name,
            NotifyEvent::ServerStarted).await?;
        println!("Test notification sent.");
    }
    Ok(())
}
```

**Step 5: Build**

```bash
cargo build 2>&1 | tail -5
```

Expected: `Finished dev`

**Step 6: Commit**

```bash
git add src/cli/
git commit -m "feat: CLI backup, console, setup, webhook commands"
```

---

## Task 17: Web Scaffold + Auth

**Files:**

- Create: `src/web/mod.rs`
- Create: `src/web/state.rs`
- Create: `src/web/handlers/mod.rs`
- Create: `src/web/handlers/auth.rs`
- Create: `src/web/handlers/dashboard.rs` (stub)
- Create: `templates/base.html`
- Create: `templates/login.html`
- Create: `src/cli/serve.rs`
- Modify: `src/db/users.rs`

> **Note:** `src/assets/` placeholder files must exist before `RustEmbed` compiles. Create them early:
>
> ```bash
> mkdir -p src/assets
> touch src/assets/style.css src/assets/htmx.min.js
> ```
>
> Real content is written in Task 20.

**Step 1: Implement `src/db/users.rs`**

```rust
use anyhow::Result;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
use rand::rngs::OsRng;
use rusqlite::params;
use crate::db::Database;

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
        let hash: Option<String> = self.conn.query_row(
            "SELECT password_hash FROM users WHERE username = ?1",
            params![username],
            |r| r.get(0),
        ).optional()?;

        let Some(hash) = hash else { return Ok(false) };
        let parsed = PasswordHash::new(&hash).map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
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

// Extension trait for optional query result
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
```

**Step 2: Create `templates/base.html`**

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Safehouse — {% block title %}{% endblock %}</title>
  <link rel="stylesheet" href="/static/style.css">
  <script src="/static/htmx.min.js" defer></script>
</head>
<body>
  {% block nav %}
  <nav class="nav">
    <span class="nav-brand">🏠 Safehouse</span>
    <a href="/">Dashboard</a>
    <a href="/config">Config</a>
    <a href="/mods">Mods</a>
    <a href="/backups">Backups</a>
    <a href="/console">Console</a>
    <a href="/logs">Logs</a>
    <a href="/logout" class="nav-right">Logout</a>
  </nav>
  {% endblock %}
  <main class="main">
    {% block content %}{% endblock %}
  </main>
</body>
</html>
```

**Step 3: Create `templates/login.html`**

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Safehouse — Login</title>
  <link rel="stylesheet" href="/static/style.css">
</head>
<body class="login-body">
  <div class="login-box">
    <h1>🏠 Safehouse</h1>
    {% match error %}
      {% when Some with (err) %}
        <p class="error">{{ err }}</p>
      {% when None %}
    {% endmatch %}
    <form method="post" action="/login">
      <label>Username <input type="text" name="username" autofocus></label>
      <label>Password <input type="password" name="password"></label>
      <button type="submit">Login</button>
    </form>
  </div>
</body>
</html>
```

**Step 4: Create `templates/dashboard.html`** (stub for now)

```html
{% extends "base.html" %}
{% block title %}Dashboard{% endblock %}
{% block content %}
<h2>Dashboard</h2>
<div class="status-card {% if running %}running{% else %}stopped{% endif %}">
  <span class="status-dot"></span>
  {% if running %}<strong>Running</strong>{% else %}<strong>Stopped</strong>{% endif %}
  — {{ server_name }}
</div>
{% endblock %}
```

**Step 5: Create `src/web/state.rs`**

```rust
use std::sync::Arc;
use parking_lot::Mutex;
use crate::config::SafehouseConfig;
use crate::db::Database;
use crate::dirs::SafehouseDirs;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub config: Arc<parking_lot::RwLock<SafehouseConfig>>,
    pub dirs: Arc<SafehouseDirs>,
    pub http: reqwest::Client,
}
```

**Step 6: Implement `src/web/handlers/mod.rs`**

```rust
pub mod auth;
pub mod dashboard;
// The following are added in Task 18:
// pub mod backups;
// pub mod configs;
// pub mod console;
// pub mod logs;
// pub mod mods;
```

> **Note:** Uncomment the additional handler modules in Task 18 when their files are created.

**Step 7: Implement login handler in `src/web/handlers/auth.rs`**

```rust
use actix_session::Session;
use actix_web::{get, post, web, HttpResponse, Responder};
use askama::Template;
use serde::Deserialize;

use crate::web::state::AppState;

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    error: Option<String>,
}

#[get("/login")]
pub async fn login_page(session: Session) -> impl Responder {
    if session.get::<String>("user").unwrap_or(None).is_some() {
        return HttpResponse::Found().insert_header(("Location", "/")).finish();
    }
    let tmpl = LoginTemplate { error: None };
    HttpResponse::Ok().content_type("text/html").body(tmpl.render().unwrap_or_default())
}

#[derive(Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
}

#[post("/login")]
pub async fn login_submit(
    form: web::Form<LoginForm>,
    session: Session,
    state: web::Data<AppState>,
) -> impl Responder {
    let db = state.db.lock();
    let ok = db.verify_password(&form.username, &form.password).unwrap_or(false);
    drop(db);

    if ok {
        let _ = session.insert("user", &form.username);
        HttpResponse::Found().insert_header(("Location", "/")).finish()
    } else {
        let tmpl = LoginTemplate { error: Some("Invalid credentials".to_string()) };
        HttpResponse::Unauthorized()
            .content_type("text/html")
            .body(tmpl.render().unwrap_or_default())
    }
}

#[get("/logout")]
pub async fn logout(session: Session) -> impl Responder {
    session.purge();
    HttpResponse::Found().insert_header(("Location", "/login")).finish()
}

/// Middleware helper: redirect to /login if no session user.
pub fn require_auth(session: &Session) -> Option<HttpResponse> {
    if session.get::<String>("user").ok().flatten().is_none() {
        Some(HttpResponse::Found().insert_header(("Location", "/login")).finish())
    } else {
        None
    }
}
```

**Step 7: Implement `src/web/mod.rs`** (minimal — expands in next tasks)

```rust
pub mod handlers;
pub mod state;

use std::sync::Arc;
use actix_session::config::PersistentSession;
use actix_session::storage::CookieSessionStore;
use actix_session::SessionMiddleware;
use actix_web::cookie::{time::Duration as CookieDuration, Key};
use actix_web::{middleware, web, App, HttpServer};
use anyhow::{Context, Result};
use parking_lot::Mutex;
use rust_embed::RustEmbed;
use actix_web::HttpResponse;

use crate::config::SafehouseConfig;
use crate::db::Database;
use crate::dirs::SafehouseDirs;
use state::AppState;

#[derive(RustEmbed)]
#[folder = "src/assets/"]
struct Assets;

async fn serve_static(path: web::Path<String>) -> HttpResponse {
    match Assets::get(&path) {
        Some(file) => {
            let ct = match path.rsplit('.').next() {
                Some("css") => "text/css",
                Some("js") => "application/javascript",
                _ => "application/octet-stream",
            };
            HttpResponse::Ok().content_type(ct).body(file.data.into_owned())
        }
        None => HttpResponse::NotFound().finish(),
    }
}

/// Start the web server and return a handle for graceful shutdown.
/// Caller should call `handle.stop(true)` to drain in-flight requests.
pub async fn run_server(
    bind: &str,
    port: u16,
    config: SafehouseConfig,
    dirs: SafehouseDirs,
    db: Database,
) -> Result<actix_web::dev::ServerHandle> {
    let key_bytes = config.session_key_bytes();
    let session_key = Key::from(&key_bytes);
    let state = web::Data::new(AppState {
        db: Arc::new(Mutex::new(db)),
        config: Arc::new(parking_lot::RwLock::new(config)),
        dirs: Arc::new(dirs),
        http: reqwest::Client::new(),
    });

    let addr = format!("{bind}:{port}");
    tracing::info!("Safehouse starting on http://{addr}");

    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(middleware::NormalizePath::trim())
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), session_key.clone())
                    .session_lifecycle(PersistentSession::default()
                        .session_ttl(CookieDuration::days(7)))
                    .cookie_secure(false) // safehouse runs on plain HTTP; set true behind TLS proxy
                    .build(),
            )
            .route("/static/{path:.*}", web::get().to(serve_static))
            // CSRF protection: require X-Requested-With header on all
            // state-mutating POST endpoints. Browsers will not send custom
            // headers cross-origin without a preflight CORS check, which
            // blocks cross-site form submissions. HTMX sends this header
            // automatically via hx-headers or htmx.config.requestHeaders.
            //
            // Implementation: add a middleware or per-handler guard that
            // rejects POST requests missing `X-Requested-With: XMLHttpRequest`.
            // Alternatively, use an actix-web CSRF crate.
            .service(handlers::auth::login_page)
            .service(handlers::auth::login_submit)
            .service(handlers::auth::logout)
            .service(handlers::dashboard::dashboard)
    })
    .bind(&addr)
    .with_context(|| format!("cannot bind to {addr}"))?
    .run();
    let handle = server.handle();

    // Spawn the server as a background task so the caller can orchestrate shutdown
    tokio::spawn(server);

    Ok(handle)
}
```

**Step 8: Implement `src/cli/serve.rs`**

```rust
use anyhow::{Context, Result};
use crate::config::SafehouseConfig;
use crate::db::Database;
use crate::dirs::SafehouseDirs;

pub async fn run(bind: Option<&str>, port: Option<u16>, cli: &super::Cli) -> Result<()> {
    let dirs = SafehouseDirs::detect(cli.data_dir.as_deref())?;
    dirs.ensure_dirs()?;

    let config_path = dirs.config_path();
    let mut config = SafehouseConfig::load(&config_path)?;

    if let Some(b) = bind { config.web_bind = b.to_string(); }
    if let Some(p) = port { config.web_port = p; }

    // Ensure admin user exists
    let db = Database::open(&dirs.db_path()).context("failed to open database")?;
    if !db.user_exists("admin")? {
        let password = rpassword::prompt_password("Set admin password: ")?;
        db.create_user("admin", &password)?;
        println!("Admin user created.");
    }

    config.ensure_session_secret();
    config.save(&config_path)?;

    let bind = config.web_bind.clone();
    let port = config.web_port;
    // Start web server (returns a handle for graceful shutdown)
    let server_handle = crate::web::run_server(&bind, port, config, dirs, db).await?;

    // Log watcher and signal handling are added in Task 19.
    // For now, wait until the server exits (Ctrl+C kills the process).
    tokio::signal::ctrl_c().await?;
    server_handle.stop(true).await;
    Ok(())
}
```

**Step 9: Build and verify**

```bash
cargo build 2>&1 | tail -10
cargo run -- serve --help
```

Expected: compiles, help shows serve options

**Step 10: Commit**

```bash
git add -A
git commit -m "feat: web scaffold — actix-web, sessions, auth, login/logout handlers"
```

---

## Task 18: Web Dashboard + All Management Pages

**Files:**

- Modify: `src/web/handlers/dashboard.rs`
- Create: `src/web/handlers/configs.rs`
- Create: `src/web/handlers/mods.rs`
- Create: `src/web/handlers/backups.rs`
- Create: `src/web/handlers/console.rs`
- Create: `src/web/handlers/logs.rs`
- Create: `templates/dashboard.html` (replace stub)
- Create: `templates/config.html`
- Create: `templates/mods.html`
- Create: `templates/backups.html`
- Create: `templates/console.html`
- Create: `templates/logs.html`
- Modify: `src/web/mod.rs` (register all routes)
- Modify: `src/web/handlers/mod.rs` (uncomment new handler modules)

> **Important:** Update `src/web/handlers/mod.rs` to declare all handler modules:
>
> ```rust
> pub mod auth;
> pub mod backups;
> pub mod configs;
> pub mod console;
> pub mod dashboard;
> pub mod logs;
> pub mod mods;
> ```

**Step 1: Dashboard handler**

```rust
// src/web/handlers/dashboard.rs
use actix_session::Session;
use actix_web::{get, web, HttpResponse, Responder};
use askama::Template;

use crate::pz::detect::is_server_running;
use crate::web::handlers::auth::require_auth;
use crate::web::state::AppState;

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    running: bool,
    server_name: String,
    player_count: Option<String>,
    recent_log: Vec<String>,
}

#[get("/")]
pub async fn dashboard(session: Session, state: web::Data<AppState>) -> impl Responder {
    if let Some(r) = require_auth(&session) { return r; }

    let (server_name, dirs, rcon_port, rcon_password) = {
        let cfg = state.config.read();
        (cfg.server_name.clone(), state.dirs.clone(), cfg.rcon_port, cfg.rcon_password.clone())
    };

    let running = is_server_running(&dirs.pid_file());

    let player_count = if running {
        // Use web::block to avoid blocking the tokio worker thread —
        // RconClient uses std::net::TcpStream with blocking I/O.
        let rcon_pass = rcon_password.clone();
        actix_web::web::block(move || {
            crate::pz::rcon::RconClient::connect("127.0.0.1", rcon_port, &rcon_pass)
                .ok()
                .and_then(|mut rcon| rcon.send_command("players").ok())
        }).await.ok().flatten()
    } else { None };

    let recent_log = dirs.latest_log(&state.config.read())
        .and_then(|p| crate::pz::logs::tail_lines(&p, 20).ok())
        .unwrap_or_default();

    let tmpl = DashboardTemplate { running, server_name, player_count, recent_log };
    HttpResponse::Ok().content_type("text/html").body(tmpl.render().unwrap_or_default())
}
```

**Step 2: Config handler**

```rust
// src/web/handlers/configs.rs
use actix_session::Session;
use actix_web::{get, post, web, HttpResponse, Responder};
use askama::Template;
use serde::Deserialize;
use crate::pz::ini::IniEditor;
use crate::web::{handlers::auth::require_auth, state::AppState};

#[derive(Template)]
#[template(path = "config.html")]
struct ConfigTemplate {
    ini_content: String,
    sandbox_content: String,
    message: Option<String>,
}

#[get("/config")]
pub async fn config_page(session: Session, state: web::Data<AppState>) -> impl Responder {
    if let Some(r) = require_auth(&session) { return r; }
    let cfg = state.config.read();
    let ini = IniEditor::load(&state.dirs.server_ini(&cfg)).map(|e| e.to_string()).unwrap_or_default();
    let sandbox = crate::pz::sandbox::SandboxEditor::load(&state.dirs.sandbox_lua(&cfg))
        .map(|e| e.to_string()).unwrap_or_default();
    drop(cfg);
    let tmpl = ConfigTemplate { ini_content: ini, sandbox_content: sandbox, message: None };
    HttpResponse::Ok().content_type("text/html").body(tmpl.render().unwrap_or_default())
}

#[derive(Deserialize)]
pub struct SetKeyForm { key: String, value: String, file: String }

#[post("/config/set")]
pub async fn config_set(form: web::Form<SetKeyForm>, session: Session, state: web::Data<AppState>) -> impl Responder {
    if let Some(r) = require_auth(&session) { return r; }
    let cfg = state.config.read();
    let result = match form.file.as_str() {
        "ini" => {
            let path = state.dirs.server_ini(&cfg);
            IniEditor::load(&path).and_then(|mut e| { e.set(&form.key, &form.value); e.save(&path) })
        }
        "sandbox" => {
            let path = state.dirs.sandbox_lua(&cfg);
            crate::pz::sandbox::SandboxEditor::load(&path)
                .and_then(|mut e| { e.set(&form.key, &form.value); e.save(&path) })
        }
        _ => Err(anyhow::anyhow!("unknown file")),
    };
    drop(cfg);
    if result.is_ok() {
        HttpResponse::Found().insert_header(("Location", "/config?saved=1")).finish()
    } else {
        HttpResponse::InternalServerError().body("Failed to update config")
    }
}
```

**Step 3: Mods handler**

```rust
// src/web/handlers/mods.rs  
use actix_session::Session;
use actix_web::{get, post, web, HttpResponse, Responder};
use askama::Template;
use serde::Deserialize;

use crate::pz::{ini::IniEditor, mods::{add_mod_to_ini, remove_mod_from_ini, list_mods}};
use crate::steam::fetch_mod_info;
use crate::web::{handlers::auth::require_auth, state::AppState};

#[derive(Template)]
#[template(path = "mods.html")]
struct ModsTemplate {
    mods: Vec<(String, String, String)>, // (workshop_id, folder_name, title)
    profiles: Vec<String>,
    message: Option<String>,
}

#[get("/mods")]
pub async fn mods_page(session: Session, state: web::Data<AppState>) -> impl Responder {
    if let Some(r) = require_auth(&session) { return r; }
    let cfg = state.config.read();
    let ini_path = state.dirs.server_ini(&cfg);
    drop(cfg);
    let ini = IniEditor::load(&ini_path).unwrap_or_else(|_| IniEditor::parse(""));
    let raw_mods = list_mods(&ini);
    let db = state.db.lock();
    let mods: Vec<(String, String, String)> = raw_mods.into_iter().map(|(id, name)| {
        let title = db.get_cached_mod(&id).ok().flatten().map(|m| m.title).unwrap_or_else(|| name.clone());
        (id, name, title)
    }).collect();
    let profiles = db.list_mod_profiles().unwrap_or_default();
    drop(db);
    let tmpl = ModsTemplate { mods, profiles, message: None };
    HttpResponse::Ok().content_type("text/html").body(tmpl.render().unwrap_or_default())
}

#[derive(Deserialize)]
pub struct AddModForm { workshop_id: String, mod_name: String }

#[post("/mods/add")]
pub async fn mods_add(form: web::Form<AddModForm>, session: Session, state: web::Data<AppState>) -> impl Responder {
    if let Some(r) = require_auth(&session) { return r; }
    let cfg = state.config.read();
    let ini_path = state.dirs.server_ini(&cfg);
    drop(cfg);
    if let Ok(mut ini) = IniEditor::load(&ini_path) {
        add_mod_to_ini(&mut ini, &form.workshop_id, &form.mod_name);
        let _ = ini.save(&ini_path);
    }
    // Fetch metadata best-effort
    let _ = fetch_mod_info(&state.http, &form.workshop_id).await.map(|info| {
        let db = state.db.lock();
        let _ = db.upsert_workshop_mod(&info, Some(&form.mod_name));
    });
    HttpResponse::Found().insert_header(("Location", "/mods")).finish()
}

#[derive(Deserialize)]
pub struct RemoveModForm { workshop_id: String, mod_name: String }

#[post("/mods/remove")]
pub async fn mods_remove(form: web::Form<RemoveModForm>, session: Session, state: web::Data<AppState>) -> impl Responder {
    if let Some(r) = require_auth(&session) { return r; }
    let cfg = state.config.read();
    let ini_path = state.dirs.server_ini(&cfg);
    drop(cfg);
    if let Ok(mut ini) = IniEditor::load(&ini_path) {
        remove_mod_from_ini(&mut ini, &form.workshop_id, &form.mod_name);
        let _ = ini.save(&ini_path);
    }
    HttpResponse::Found().insert_header(("Location", "/mods")).finish()
}
```

**Step 4: Backups handler**

```rust
// src/web/handlers/backups.rs
use actix_session::Session;
use actix_web::{get, post, web, HttpResponse, Responder};
use askama::Template;
use serde::Deserialize;

use crate::backup::{create_snapshot, list_snapshots};
use crate::web::{handlers::auth::require_auth, state::AppState};

#[derive(Template)]
#[template(path = "backups.html")]
struct BackupsTemplate {
    snapshots: Vec<(String, String)>, // (filename, size_human)
    message: Option<String>,
}

#[get("/backups")]
pub async fn backups_page(session: Session, state: web::Data<AppState>) -> impl Responder {
    if let Some(r) = require_auth(&session) { return r; }
    let snaps = list_snapshots(&state.dirs.backups_dir()).unwrap_or_default();
    let snapshots: Vec<(String, String)> = snaps.iter().map(|p| {
        let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
        let size = p.metadata().map(|m| format!("{:.1} MB", m.len() as f64 / 1_048_576.0)).unwrap_or_default();
        (name, size)
    }).collect();
    let tmpl = BackupsTemplate { snapshots, message: None };
    HttpResponse::Ok().content_type("text/html").body(tmpl.render().unwrap_or_default())
}

#[derive(Deserialize)]
pub struct CreateBackupForm { label: Option<String> }

#[post("/backups/create")]
pub async fn backup_create(form: web::Form<CreateBackupForm>, session: Session, state: web::Data<AppState>) -> impl Responder {
    if let Some(r) = require_auth(&session) { return r; }
    let label = form.into_inner().label.filter(|s| !s.is_empty());
    let (saves_dir, backup_dir, server_name) = {
        let cfg = state.config.read();
        (state.dirs.saves_dir(&cfg), state.dirs.backups_dir(), cfg.server_name.clone())
    };
    match create_snapshot(&saves_dir, &backup_dir, &server_name, label.as_deref()) {
        Ok(snap) => {
            let filename = snap.file_name().unwrap_or_default().to_string_lossy().to_string();
            let size = snap.metadata().map(|m| m.len() as i64).ok();
            let db = state.db.lock();
            let _ = db.record_backup(&filename, None, size, &server_name, "web");
            HttpResponse::Found().insert_header(("Location", "/backups?created=1")).finish()
        }
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}
```

**Step 5: Console + Logs handlers**

```rust
// src/web/handlers/console.rs
use actix_session::Session;
use actix_web::{get, post, web, HttpResponse, Responder};
use askama::Template;
use serde::Deserialize;
use crate::web::{handlers::auth::require_auth, state::AppState};

#[derive(Template)]
#[template(path = "console.html")]
struct ConsoleTemplate { output: Option<String> }

#[get("/console")]
pub async fn console_page(session: Session) -> impl Responder {
    if let Some(r) = require_auth(&session) { return r; }
    let tmpl = ConsoleTemplate { output: None };
    HttpResponse::Ok().content_type("text/html").body(tmpl.render().unwrap_or_default())
}

#[derive(Deserialize)]
pub struct RconForm { command: String }

#[post("/console/exec")]
pub async fn console_exec(form: web::Form<RconForm>, session: Session, state: web::Data<AppState>) -> impl Responder {
    if let Some(r) = require_auth(&session) { return r; }
    let (port, pass) = {
        let cfg = state.config.read();
        (cfg.rcon_port, cfg.rcon_password.clone())
    };
    // Use web::block to avoid blocking the tokio worker thread —
    // RconClient uses std::net::TcpStream with blocking I/O.
    let command = form.into_inner().command;
    let output = actix_web::web::block(move || {
        match crate::pz::rcon::RconClient::connect("127.0.0.1", port, &pass) {
            Ok(mut rcon) => rcon.send_command(&command).unwrap_or_else(|e| e.to_string()),
            Err(e) => format!("RCON error: {e}"),
        }
    }).await.unwrap_or_else(|e| format!("Internal error: {e}"));
    // HTMX partial response
    HttpResponse::Ok().content_type("text/plain").body(output)
}
```

```rust
// src/web/handlers/logs.rs
use actix_session::Session;
use actix_web::{get, web, HttpResponse, Responder};
use askama::Template;
use crate::web::{handlers::auth::require_auth, state::AppState};

#[derive(Template)]
#[template(path = "logs.html")]
struct LogsTemplate { lines: Vec<String> }

#[get("/logs")]
pub async fn logs_page(session: Session, state: web::Data<AppState>) -> impl Responder {
    if let Some(r) = require_auth(&session) { return r; }
    let cfg = state.config.read();
    let lines = state.dirs.latest_log(&cfg)
        .and_then(|p| crate::pz::logs::tail_lines(&p, 200).ok())
        .unwrap_or_default();
    drop(cfg);
    let tmpl = LogsTemplate { lines };
    HttpResponse::Ok().content_type("text/html").body(tmpl.render().unwrap_or_default())
}
```

**Step 6: Register all routes in `src/web/mod.rs`**

Add these services inside the `App::new()` builder, after the auth services:

```rust
.service(handlers::dashboard::dashboard)
.service(handlers::configs::config_page)
.service(handlers::configs::config_set)
.service(handlers::mods::mods_page)
.service(handlers::mods::mods_add)
.service(handlers::mods::mods_remove)
.service(handlers::backups::backups_page)
.service(handlers::backups::backup_create)
.service(handlers::console::console_page)
.service(handlers::console::console_exec)
.service(handlers::logs::logs_page)
```

**Step 7: Create remaining templates**

`templates/config.html`:

```html
{% extends "base.html" %}
{% block title %}Config{% endblock %}
{% block content %}
<h2>Server Config</h2>
{% match message %}
  {% when Some with (msg) %}<p class="error">{{ msg }}</p>{% when None %}
{% endmatch %}
<h3>server.ini</h3>
<pre>{{ ini_content }}</pre>
<form method="post" action="/config/set" style="margin-bottom:2rem">
  <input type="hidden" name="file" value="ini">
  <input type="text" name="key" placeholder="Key (e.g. MaxPlayers)" style="width:30%">
  <input type="text" name="value" placeholder="Value" style="width:30%">
  <button type="submit" class="btn btn-primary">Set</button>
</form>
<h3>SandboxVars.lua</h3>
<pre>{{ sandbox_content }}</pre>
<form method="post" action="/config/set">
  <input type="hidden" name="file" value="sandbox">
  <input type="text" name="key" placeholder="Key (e.g. Zombies.Speed)" style="width:30%">
  <input type="text" name="value" placeholder="Value" style="width:30%">
  <button type="submit" class="btn btn-primary">Set</button>
</form>
{% endblock %}
```

`templates/mods.html`:

```html
{% extends "base.html" %}
{% block title %}Mods{% endblock %}
{% block content %}
<h2>Workshop Mods</h2>
{% match message %}
  {% when Some with (msg) %}<p class="error">{{ msg }}</p>{% when None %}
{% endmatch %}
<form method="post" action="/mods/add" style="margin-bottom:1rem">
  <input type="text" name="workshop_id" placeholder="Workshop ID">
  <input type="text" name="mod_name" placeholder="Mod folder name">
  <button type="submit" class="btn btn-primary">Add Mod</button>
</form>
<table>
  <thead><tr><th>Workshop ID</th><th>Folder</th><th>Title</th><th></th></tr></thead>
  <tbody>
  {% for m in mods %}
    <tr>
      <td>{{ m.0 }}</td><td>{{ m.1 }}</td><td>{{ m.2 }}</td>
      <td><form method="post" action="/mods/remove" style="display:inline">
        <input type="hidden" name="workshop_id" value="{{ m.0 }}">
        <input type="hidden" name="mod_name" value="{{ m.1 }}">
        <button type="submit" class="btn btn-danger">Remove</button>
      </form></td>
    </tr>
  {% endfor %}
  </tbody>
</table>
<h3>Profiles</h3>
<ul>{% for p in profiles %}<li>{{ p }}</li>{% endfor %}</ul>
{% endblock %}
```

`templates/backups.html`:

```html
{% extends "base.html" %}
{% block title %}Backups{% endblock %}
{% block content %}
<h2>Backups</h2>
{% match message %}
  {% when Some with (msg) %}<p class="error">{{ msg }}</p>{% when None %}
{% endmatch %}
<form method="post" action="/backups/create" style="margin-bottom:1rem">
  <input type="text" name="label" placeholder="Label (optional)">
  <button type="submit" class="btn btn-primary">Create Backup</button>
</form>
<table>
  <thead><tr><th>Filename</th><th>Size</th></tr></thead>
  <tbody>
  {% for s in snapshots %}
    <tr><td>{{ s.0 }}</td><td>{{ s.1 }}</td></tr>
  {% endfor %}
  </tbody>
</table>
{% endblock %}
```

`templates/console.html`:

```html
{% extends "base.html" %}
{% block title %}Console{% endblock %}
{% block content %}
<h2>RCON Console</h2>
<form hx-post="/console/exec" hx-target="#output" hx-swap="innerHTML">
  <input type="text" name="command" placeholder="RCON command" autofocus style="width:60%">
  <button type="submit" class="btn btn-primary">Send</button>
</form>
<pre id="output">{% match output %}{% when Some with (text) %}{{ text }}{% when None %}{% endmatch %}</pre>
{% endblock %}
```

`templates/logs.html`:

```html
{% extends "base.html" %}
{% block title %}Logs{% endblock %}
{% block content %}
<h2>Server Logs</h2>
<pre>{% for line in lines %}{{ line }}
{% endfor %}</pre>
{% endblock %}
```

**Step 8: Build + smoke test**

```bash
cargo build --release 2>&1 | tail -5
# If config exists:
cargo run -- serve
# Navigate to http://localhost:9292
```

Expected: login page loads, dashboard renders after login

**Step 9: Commit**

```bash
git add -A
git commit -m "feat: web UI — dashboard, config editor, mods, backups, console, logs"
```

---

## Task 19: Wire Up Log Watcher, Signal Handling, and Discord Events in serve

In production `serve`, we want a background task that tails PZ logs, fires Discord notifications for player events, and handles graceful shutdown via SIGTERM/SIGINT.

**Files:**

- Modify: `src/cli/serve.rs`

> **Note:** `src/logging/mod.rs` was already implemented with real tracing initialization in Task 2 (not a stub).

**Step 1: Implement log watcher + signal handling in `src/cli/serve.rs`**

Add signal handling and a background task after the web server is started:

```rust
// Signal handling: graceful shutdown on SIGTERM/SIGINT
let shutdown = async {
    let mut sigterm = tokio::signal::unix::signal(
        tokio::signal::unix::SignalKind::terminate()
    ).expect("failed to register SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received SIGINT, shutting down...");
        }
        _ = sigterm.recv() => {
            tracing::info!("Received SIGTERM, shutting down...");
        }
    }
};

// After db setup, before calling run_server, spawn log watcher
let log_watcher_config = config.clone();
let log_watcher_dirs = dirs.clone();  // SafehouseDirs must be Clone
let http_client = reqwest::Client::new();

tokio::spawn(async move {
    let mut last_pos: u64 = 0;
    let mut last_log_path: Option<std::path::PathBuf> = None;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let Some(log_path) = log_watcher_dirs.latest_log(&log_watcher_config) else { continue };
        // Handle log file rotation: when PZ creates a new log file after restart,
        // seek to current end to avoid replaying historical events as Discord spam.
        if last_log_path.as_ref() != Some(&log_path) {
            last_pos = std::fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0);
            last_log_path = Some(log_path.clone());
            continue; // skip this tick — start watching from here on next iteration
        }
        let Ok(meta) = std::fs::metadata(&log_path) else { continue };
        if meta.len() <= last_pos { continue; }
        
        use std::io::{Read, Seek, SeekFrom};
        let Ok(mut f) = std::fs::File::open(&log_path) else { continue };
        let _ = f.seek(SeekFrom::Start(last_pos));
        let mut buf = String::new();
        let _ = f.read_to_string(&mut buf);
        last_pos = meta.len();
        
        for line in buf.lines() {
            if let Some(event) = crate::pz::logs::parse_log_line(line) {
                let notify_event = match event {
                    crate::pz::logs::PlayerEvent::Connected { ref name } => {
                        Some(crate::notify::NotifyEvent::PlayerJoined(name.clone()))
                    }
                    crate::pz::logs::PlayerEvent::Disconnected { ref name } => {
                        Some(crate::notify::NotifyEvent::PlayerLeft(name.clone()))
                    }
                };
                if let Some(ev) = notify_event {
                    let _ = crate::notify::notify(
                        &http_client,
                        log_watcher_config.discord_webhook_url.as_deref(),
                        &log_watcher_config.server_name,
                        ev,
                    ).await;
                }
            }
        }
    }
});

// Start web server and get the handle for graceful shutdown
let server_handle = crate::web::run_server(&bind, port, config, dirs, db).await?;

// Wait for shutdown signal, then gracefully drain in-flight requests
shutdown.await;
server_handle.stop(true).await;
```

> **Integration note:** The fragments above should be merged into the complete `serve.rs` from Task 17 Step 8. Replace the `tokio::signal::ctrl_c()` + `server_handle.stop(true)` block with the signal handler, log watcher spawn, and `shutdown.await` + `server_handle.stop(true)` from this task.

**Step 2: Final build check**

```bash
cargo build --release 2>&1 | tail -5
cargo test 2>&1 | tail -20
```

Expected: All tests pass, release build succeeds

**Step 3: Final commit**

```bash
git add -A
git commit -m "feat: background log watcher — Discord notifications on player events"
```

---

## Task 20: Minimal CSS + Download HTMX

**Files:**

- Create: `src/assets/style.css`
- Create: `src/assets/htmx.min.js`

**Step 1: Download HTMX**

```bash
curl -L https://unpkg.com/htmx.org@2.0.3/dist/htmx.min.js -o src/assets/htmx.min.js
```

**Step 2: Write `src/assets/style.css`** (minimal functional CSS)

```css
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
body { font-family: system-ui, sans-serif; background: #1a1a2e; color: #e0e0e0; }

.nav { display: flex; align-items: center; gap: 1rem; padding: .75rem 1.5rem;
       background: #16213e; border-bottom: 1px solid #0f3460; }
.nav-brand { font-weight: bold; font-size: 1.1rem; color: #e94560; margin-right: 1rem; }
.nav a { color: #b0b0c0; text-decoration: none; }
.nav a:hover { color: #e94560; }
.nav-right { margin-left: auto; }
.main { padding: 2rem; max-width: 1100px; margin: 0 auto; }

.login-body { display: flex; align-items: center; justify-content: center; min-height: 100vh; }
.login-box { background: #16213e; border-radius: 8px; padding: 2rem; width: 320px; }
.login-box h1 { text-align: center; margin-bottom: 1.5rem; color: #e94560; }
.login-box label { display: block; margin-bottom: 1rem; }
.login-box input { width: 100%; padding: .5rem; margin-top: .25rem;
                   background: #0f3460; border: 1px solid #444; color: #e0e0e0;
                   border-radius: 4px; }
.login-box button { width: 100%; padding: .6rem; background: #e94560;
                    color: white; border: none; border-radius: 4px; cursor: pointer; }

.status-card { padding: 1rem 1.5rem; border-radius: 8px; background: #16213e;
               border-left: 4px solid #888; margin-bottom: 1.5rem; }
.status-card.running { border-color: #00cc66; }
.status-card.stopped { border-color: #e94560; }

table { width: 100%; border-collapse: collapse; background: #16213e; border-radius: 8px; }
th, td { padding: .6rem 1rem; text-align: left; border-bottom: 1px solid #0f3460; }
th { color: #888; font-weight: 500; }

.btn { padding: .4rem .9rem; border: none; border-radius: 4px; cursor: pointer; font-size: .9rem; }
.btn-primary { background: #e94560; color: white; }
.btn-danger  { background: #cc3333; color: white; }
.btn-secondary { background: #0f3460; color: #e0e0e0; }

input, textarea, select { background: #0f3460; border: 1px solid #444; color: #e0e0e0;
                          border-radius: 4px; padding: .4rem .6rem; }
textarea { width: 100%; font-family: monospace; font-size: .85rem; }
.error { color: #e94560; margin-bottom: 1rem; }
pre { background: #0d0d1a; padding: 1rem; border-radius: 4px; font-size: .82rem;
      overflow-x: auto; max-height: 400px; }
```

**Step 3: Verify assets are embedded in binary**

```bash
cargo build --release
# Check binary doesn't need external CSS/JS files:
file target/release/safehouse
ls -lh target/release/safehouse
```

Expected: Single self-contained binary (assets baked in via RustEmbed)

**Step 4: Commit**

```bash
git add src/assets/
git commit -m "feat: static assets — minimal CSS and HTMX embedded in binary"
```

---

## Task 21: README

**Files:**

- Create: `README.md`

**Step 1: Write `README.md`**

```markdown
# Safehouse

A single-binary Project Zomboid dedicated server manager with CLI and embedded web UI.

## Features

- **Server lifecycle** — start, stop, restart, status, log tailing
- **Mod management** — add/remove Steam Workshop mods, named profiles
- **World backups** — create/restore/prune `.tar.gz` snapshots
- **Config editing** — comment-preserving `server.ini` and `SandboxVars.lua` editors
- **RCON console** — send admin commands (kick, ban, chat, save)
- **Web UI** — embedded HTMX dashboard with auth, config, mods, backups, console, logs
- **Discord notifications** — player join/leave, server start/stop, backup events

## Quick Start

```bash
# Install (requires Rust toolchain)
cargo install --path .

# Initial setup (downloads PZ server via SteamCMD)
safehouse setup --install-dir ~/pzserver

# Edit config
vim ~/.local/share/safehouse/safehouse.toml

# Start server
safehouse server start

# Start web UI
safehouse serve
```

## Security Notes

- The RCON password is stored in plaintext in `safehouse.toml` and passed as a CLI argument to the PZ server binary. This is inherent to how PZ works. Set `chmod 600` on `safehouse.toml`.
- The web UI binds to `0.0.0.0` by default. Put it behind a reverse proxy with TLS for network-accessible deployments.
- Session cookies are signed but transmitted over plain HTTP unless you use a TLS reverse proxy.

## License

AGPL-3.0-only

```

**Step 2: Commit**

```bash
git add README.md
git commit -m "docs: add README with features, quick start, security notes"
```

---

## Final Validation

**Run all tests:**

```bash
cargo test 2>&1
```

Expected: All unit tests pass (pz::ini, pz::sandbox, pz::rcon, pz::logs, pz::detect, pz::mods, backup::, config::, notify::, steam::, db::)

**Build release binary:**

```bash
cargo build --release
ls -lh target/release/safehouse
```

Expected: Single binary, roughly 20-35 MB

**Smoke test CLI:**

```bash
./target/release/safehouse --help
./target/release/safehouse server --help
./target/release/safehouse mods --help
./target/release/safehouse backup --help
./target/release/safehouse console --help
```

**Integration test checklist** (requires a PZ server):

```
[ ] safehouse setup --install-dir ~/pzserver
[ ] safehouse server start
[ ] safehouse server status  → shows Running
[ ] safehouse mods add 2392987220 BritasWeaponPack
[ ] safehouse mods list      → shows the mod with title
[ ] safehouse backup create --label "before-mods"
[ ] safehouse backup list    → shows snapshot
[ ] safehouse console players  → lists connected players
[ ] safehouse console chat "Hello from safehouse"
[ ] safehouse serve          → web UI accessible at localhost:9292
```

**Final commit:**

```bash
git add -A
git commit -m "chore: complete safehouse v0.1.0 implementation"
git tag v0.1.0
```
