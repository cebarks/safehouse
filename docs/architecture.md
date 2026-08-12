# Architecture

Safehouse is a single Rust binary that combines a CLI (clap) with an embedded web server (actix-web). All state lives in a SQLite database and a TOML config file.

## Module Map

```
src/
├── main.rs              # Entry point — clap parse → command dispatch
├── lib.rs               # Crate root — module declarations, #![deny(clippy::unwrap_used)]
│
├── config.rs            # SafehouseConfig — TOML serialization, defaults, session secret
├── dirs.rs              # SafehouseDirs — path resolution for all disk locations
├── backup.rs            # Backup engine — tar.gz create/restore/list/prune
├── notify.rs            # Discord webhook — event types, payload builder, sender
│
├── logging/
│   └── mod.rs           # Tracing initialization (env-filter, verbosity levels)
│
├── db/
│   ├── mod.rs           # Database struct (rusqlite Connection), open/migrate
│   ├── schema.rs        # Migration runner (user_version pragma)
│   ├── users.rs         # User auth (argon2 hash/verify)
│   ├── mods.rs          # Workshop mod cache, mod profiles
│   ├── backups.rs       # Backup snapshot records
│   └── players.rs       # Player session tracking
│
├── pz/
│   ├── mod.rs           # PZ module declarations
│   ├── detect.rs        # Binary detection, PID file, flock, is_running
│   ├── ini.rs           # IniEditor — comment-preserving server.ini parser
│   ├── sandbox.rs       # SandboxEditor — Lua nested table parser (dotted keys)
│   ├── rcon.rs          # Source RCON protocol client (TCP)
│   ├── logs.rs          # PZ log parser (player connect/disconnect regex)
│   └── mods.rs          # Mod list helpers (add/remove from both ini lists)
│
├── steam/
│   ├── mod.rs           # Re-exports
│   └── workshop.rs      # Steam Workshop API client (GetPublishedFileDetails)
│
├── cli/
│   ├── mod.rs           # Cli struct, Command enum, all subcommand enums
│   ├── common.rs        # CliContext (config + dirs + db + http client)
│   ├── setup.rs         # SteamCMD install, config creation
│   ├── server.rs        # start/stop/restart/status/logs
│   ├── config.rs        # show/set ini/sandbox, presets
│   ├── mods.rs          # list/add/remove/info, profiles
│   ├── backup.rs        # create/list/restore/prune
│   ├── console.rs       # RCON command dispatch
│   ├── webhook.rs       # URL config, test notification
│   └── serve.rs         # Web server startup, log watcher, signal handling
│
├── web/
│   ├── mod.rs           # actix-web server setup, RustEmbed static files, route registration
│   ├── state.rs         # AppState (shared DB, config, dirs, http client)
│   └── handlers/
│       ├── mod.rs       # Handler module declarations
│       ├── auth.rs      # Login/logout, session management, require_auth guard
│       ├── dashboard.rs # Server status, player count, recent logs
│       ├── configs.rs   # INI/sandbox viewer and editor
│       ├── mods.rs      # Mod list, add/remove, profiles
│       ├── backups.rs   # Backup list and create
│       ├── console.rs   # RCON console (HTMX partial responses)
│       └── logs.rs      # Log tail viewer
│
└── assets/
    ├── style.css        # Dark theme CSS (embedded via RustEmbed)
    └── htmx.min.js      # HTMX 2.0.3 (embedded via RustEmbed)

templates/                # Askama HTML templates (compiled at build time)
├── base.html            # Layout with nav bar
├── login.html           # Standalone login page
├── dashboard.html       # Server status dashboard
├── config.html          # Config editor
├── mods.html            # Mod manager
├── backups.html         # Backup manager
├── console.html         # RCON console
└── logs.html            # Log viewer

migrations/
└── 001_initial.sql      # SQLite schema (users, workshop_mods, mod_profiles,
                         #   backup_snapshots, player_sessions)
```

## Key Design Decisions

### Single Binary

All static assets (CSS, HTMX JS) are embedded via `rust-embed` and templates are compiled via `askama`. The SQLite database uses the `bundled` feature. The result is a single binary with zero runtime dependencies.

### Comment-Preserving Parsers

PZ server admins heavily annotate their `server.ini` and `SandboxVars.lua` files. The `IniEditor` and `SandboxEditor` operate on raw lines, preserving comments, blank lines, and formatting. Only the targeted key=value line is modified.

### PID File Locking

Server lifecycle uses `flock(LOCK_EX | LOCK_NB)` on the PID file to prevent concurrent starts. The lock is held by the safehouse process that spawned the server. If safehouse crashes, the lock is automatically released by the OS.

### RCON for Graceful Shutdown

`server stop` uses RCON (`save` → `quit`) rather than SIGTERM directly. This ensures the world is saved before shutdown. SIGTERM is the fallback if RCON is unavailable, with SIGKILL as the last resort after 40 seconds.

### Blocking RCON in Web Handlers

The `RconClient` uses `std::net::TcpStream` (blocking I/O). Web handlers wrap RCON calls in `actix_web::web::block()` to avoid blocking the tokio runtime's worker threads.

### Session Security

- Passwords: Argon2id (via the `argon2` crate)
- Sessions: signed cookies with a 64-byte hex secret
- Cookie `Secure` flag is `false` by default (safehouse runs on plain HTTP); set to `true` when behind a TLS reverse proxy

## Database Schema

```
users              — web UI authentication
workshop_mods      — Steam Workshop metadata cache
mod_profiles       — named mod collection presets (JSON arrays)
backup_snapshots   — backup file records
player_sessions    — player join/leave tracking
```

All tables use `datetime('now')` for timestamps and `INTEGER PRIMARY KEY AUTOINCREMENT` for IDs. The database runs in WAL mode with a 5-second busy timeout.

## Tech Stack

| Component | Crate | Purpose |
| ----------- | ------- | --------- |
| CLI | clap 4 (derive) | Command parsing |
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
| Locking | parking_lot 0.12 | Mutex/RwLock for shared state |
