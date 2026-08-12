use anyhow::Result;
use serde_json::{json, Value};

pub enum NotifyEvent {
    ServerStarted,
    ServerStopped,
    PlayerJoined(String),
    PlayerLeft(String),
    BackupComplete { filename: String },
    UpdateAvailable { version: String },
}

impl NotifyEvent {
    /// Returns (title, hex_color). Title is owned because dynamic variants
    /// interpolate runtime strings.
    pub fn title_and_color(&self) -> (String, &'static str) {
        match self {
            NotifyEvent::ServerStarted => ("🟢 Server started".to_string(), "00b300"),
            NotifyEvent::ServerStopped => ("🔴 Server stopped".to_string(), "cc0000"),
            NotifyEvent::PlayerJoined(n) => (format!("👤 {n} joined"), "0099cc"),
            NotifyEvent::PlayerLeft(n) => (format!("👋 {n} left"), "888888"),
            NotifyEvent::BackupComplete { filename } => {
                (format!("💾 Backup: {filename}"), "ffaa00")
            }
            NotifyEvent::UpdateAvailable { version } => {
                (format!("⬆️ Update available: v{version}"), "aa00ff")
            }
        }
    }
}

pub fn build_webhook_payload(title: &str, server_name: &str, hex_color: &str) -> Value {
    let color = i64::from_str_radix(hex_color, 16).unwrap_or(0);
    json!({
        "username": format!("Safehouse | {server_name}"),
        "embeds": [{
            "title": title,
            "color": color
        }]
    })
}

pub async fn send_webhook(
    client: &reqwest::Client,
    url: &str,
    payload: Value,
) -> Result<()> {
    client
        .post(url)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

pub async fn notify(
    client: &reqwest::Client,
    webhook_url: Option<&str>,
    server_name: &str,
    event: NotifyEvent,
) -> Result<()> {
    let Some(url) = webhook_url else {
        return Ok(());
    };
    let (title, color) = event.title_and_color();
    let payload = build_webhook_payload(&title, server_name, color);
    send_webhook(client, url, payload).await
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_build_embed_payload() {
        let payload = build_webhook_payload("🟢 Server started", "safehouse", "00ff00");
        let obj = payload.as_object().unwrap();
        assert!(obj.contains_key("embeds"));
        assert!(obj.contains_key("username"));
        let embeds = obj["embeds"].as_array().unwrap();
        assert_eq!(embeds.len(), 1);
        assert_eq!(embeds[0]["title"], "🟢 Server started");
    }

    #[test]
    fn test_title_and_color_variants() {
        let (title, color) = NotifyEvent::ServerStarted.title_and_color();
        assert!(title.contains("started"));
        assert_eq!(color, "00b300");

        let (title, color) = NotifyEvent::PlayerJoined("Alice".to_string()).title_and_color();
        assert!(title.contains("Alice"));
        assert_eq!(color, "0099cc");

        let (title, _) = NotifyEvent::BackupComplete {
            filename: "snap.tar.gz".to_string(),
        }
        .title_and_color();
        assert!(title.contains("snap.tar.gz"));
    }

    #[test]
    fn test_color_parsed_as_hex() {
        let payload = build_webhook_payload("test", "server", "ff0000");
        let color = payload["embeds"][0]["color"].as_i64().unwrap();
        assert_eq!(color, 0xff0000);
    }

    #[test]
    fn test_invalid_hex_color_defaults_to_zero() {
        let payload = build_webhook_payload("test", "server", "not_hex");
        let color = payload["embeds"][0]["color"].as_i64().unwrap();
        assert_eq!(color, 0);
    }
}
