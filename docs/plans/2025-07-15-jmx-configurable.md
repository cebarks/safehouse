# JMX Configurable Port — Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task.

**Goal:** Make JMX remote monitoring a first-class, opt-in config option in `safehouse.toml` with proper JVM arg injection into `ProjectZomboid64.json` and status visibility.

**Architecture:** Add `jmx_port: Option<u16>` to `SafehouseConfig` (`None` = disabled). When set, a new `pz/jvm_config.rs` module reads `ProjectZomboid64.json` from the server install dir, injects the JMX `vmArgs`, and writes it back — exactly like how RCON settings are written to `server.ini` before start. Container port 9010 is bound to `127.0.0.1:<jmx_port>` on the host. Status command surfaces JMX state.

**Tech Stack:** Rust, `serde_json` (already a dep), bollard container API, `ProjectZomboid64.json` vmArgs editing.

**Behavior note:** Default is `None` (disabled). Existing users who relied on the hardcoded port 9010 binding need to add `jmx_port = 9010` to their `safehouse.toml`.

**The JSON looks like this** (from `/home/anten/pzserver/ProjectZomboid64.json`):

```json
{
    "mainClass": "zombie/network/GameServer",
    "classpath": ["java/.", "java/projectzomboid.jar"],
    "vmArgs": [
        "-Djava.awt.headless=true",
        "-Xmx8g",
        "-Dzomboid.steam=1",
        ...
    ]
}
```

---

### Task 1: Add `jmx_port` to `SafehouseConfig`

**Files:**

- Modify: `src/config.rs`

**Step 1: Write the failing tests first**

In the `#[cfg(test)] mod tests` block at the bottom of `src/config.rs`:

```rust
#[test]
fn test_jmx_port_defaults_to_none() {
    let cfg = SafehouseConfig::default();
    assert!(cfg.jmx_port.is_none());
}

#[test]
fn test_jmx_port_round_trip() {
    let tmp = tempdir().unwrap();
    let path = tmp.path().join("safehouse.toml");
    let mut cfg = SafehouseConfig::default();
    cfg.jmx_port = Some(9010);
    cfg.save(&path).unwrap();
    let loaded = SafehouseConfig::load(&path).unwrap();
    assert_eq!(loaded.jmx_port, Some(9010));
}

#[test]
fn test_jmx_port_disabled_round_trip() {
    let tmp = tempdir().unwrap();
    let path = tmp.path().join("safehouse.toml");
    let cfg = SafehouseConfig::default();
    cfg.save(&path).unwrap();
    let loaded = SafehouseConfig::load(&path).unwrap();
    assert!(loaded.jmx_port.is_none());
}
```

**Step 2: Run failing tests — confirm they fail to compile**

```bash
cargo test -q config::tests::test_jmx 2>&1 | grep -E 'error|FAILED'
```

Expected: compile error — field doesn't exist yet.

**Step 3: Add the field**

Add to the `SafehouseConfig` struct, after the `rcon_port` field:

```rust
/// JMX remote monitoring host port. `None` disables JMX (default).
/// When set, safehouse injects JMX vmArgs into ProjectZomboid64.json
/// and binds container port 9010 to 127.0.0.1:<jmx_port>.
pub jmx_port: Option<u16>,
```

Add `jmx_port: None` to `Default::default()` after `rcon_port`.

**Step 4: Run tests to confirm they pass**

```bash
cargo test -q config::tests
```

Expected: all config tests pass including the 3 new ones.

**Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): add jmx_port opt-in field to SafehouseConfig"
```

---

### Task 2: Add `pz/jvm_config.rs` — edit `ProjectZomboid64.json`

This module is responsible for reading, modifying, and writing `ProjectZomboid64.json`. It follows the same pre-start injection pattern as RCON settings in `server.ini`.

**Files:**

- Create: `src/pz/jvm_config.rs`
- Modify: `src/pz/mod.rs`

**Step 1: Write the failing tests first**

Create `src/pz/jvm_config.rs` with the tests only (no impl yet):

```rust
use std::path::Path;
use anyhow::Result;

/// The JMX vmArgs injected into ProjectZomboid64.json when JMX is enabled.
/// The port inside the container is always 9010; the host binding is separate.
const JMX_FLAGS: &[&str] = &[
    "-Dcom.sun.management.jmxremote",
    "-Dcom.sun.management.jmxremote.port=9010",
    "-Dcom.sun.management.jmxremote.rmi.port=9010",
    "-Djava.rmi.server.hostname=127.0.0.1",
    "-Dcom.sun.management.jmxremote.ssl=false",
    "-Dcom.sun.management.jmxremote.authenticate=false",
];

