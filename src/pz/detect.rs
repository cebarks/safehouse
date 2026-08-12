use std::path::{Path, PathBuf};

/// Find the PZ server binary in the given install directory.
pub fn find_server_binary(install_dir: &Path) -> Option<PathBuf> {
    let bin = install_dir.join("ProjectZomboid64");
    if bin.exists() {
        Some(bin)
    } else {
        None
    }
}

/// Read a PID from a PID file. Returns None if file is missing or unparseable.
pub fn read_pid(pid_file: &Path) -> Option<u32> {
    std::fs::read_to_string(pid_file).ok()?.trim().parse().ok()
}

/// Check whether a PID is alive by probing /proc/<pid>.
pub fn pid_is_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// Check if the PZ server is currently running via PID file.
pub fn is_server_running(pid_file: &Path) -> bool {
    read_pid(pid_file).is_some_and(pid_is_alive)
}

/// Acquire an exclusive advisory lock on the PID file.
/// Returns the locked File handle (caller must keep it alive while server runs).
/// Fails if another process already holds the lock.
pub fn lock_pid_file(pid_file: &Path) -> Result<std::fs::File, anyhow::Error> {
    use std::os::unix::io::AsRawFd;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(pid_file)?;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        anyhow::bail!(
            "Another safehouse instance is already managing this server (PID file locked)"
        );
    }
    Ok(file)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_binary_found_when_present() {
        let tmp = tempdir().unwrap();
        let bin = tmp.path().join("ProjectZomboid64");
        std::fs::write(&bin, "").unwrap();
        let result = find_server_binary(tmp.path());
        assert!(result.is_some());
    }

    #[test]
    fn test_binary_absent() {
        let tmp = tempdir().unwrap();
        assert!(find_server_binary(tmp.path()).is_none());
    }

    #[test]
    fn test_read_pid_file_missing() {
        let tmp = tempdir().unwrap();
        let pid_file = tmp.path().join("server.pid");
        assert!(read_pid(&pid_file).is_none());
    }

    #[test]
    fn test_read_pid_file_valid() {
        let tmp = tempdir().unwrap();
        let pid_file = tmp.path().join("server.pid");
        std::fs::write(&pid_file, "12345\n").unwrap();
        assert_eq!(read_pid(&pid_file), Some(12345));
    }
}
