# Safehouse

A Project Zomboid dedicated server manager with CLI and embedded web UI.

Built in Rust — one `safehouse` binary handles server lifecycle, mod management, world backups, config editing, RCON admin commands, Discord notifications, and a full web dashboard. The PZ server runs inside a podman container for clean process management and signal handling.

## Features

- **Server lifecycle** — start, stop, restart, status, real-time log tailing with `--follow`
- **Container-managed** — PZ runs in a podman container (fedora-minimal + steamcmd); clean PID 1 signal propagation, no orphan processes
- **Mod management** — add/remove Steam Workshop mods by ID, named mod profiles for quick switching
- **World backups** — create/restore/prune `.tar.gz` snapshots with automatic RCON save-before-backup
- **Config editing** — comment-preserving editors for `server.ini` and `SandboxVars.lua` (dotted key support for nested Lua tables)
- **RCON console** — send admin commands: kick, ban, chat, save, give items, list players
- **Web UI** — embedded HTMX dashboard with argon2 auth, config editor, mod manager, backup manager, RCON console, log viewer
- **Discord notifications** — player join/leave, server start/stop, backup completion events via webhook

## Quick Start

```bash
# Build from source (requires Rust 1.70+)
cargo install --path .

# Build the container image (requires podman)
podman build -t safehouse-pz -f Containerfile .

# Initial setup — downloads PZ dedicated server via SteamCMD
safehouse setup --install-dir ~/pzserver

# Edit configuration (set rcon_password at minimum)
vim ~/.local/share/safehouse/safehouse.toml

# Start the PZ server (runs in a container)
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
- **Podman** with an active user socket (`systemctl --user enable --now podman.socket`)
- The PZ dedicated server is downloaded automatically via SteamCMD inside the container

## Security Notes

- The RCON password is stored in plaintext in `safehouse.toml` and written to `server.ini` before each server start. Set `chmod 600` on `safehouse.toml`.
- The web UI binds to `0.0.0.0:9292` by default. For network-accessible deployments, put it behind a reverse proxy with TLS (see [Deployment](docs/deployment.md)).
- RCON is bound to `127.0.0.1` inside the container port mapping — not exposed to the network.
- Session cookies are signed with a 64-byte secret (auto-generated on first `serve`) but transmitted over plain HTTP unless you use a TLS proxy.
- Web UI passwords are hashed with Argon2id.

## AI Disclaimer

The architecture, design, and implementation plan for this project were created by a human. Code implementation was generated with assistance from large language models (LLMs) and subsequently reviewed. While the code compiles, passes tests, and has been audited, use discretion before running in production — especially the security-sensitive components (auth, session management, RCON credential handling).

## License

AGPL-3.0-only