/// Read ProjectZomboid64.json, add or remove the JMX vmArgs block, and write
/// it back. Idempotent — repeated calls with the same `port` are safe.
///
/// `port = Some(_)` → inject JMX flags (replacing any previous JMX flags).
/// `port = None`    → remove JMX flags, leave everything else untouched.
pub fn apply_jmx(install_dir: &Path, port: Option<u16>) -> Result<()> {
    todo!()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn base_json() -> serde_json::Value {
        serde_json::json!({
            "mainClass": "zombie/network/GameServer",
            "classpath": ["java/."],
            "vmArgs": [
                "-Djava.awt.headless=true",
                "-Xmx8g",
                "-Dzomboid.steam=1"
            ]
        })
    }

    fn write_json(dir: &Path, val: &serde_json::Value) -> std::path::PathBuf {
        let path = dir.join("ProjectZomboid64.json");
        fs::write(&path, serde_json::to_string_pretty(val).unwrap()).unwrap();
        path // return the install_dir (parent is same as dir here since we write the file there)
    }

    #[test]
    fn test_apply_jmx_injects_flags() {
        let tmp = tempdir().unwrap();
        write_json(tmp.path(), &base_json());

        apply_jmx(tmp.path(), Some(9010)).unwrap();

        let content = fs::read_to_string(tmp.path().join("ProjectZomboid64.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        let args: Vec<&str> = v["vmArgs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap())
            .collect();

        assert!(args.contains(&"-Dcom.sun.management.jmxremote"));
        assert!(args.contains(&"-Dcom.sun.management.jmxremote.port=9010"));
        assert!(args.contains(&"-Djava.rmi.server.hostname=127.0.0.1"));
        // Original args preserved
        assert!(args.contains(&"-Xmx8g"));
        assert!(args.contains(&"-Djava.awt.headless=true"));
    }

    #[test]
    fn test_apply_jmx_is_idempotent() {
        let tmp = tempdir().unwrap();
        write_json(tmp.path(), &base_json());

        apply_jmx(tmp.path(), Some(9010)).unwrap();
        apply_jmx(tmp.path(), Some(9010)).unwrap(); // second call

        let content = fs::read_to_string(tmp.path().join("ProjectZomboid64.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        let args: Vec<&str> = v["vmArgs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap())
            .collect();

        // JMX flag should appear exactly once
        assert_eq!(
            args.iter().filter(|&&a| a == "-Dcom.sun.management.jmxremote").count(),
            1
        );
    }

    #[test]
    fn test_apply_jmx_removes_flags() {
        let tmp = tempdir().unwrap();
        write_json(tmp.path(), &base_json());

        apply_jmx(tmp.path(), Some(9010)).unwrap(); // add
        apply_jmx(tmp.path(), None).unwrap();        // remove

        let content = fs::read_to_string(tmp.path().join("ProjectZomboid64.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        let args: Vec<&str> = v["vmArgs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap())
            .collect();

        assert!(!args.iter().any(|a| a.contains("jmxremote")));
        // Original args still present
        assert!(args.contains(&"-Xmx8g"));
    }

    #[test]
    fn test_missing_json_returns_error() {
        let tmp = tempdir().unwrap();
        // No JSON file written
        let result = apply_jmx(tmp.path(), Some(9010));
        assert!(result.is_err());
    }
}
```

**Step 2: Run failing tests**

First register the module in `src/pz/mod.rs`:

```rust
pub mod jvm_config;
```

Then run:

```bash
cargo test -q pz::jvm_config::tests 2>&1 | grep -E 'FAILED|error'
```

Expected: 4 failures (all panic on `todo!()`), `test_missing_json_returns_error` may also fail.

**Step 3: Implement `apply_jmx`**

Replace `todo!()` with the real implementation:

```rust
pub fn apply_jmx(install_dir: &Path, port: Option<u16>) -> Result<()> {
    let json_path = install_dir.join("ProjectZomboid64.json");
    let content = std::fs::read_to_string(&json_path)
        .with_context(|| format!("cannot read {}", json_path.display()))?;
    let mut val: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("invalid JSON in {}", json_path.display()))?;

    let vm_args = val["vmArgs"]
        .as_array_mut()
        .with_context(|| "ProjectZomboid64.json has no vmArgs array")?;

    // Remove any existing JMX flags (makes this idempotent).
    vm_args.retain(|a| {
        !a.as_str()
            .map(|s| JMX_FLAGS.iter().any(|f| s.starts_with(f.split('=').next().unwrap_or(f))))
            .unwrap_or(false)
    });

    if port.is_some() {
        for flag in JMX_FLAGS {
            vm_args.push(serde_json::Value::String(flag.to_string()));
        }
    }

    let output = serde_json::to_string_pretty(&val)
        .context("failed to serialize ProjectZomboid64.json")?;
    std::fs::write(&json_path, output)
        .with_context(|| format!("cannot write {}", json_path.display()))?;

    Ok(())
}
```

Add `use anyhow::Context;` at the top of the file.

**Step 4: Run tests to confirm they pass**

```bash
cargo test -q pz::jvm_config::tests
```

Expected: all 4 tests pass.

**Step 5: Commit**

```bash
git add src/pz/jvm_config.rs src/pz/mod.rs
git commit -m "feat(pz): add jvm_config module for ProjectZomboid64.json vmArgs editing"
```

---

### Task 3: Wire JMX into container startup and port binding

**Files:**

- Modify: `src/container.rs` — conditional port binding
- Modify: `src/cli/server.rs` — call `apply_jmx` before container create (mirrors RCON write)

**Step 1: Update `container.rs`**

Replace the hardcoded JMX port binding block with a conditional one. Remove:

```rust
// JMX remote monitoring (localhost-only, tunneled via SSH)
port_bindings.insert(
    "9010/tcp".to_string(),
    Some(vec![PortBinding {
        host_ip: Some("127.0.0.1".to_string()),
        host_port: Some("9010".to_string()),
    }]),
);
```

Add after the RCON binding:

```rust
// JMX remote monitoring (localhost-only, tunneled via SSH)
// JVM args are injected into ProjectZomboid64.json before container start.
if let Some(host_port) = config.jmx_port {
    port_bindings.insert(
        "9010/tcp".to_string(),
        Some(vec![PortBinding {
            host_ip: Some("127.0.0.1".to_string()),
            host_port: Some(host_port.to_string()),
        }]),
    );
}
```

**Step 2: Update `cli/server.rs` — inject vmArgs before start**

In `start()`, after the block that writes RCON settings to `server.ini`, add:

```rust
// Inject or clear JMX vmArgs in ProjectZomboid64.json
let json_path = ctx.config.server_install_dir.join("ProjectZomboid64.json");
if json_path.exists() {
    crate::pz::jvm_config::apply_jmx(&ctx.config.server_install_dir, ctx.config.jmx_port)
        .with_context(|| "failed to configure JMX in ProjectZomboid64.json")?;
    if ctx.config.jmx_port.is_some() {
        tracing::info!("JMX enabled in ProjectZomboid64.json (port {})", ctx.config.jmx_port.unwrap());
    }
} else if ctx.config.jmx_port.is_some() {
    tracing::warn!("JMX requested but ProjectZomboid64.json not found in {}", ctx.config.server_install_dir.display());
}
```

**Step 3: Compile check**

```bash
cargo clippy 2>&1 | grep -E '^error'
```

Expected: no errors.

**Step 4: Commit**

```bash
git add src/container.rs src/cli/server.rs
git commit -m "feat(server): wire JMX port binding and vmArgs injection into server start"
```

---

### Task 4: Expose JMX port in Containerfile + surface in status

**Files:**

- Modify: `Containerfile`
- Modify: `src/cli/server.rs`

**Step 1: Add EXPOSE in Containerfile**

```dockerfile
EXPOSE 16261/udp 16262/udp 27015/tcp 9010/tcp
```

**Step 2: Add JMX line to `status()`**

In `src/cli/server.rs` `status()`, after the `println!("Install: ...")` line:

```rust
if let Some(port) = ctx.config.jmx_port {
    println!("JMX:     🟢 Enabled (127.0.0.1:{})", port);
} else {
    println!("JMX:     ⚫ Disabled");
}
```

**Step 3: Full test suite**

```bash
cargo test 2>&1 | tail -5
```

Expected: `test result: ok. 110 tests passed` (106 existing + 4 new jvm_config + 3 new config tests — 1 was counted in task 1 already, but count may vary by 1-3; point is zero failures).

**Step 4: Commit**

```bash
git add Containerfile src/cli/server.rs
git commit -m "feat(server): expose JMX port and show status in server status output"
```

---

### Task 5: Final verification

```bash
cargo build 2>&1 | grep -E '^error'
cargo clippy 2>&1 | grep -E '^error'
cargo test 2>&1 | tail -5
```

All must be clean.
