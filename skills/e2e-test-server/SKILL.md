---
name: e2e-test-server
description: Use when the user asks to test, demo, or run safehouse locally — spins up a real PZ server with web UI for manual or automated testing.
---

# E2E Test Server

Spin up a full safehouse instance (web UI + real PZ dedicated server in a container) for testing. Uses an isolated data directory so it won't touch production state.

## Prerequisites

- `cargo build --release` passes
- `podman` running with socket active (`systemctl --user is-active podman.socket`)

## Steps

### 1. Build

```bash
cargo build --release
```

### 2. Create isolated test environment

```bash
export SAFEHOUSE_DIR=/tmp/safehouse-e2e
mkdir -p $SAFEHOUSE_DIR

cat > $SAFEHOUSE_DIR/safehouse.toml << 'EOF'
server_install_dir = "/tmp/safehouse-e2e/pzserver"
server_name = "testworld"
rcon_password = "testpass"
rcon_port = 27015
web_bind = "127.0.0.1"
web_port = 9292
EOF
```

### 3. Create admin user

`safehouse serve` prompts interactively for the first admin password, which doesn't work in automation. Seed the DB directly:

```bash
cat > /tmp/setup_admin.rs << 'RUST'
use safehouse::db::Database;
use safehouse::config::SafehouseConfig;
fn main() {
    let db = Database::open(std::path::Path::new("/tmp/safehouse-e2e/safehouse.db")).unwrap();
    if !db.user_exists("admin").unwrap() {
        db.create_user("admin", "safehouse123").unwrap();
        println!("Admin user created");
    }
    let cfg_path = std::path::Path::new("/tmp/safehouse-e2e/safehouse.toml");
    let mut cfg = SafehouseConfig::load(cfg_path).unwrap();
    cfg.ensure_session_secret();
    cfg.save(cfg_path).unwrap();
}
RUST

rustc --edition 2021 -L target/release/deps \
  --extern safehouse=target/release/libsafehouse.rlib \
  /tmp/setup_admin.rs -o /tmp/setup_admin && /tmp/setup_admin
```

**Credentials:** `admin` / `safehouse123`

### 4. Build container image

```bash
podman build -t safehouse-pz -f Containerfile .
```

### 5. Start web UI

```bash
SAFEHOUSE_DIR=/tmp/safehouse-e2e target/release/safehouse serve &
```

Web UI at **<http://127.0.0.1:9292>** — login with admin/safehouse123.

### 6. Install PZ dedicated server (~2GB, takes a few minutes)

```bash
mkdir -p /tmp/safehouse-e2e/pzserver /tmp/safehouse-e2e/zomboid

podman run --rm --name safehouse-setup \
  -v /tmp/safehouse-e2e/pzserver:/server:Z \
  --entrypoint steamcmd.sh \
  localhost/safehouse-pz:latest \
  +force_install_dir /server +login anonymous \
  +app_update 380870 validate +quit
```

### 7. Start PZ server

```bash
SAFEHOUSE_DIR=/tmp/safehouse-e2e target/release/safehouse server start
```

Dashboard should show 🟢 Running with player count.

## Teardown

```bash
SAFEHOUSE_DIR=/tmp/safehouse-e2e target/release/safehouse server stop
podman rm -f safehouse-pz 2>/dev/null
rm -rf /tmp/safehouse-e2e /tmp/setup_admin /tmp/setup_admin.rs
```

## Common Issues

| Symptom | Cause | Fix |
| --------- | ------- | ----- |
| `failed to connect to podman/docker` | Socket not active | `systemctl --user start podman.socket` |
| Dashboard shows Stopped when server is running | Stale PID-file check (pre-container bug) | Ensure commit `ecbbb03` or later |
| `serve` hangs on "Set admin password" | No pre-seeded DB | Run step 3 before step 5 |
| Container exits immediately | PZ binary missing | Run step 6 (SteamCMD install) first |
