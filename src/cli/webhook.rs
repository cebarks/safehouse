use anyhow::Result;

use super::common::CliContext;
use crate::notify::{notify, NotifyEvent};

pub async fn run(url: Option<&str>, test: bool, ctx: &CliContext) -> Result<()> {
    if let Some(u) = url {
        // Save to config
        let mut cfg = ctx.config.clone();
        cfg.discord_webhook_url = Some(u.to_string());
        cfg.save(&ctx.dirs.config_path())?;
        println!("Webhook URL saved.");
    }

    if test {
        // Use the newly-saved URL if one was provided, otherwise fall back to config
        let effective_url = url.or(ctx.config.discord_webhook_url.as_deref());
        notify(
            &ctx.http,
            effective_url,
            &ctx.config.server_name,
            NotifyEvent::ServerStarted,
        )
        .await?;
        println!("Test notification sent.");
    }
    Ok(())
}
