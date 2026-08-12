# Safehouse

A single-binary Project Zomboid dedicated server manager with CLI and embedded web UI.

Built in Rust — one `safehouse` binary handles server lifecycle, mod management, world backups, config editing, RCON admin commands, Discord notifications, and a full web dashboard. All static assets (CSS, HTMX) are baked into the binary; the only runtime dependency is SQLite (bundled).

## Features

- **Server lifecycle** — start, stop, restart, status, real-time log tailing with `--follow`
- **Mod management** — add/remove Steam Workshop mods by ID, named mod profiles for quick switching
- **World backups** — create/restore/prune `.tar.gz` snapshots with automatic RCON save-before-backup
- **Config editing** — comment-preserving editors for `server.ini` and `SandboxVars.lua` (dotted key support for nested Lua tables)
- **RCON console** — send admin commands: kick, ban, chat, save, give items, list players
- **Web UI** — embedded HTMX dashboard with argon2 auth, config editor, mod manager, backup manager, RCON console, log viewer
- **Discord notifications** — player join/leave, server start/stop, backup completion events via webhook
- **Single binary** — no containers, no external services, no runtime dependencies

## Quick Start

```bash
# Build from source (requires Rust 1.70+)
cargo install --path .

# Initial setup — downloads PZ dedicated server via SteamCMD
safehouse setup --install-dir ~/pzserver

# Edit configuration
vim ~/.local/share/safehouse/safehouse.toml

# Start the PZ server
safehouse server start

# Check status
safehouse server status

# Start the web UI (prompts for admin password on first run)
safehouse serve
# → http://localhost:9292
```

## Documentation

| Document | Description |
| ---------- | ------------- |
| [Configuration](docs/configuration.md) | `safehouse.toml` reference, directory layout, environment variables |
| [CLI Reference](docs/cli-reference.md) | Complete command reference with examples |
| [Web UI](docs/web-ui.md) | Web dashboard setup, authentication, page reference |
| [Mod Management](docs/mod-management.md) | Workshop mods, profiles, bulk operations |
| [Backups](docs/backups.md) | Snapshot lifecycle, restore, retention policies |
| [Discord Integration](docs/discord.md) | Webhook setup and event reference |
| [Architecture](docs/architecture.md) | Codebase structure, module map, design decisions |
| [Deployment](docs/deployment.md) | Production setup, reverse proxy, systemd service |

## Requirements

- **Linux** (x86_64) — PZ dedicated server is Linux-only
- **Rust 1.70+** for building
- **SteamCMD** for initial PZ server installation (`safehouse setup` will prompt if missing)
- The PZ dedicated server itself (free, no Steam account required — anonymous login)

## Security Notes

- The RCON password is stored in plaintext in `safehouse.toml` and passed as a CLI argument to the PZ server binary. This is inherent to how Project Zomboid works. Set `chmod 600` on `safehouse.toml`.
- The web UI binds to `0.0.0.0:9292` by default. For network-accessible deployments, put it behind a reverse proxy with TLS (see [Deployment](docs/deployment.md)).
- Session cookies are signed with a 64-byte secret (auto-generated on first `serve`) but transmitted over plain HTTP unless you use a TLS proxy.
- Web UI passwords are hashed with Argon2id.

## AI Disclaimer

The architecture, design, and implementation plan for this project were created by a human. Code implementation was generated with assistance from large language models (LLMs) and subsequently reviewed. While the code compiles, passes tests, and has been audited, use discretion before running in production — especially the security-sensitive components (auth, session management, RCON credential handling).

## License

AGPL-3.0-only
