use std::sync::OnceLock;

use regex::Regex;

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    Connected { name: String },
    Disconnected { name: String },
}

static CONNECT_RE: OnceLock<Regex> = OnceLock::new();
static DISCONNECT_RE: OnceLock<Regex> = OnceLock::new();

#[allow(clippy::unwrap_used)] // static regexes, always valid
fn connect_re() -> &'static Regex {
    CONNECT_RE.get_or_init(|| Regex::new(r"user '([^']+)' connected").unwrap())
}

#[allow(clippy::unwrap_used)] // static regexes, always valid
fn disconnect_re() -> &'static Regex {
    DISCONNECT_RE.get_or_init(|| Regex::new(r"user '([^']+)' disconnected").unwrap())
}

pub fn parse_log_line(line: &str) -> Option<PlayerEvent> {
    // Check disconnect first since "disconnected" also contains "connected"
    if let Some(cap) = disconnect_re().captures(line) {
        return Some(PlayerEvent::Disconnected {
            name: cap[1].to_owned(),
        });
    }
    if let Some(cap) = connect_re().captures(line) {
        return Some(PlayerEvent::Connected {
            name: cap[1].to_owned(),
        });
    }
    None
}

/// Tail `n` lines from a file path.
pub fn tail_lines(path: &std::path::Path, n: usize) -> anyhow::Result<Vec<String>> {
    use std::io::{BufRead, BufReader};
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().collect::<std::io::Result<_>>()?;
    let start = lines.len().saturating_sub(n);
    Ok(lines[start..].to_vec())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_connect() {
        let line = "1723165432000 LOG  : General     , 1723165432000> user 'Alice' connected";
        let event = parse_log_line(line);
        assert_eq!(
            event,
            Some(PlayerEvent::Connected {
                name: "Alice".to_string()
            })
        );
    }

    #[test]
    fn test_parse_disconnect() {
        let line = "1723165432783 LOG  : General     , 1723165432783> user 'Bob' disconnected";
        let event = parse_log_line(line);
        assert_eq!(
            event,
            Some(PlayerEvent::Disconnected {
                name: "Bob".to_string()
            })
        );
    }

    #[test]
    fn test_parse_irrelevant_line() {
        let line = "1723165432000 LOG  : General     , something else happened";
        assert_eq!(parse_log_line(line), None);
    }

    #[test]
    fn test_tail_lines() {
        use std::io::Write;
        let mut f = NamedTempFile::new().unwrap();
        for i in 0..10 {
            writeln!(f, "line {i}").unwrap();
        }
        let lines = tail_lines(f.path(), 3).unwrap();
        assert_eq!(lines, vec!["line 7", "line 8", "line 9"]);
    }

    #[test]
    fn test_tail_lines_more_than_available() {
        use std::io::Write;
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "only line").unwrap();
        let lines = tail_lines(f.path(), 100).unwrap();
        assert_eq!(lines, vec!["only line"]);
    }
}
