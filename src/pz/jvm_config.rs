use std::path::Path;

use anyhow::{Context, Result};

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

/// Read `ProjectZomboid64.json` from `install_dir`, add or remove the JMX
/// vmArgs block, and write it back. Idempotent — repeated calls with the same
/// `port` are safe.
///
/// `port = Some(_)` → inject JMX flags (replacing any previous JMX flags).
/// `port = None`    → remove JMX flags, leave everything else untouched.
pub fn apply_jmx(install_dir: &Path, port: Option<u16>) -> Result<()> {
    let json_path = install_dir.join("ProjectZomboid64.json");
    let content = std::fs::read_to_string(&json_path)
        .with_context(|| format!("cannot read {}", json_path.display()))?;
    let mut val: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("invalid JSON in {}", json_path.display()))?;

    let vm_args = val["vmArgs"]
        .as_array_mut()
        .with_context(|| "ProjectZomboid64.json has no vmArgs array")?;

    // Remove any existing JMX flags so this is idempotent.
    vm_args.retain(|a| {
        !a.as_str()
            .map(|s| {
                JMX_FLAGS
                    .iter()
                    .any(|f| s.starts_with(f.split('=').next().unwrap_or(f)))
            })
            .unwrap_or(false)
    });

    if port.is_some() {
        for flag in JMX_FLAGS {
            vm_args.push(serde_json::Value::String(flag.to_string()));
        }
    }

    let output =
        serde_json::to_string_pretty(&val).context("failed to serialize ProjectZomboid64.json")?;
    std::fs::write(&json_path, output)
        .with_context(|| format!("cannot write {}", json_path.display()))?;

    Ok(())
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

    fn write_json(dir: &Path, val: &serde_json::Value) {
        let path = dir.join("ProjectZomboid64.json");
        fs::write(&path, serde_json::to_string_pretty(val).unwrap()).unwrap();
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
        apply_jmx(tmp.path(), Some(9010)).unwrap();

        let content = fs::read_to_string(tmp.path().join("ProjectZomboid64.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        let args: Vec<&str> = v["vmArgs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap())
            .collect();

        assert_eq!(
            args.iter()
                .filter(|&&a| a == "-Dcom.sun.management.jmxremote")
                .count(),
            1
        );
    }

    #[test]
    fn test_apply_jmx_removes_flags() {
        let tmp = tempdir().unwrap();
        write_json(tmp.path(), &base_json());

        apply_jmx(tmp.path(), Some(9010)).unwrap();
        apply_jmx(tmp.path(), None).unwrap();

        let content = fs::read_to_string(tmp.path().join("ProjectZomboid64.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        let args: Vec<&str> = v["vmArgs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap())
            .collect();

        assert!(!args.iter().any(|a| a.contains("jmxremote")));
        assert!(args.contains(&"-Xmx8g"));
    }

    #[test]
    fn test_missing_json_returns_error() {
        let tmp = tempdir().unwrap();
        let result = apply_jmx(tmp.path(), Some(9010));
        assert!(result.is_err());
    }
}
