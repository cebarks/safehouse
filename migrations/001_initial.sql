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
    mod_folder_name TEXT,
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
    workshop_ids    TEXT    NOT NULL DEFAULT '[]',
    mod_names       TEXT    NOT NULL DEFAULT '[]',
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
    created_by      TEXT    NOT NULL DEFAULT 'cli'
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
