# Web UI

The safehouse web UI is an embedded HTMX dashboard served by actix-web. All static assets (CSS, JS) are compiled into the binary — no external CDN or build step required.

## Starting the Web UI

```bash
safehouse serve
safehouse serve --bind 127.0.0.1 --port 8080
```

On first run, you'll be prompted to set an admin password. The web UI is then available at `http://localhost:9292` (default).

The `serve` command also starts a background log watcher for Discord notifications (see below).

## Pages

### Dashboard (`/`)

Server overview: running status, player count, server name, uptime. Data is fetched via RCON when the server is running.

### Config (`/config`)

View and edit `server.ini` and `SandboxVars.lua` through the browser. Changes are written with the comment-preserving editor — existing comments and formatting are retained.

Restart the server after changing config values.

### Mods (`/mods`)

Add, remove, and list Steam Workshop mods. Shows cached mod metadata (name, description) from the Steam API. Supports mod profiles for quick switching between mod sets.

### Backups (`/backups`)

Create, list, and restore world backups. Shows file size and timestamp for each snapshot.

### Console (`/console`)

Send RCON commands to the running server: chat, kick, ban, give items, save. Shows command output.

### Logs (`/logs`)

View recent PZ server log output.

## Authentication

The web UI uses cookie-based sessions with a 64-byte signed secret. Passwords are hashed with Argon2id.

- Session secret is auto-generated on first `safehouse serve` and saved to `safehouse.toml`
- Default bind is `0.0.0.0` — use a reverse proxy with TLS for production (see [Deployment](deployment.md))
- Only one admin user is created; multi-user support is not implemented

## Background Log Watcher

When `safehouse serve` is running, a background task tails the PZ server log every 2 seconds and parses player connect/disconnect events. These trigger Discord webhook notifications if configured.

The watcher handles log file rotation — when PZ creates a new log file after a restart, it resets to the current end of the new file to avoid replaying old events.

## Customization

| Setting | How to change |
| --------- | --------------- |
| Port | `--port` flag or `web_port` in `safehouse.toml` |
| Bind address | `--bind` flag or `web_bind` in `safehouse.toml` |
| Admin password | Delete `safehouse.db` and restart `serve` to re-create |
| TLS | Use a reverse proxy (nginx, caddy) — see [Deployment](deployment.md) |
