# Safehouse — Agent Instructions

Project Zomboid dedicated server manager. Rust single-crate binary with CLI + embedded web UI.
PZ server runs in a podman container managed via bollard.

## Build & Test

- `cargo clippy` before committing — crate enforces `#![deny(clippy::unwrap_used)]` in `lib.rs`.
  Use `.expect("reason")` or `?` with `anyhow`, never `.unwrap()`.
- Known clippy warnings (non-blocking): `await_holding_lock` in `web/handlers/mods.rs` and
  `field_assignment_outside_of_initializer` — these predate the current work.
- `cargo test` runs 106 unit tests (all in-module `#[cfg(test)]`). No integration test harness yet.

## Architecture

See `docs/architecture.md` for the full module map and design decisions — it's kept current.

Key non-obvious points:

- **Comment-preserving parsers** — `pz/ini.rs` (IniEditor) and `pz/sandbox.rs` (SandboxEditor)
  parse PZ config files line-by-line to preserve comments and ordering. Do NOT replace these
  with serde or a generic TOML/INI crate — PZ's inline documentation in `server.ini` matters.
- **Container lifecycle** — `container.rs` manages the PZ server via bollard's async API.
  The `Containerfile` sets `ENTRYPOINT ["./ProjectZomboid64"]` so Java is PID 1 (clean SIGTERM).
  Build the image first: `podman build -t safehouse-pz -f Containerfile .`
- **Graceful shutdown** — `server stop` sends RCON `save` + `quit`, then falls back to
  `podman stop` (SIGTERM) if RCON fails. Don't skip the RCON path.

## Config & Secrets

- `safehouse.toml` lives in `~/.local/share/safehouse/` (via `dirs.rs`).
  RCON password is stored plaintext there and written to `server.ini` before each start.
- Web UI session secret is a 64-byte random value auto-generated on first `safehouse serve`.
- Passwords hashed with Argon2id (`db/users.rs`).

## Web UI

- actix-web 4 + Askama templates in `templates/`. HTMX for dynamic updates (no JS framework).
- Static assets embedded via `rust-embed` from `src/assets/`.
- Auth guard is `require_auth` in `web/handlers/auth.rs` — all handler routes use it.

## Gotchas

- PZ's `-cachedir=/zomboid` flag is critical — without it, PZ writes to `/root` inside the
  container which isn't volume-mounted. Set in `container.rs` cmd args.
- The `parking_lot` `RwLock` in `web/state.rs` is sync — holding it across `.await` triggers
  `clippy::await_holding_lock`. The fix is restructuring to drop before await, not switching
  to `tokio::sync::RwLock` (which would require changing all read sites).
- Steam Workshop API (`steam/workshop.rs`) has two TODOs for pagination and rate limiting.
- `validate.rs` sanitizes user input for mod IDs and config values — don't bypass it.
