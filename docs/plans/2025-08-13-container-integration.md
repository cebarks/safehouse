# Container Integration Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task.

**Goal:** Replace direct process spawning with a podman container managed via bollard, fixing PID tracking, signal propagation, log capture, and env setup issues.

**Architecture:** A new `src/container.rs` module wraps bollard to provide create/start/stop/logs/inspect for a `safehouse-pz` container. Server lifecycle commands in `src/cli/server.rs` call this module instead of spawning `start-server.sh` directly. The `Containerfile` (already built) provides the runtime image.

**Tech Stack:** bollard 0.21 (async podman/docker API), existing tokio runtime

---

### Task 1: Add bollard dependency

**Files:**

- Modify: `Cargo.toml`

Add `bollard = "0.21"` to `[dependencies]`.

---

### Task 2: Create `src/container.rs` module

**Files:**

- Create: `src/container.rs`
- Modify: `src/lib.rs` (add `pub mod container;`)

Thin wrapper around bollard with these functions:

- `connect()` → `Docker::connect_with_podman_defaults()`
- `ensure_image(docker, tag)` → inspect image, bail if missing with build instructions
- `is_running(docker, name)` → inspect container, return bool
- `create_and_start(docker, config)` → create container with volumes/ports/args, start it
- `stop(docker, name, timeout)` → stop container
- `remove(docker, name)` → remove container
- `logs(docker, name, follow, tail)` → stream container logs

Container name: `safehouse-pz`
Image: `localhost/safehouse-pz:latest`
Volumes: `server_install_dir:/server:Z`, `zomboid_dir:/zomboid:Z`
Ports: `16261:16261/udp`, `16262:16262/udp`, `rcon_port:27015/tcp`
Args: `-servername <name>` + optional `-adminpassword <pass>`

---

### Task 3: Rewrite `server start` to use container

**Files:**

- Modify: `src/cli/server.rs`

Replace the `Command::new(launcher)` spawn block with:

1. Write RCON settings to server.ini (keep existing logic)
2. `container::create_and_start()` with config from `SafehouseConfig`
3. Poll RCON for readiness (keep existing loop)

Remove: `Stdio` imports, `find_server_binary` check, `start-server.sh` check, PID file write, `tokio::spawn` detach.

---

### Task 4: Rewrite `server stop` to use container

**Files:**

- Modify: `src/cli/server.rs`

Replace PID-based stop with:

1. RCON save + quit (keep existing)
2. Fallback: `container::stop()` (replaces SIGTERM/SIGKILL to PID)
3. `container::remove()` to clean up

Remove: PID file read, `libc::kill` calls, `/proc` polling.

---

### Task 5: Rewrite `server status` to use container

**Files:**

- Modify: `src/cli/server.rs`

Replace `is_server_running(&pid_file)` with `container::is_running()`.
Keep RCON player count query.

---

### Task 6: Rewrite `server logs` to use container

**Files:**

- Modify: `src/cli/server.rs`

Replace `latest_log()` file glob with `container::logs()` stream.
Support `--follow` via bollard's `follow: true` option.
Support `--lines` via bollard's `tail` option.

---

### Task 7: Update `server restart`

**Files:**

- Modify: `src/cli/server.rs`

Stop + remove container, then create + start new one.

---

### Task 8: Update `setup` to use container for SteamCMD

**Files:**

- Modify: `src/cli/setup.rs`

Replace direct `steamcmd` invocation with:

1. `podman run --rm --entrypoint steamcmd.sh safehouse-pz +force_install_dir /server +login anonymous +app_update 380870 validate +quit` via bollard
2. Server files persist in the volume

---

### Task 9: Clean up dead code

**Files:**

- Modify: `src/pz/detect.rs` — remove `lock_pid_file`, simplify `is_server_running`
- Modify: `src/dirs.rs` — remove `latest_log()`, `pid_file()`, `run_dir()` if unused
- Modify: `src/cli/serve.rs` — update log watcher to use container logs or connections file

---

### Task 10: Test and verify

- `cargo test` — all existing tests pass
- `safehouse server start` — container starts, RCON responds
- `safehouse server status` — shows running + player count
- `safehouse server logs` — streams container output
- `safehouse server stop` — graceful RCON + container stop
- `safehouse console players` — RCON works
- `safehouse backup create` — backup works
