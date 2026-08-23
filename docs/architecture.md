# Architecture

Safehouse is a Rust binary that manages a Project Zomboid dedicated server running inside a podman container. It communicates with the server via RCON and with podman via the bollard API over the local socket.

## Module Map

```
src/
├── main.rs              # Entry point — clap parse → command dispatch
├── lib.rs               # Crate root — module declarations, #![deny(clippy::unwrap_used)]
│
├── config.rs            # SafehouseConfig — TOML serialization, defaults, session secret
├── dirs.rs              # SafehouseDirs — path resolution for all disk locations
├── container.rs         # Podman container lifecycle via bollard (create/start/stop/logs)
├── backup.rs            # Backup engine — tar.gz create/restore/list/prune
├── notify.rs            # Discord webhook notifications
│
├── cli/
│   ├── mod.rs           # Clap command/subcommand definitions
│   ├── common.rs        # CliContext (shared config, dirs, db handle)
│   ├── setup.rs         # Initial PZ install via SteamCMD
│   ├── server.rs        # Server lifecycle — start/stop/restart/status/logs via container
│   ├── config.rs        # INI/sandbox show/set, config presets
│   ├── mods.rs          # Workshop mod add/remove, profiles
│   ├── backup.rs        # Backup create/restore/list/prune
│   ├── console.rs       # RCON commands (players, chat, kick, ban, give, save)
│   ├── webhook.rs       # Discord webhook setup/test
│   └── serve.rs         # Web server startup, log watcher, signal handling
│
├── db/
│   ├── mod.rs           # Database struct, SQLite open, migration runner
│   ├── users.rs         # User auth (argon2 hash/verify)
│   ├── mods.rs          # Workshop mod cache, mod profiles
│   ├── backups.rs       # Backup snapshot records
│   └── players.rs       # Player session tracking
│
├── pz/
│   ├── mod.rs           # PZ module declarations
│   ├── case_fix.rs      # Lowercase symlink fixer for case-sensitive Linux filesystems
│   ├── detect.rs        # Binary detection, PID utilities
│   ├── ini.rs           # IniEditor — comment-preserving server.ini parser
│   ├── sandbox.rs       # SandboxEditor — Lua nested table parser (dotted keys)
│   ├── rcon.rs          # Source RCON protocol client (TCP)
│   ├── logs.rs          # Log file parser (player connect/disconnect events)
│   └── mods.rs          # Mod list sync (workshop IDs ↔ mod folder names)
│
├── steam/
│   └── workshop.rs      # Steam Workshop API client (mod metadata)
│
├── web/
│   ├── mod.rs           # actix-web server setup, static files, route registration
│   ├── state.rs         # AppState (shared DB, config, dirs, http client)
│   └── handlers/
│       ├── mod.rs       # Handler module declarations
│       ├── auth.rs      # Login/logout, session management, require_auth guard
│       ├── dashboard.rs # Server status, player count, recent logs
│       ├── configs.rs   # INI/sandbox viewer and editor
│       ├── mods.rs      # Mod list, add/remove, profiles
│       ├── backups.rs   # Backup create/restore/list
│       ├── console.rs   # RCON web console
│       └── logs.rs      # Log viewer
│
├── logging/             # tracing-subscriber setup
└── assets/              # Embedded CSS, HTMX JS (via rust-embed)

templates/               # Askama HTML templates
├── base.html            # Layout with nav, HTMX script
├── login.html           # Auth page
├── dashboard.html       # Server overview
├── config.html          # INI/sandbox editor
├── mods.html            # Mod manager
├── backups.html         # Backup manager
├── console.html         # RCON console
└── logs.html            # Log viewer

migrations/
└── 001_initial.sql      # SQLite schema (users, workshop_mods, mod_profiles,
                         #   backup_snapshots, player_sessions)

Containerfile            # fedora-minimal:44 + steamcmd + PZ runtime deps
```

## Key Design Decisions

### Container-Managed Server

The PZ server runs inside a podman container (`safehouse-pz`) managed via the bollard crate (async podman/Docker API). This solves several problems:

- **PID 1 = Java process** — `ProjectZomboid64` runs as PID 1 in the container, so SIGTERM reaches it directly. No shell wrapper, no orphan processes.
- **Environment isolation** — `LD_LIBRARY_PATH`, `PATH`, and JRE setup are baked into the container image `ENV` directives, replacing the fragile `start-server.sh` wrapper.
- **Log capture** — container stdout/stderr is captured by podman and accessible via `podman logs` / bollard's log stream API.
- **Clean lifecycle** — container create/start/stop/remove replaces PID file tracking, `flock`, and `/proc` probing.

The `-cachedir=/zomboid` flag tells PZ to use the volume-mounted data directory instead of Java's `user.home` (which defaults to `/root` inside the container).

### RCON for Graceful Shutdown

`server stop` uses RCON `save` + `quit` for graceful world-saving shutdown. If RCON fails (server crashed, password wrong), safehouse falls back to `podman stop` which sends SIGTERM to PID 1.

### Comment-Preserving Parsers

`IniEditor` and `SandboxEditor` parse PZ config files line-by-line and preserve comments, blank lines, and ordering. This avoids stripping the extensive inline documentation PZ generates in `server.ini`.

### Session Security

Web UI sessions use a 64-byte random secret (auto-generated, stored in `safehouse.toml`) for cookie signing via `actix-session`. Passwords are hashed with Argon2id.

### Case-Sensitivity Fix

PZ lowercases file paths internally (Windows heritage). On Linux's case-sensitive filesystems, this breaks mod loading when files have uppercase names. `case_fix.rs` walks the server install directory and creates relative lowercase symlinks alongside any entry with uppercase ASCII characters — e.g., `animsets -> AnimSets`. The fix runs automatically before container creation and after SteamCMD installs, plus on-demand via `safehouse mods fix-case`. Uses `symlink_metadata` (lstat) instead of `Path::exists()` to correctly handle dangling symlinks at target paths.

## Database Schema

SQLite with bundled `libsqlite3` (no system dependency). Single migration:

- **users** — username, argon2 password hash
- **workshop_mods** — cached mod metadata from Steam API
- **mod_profiles** — named mod list snapshots
- **backup_snapshots** — backup filename, size, timestamp
- **player_sessions** — join/leave timestamps for Discord notifications

## Tech Stack

| Component | Crate | Purpose |
| ----------- | ------- | --------- |
| CLI | clap 4 (derive) | Command parsing |
| Container API | bollard 0.21 | Podman/Docker container management |
| Web server | actix-web 4 | HTTP server |
| Templates | askama 0.12 | Compile-time HTML templates |
| Database | rusqlite 0.32 (bundled) | SQLite with bundled libsqlite3 |
| HTTP client | reqwest 0.12 (rustls-tls) | Steam API, Discord webhooks |
| Async runtime | tokio 1 (full) | Async I/O, process spawning, signals |
| Password hashing | argon2 0.5 | Argon2id password hashing |
| Sessions | actix-session 0.10 | Cookie-based sessions |
| Static assets | rust-embed 8 | Compile-time asset embedding |
| Compression | flate2 + tar | Backup archives |
| Interactivity | HTMX 2.0 | Web UI dynamic updates |
| Logging | tracing + tracing-subscriber | Structured logging |
