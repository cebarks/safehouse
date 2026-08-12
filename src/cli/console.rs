use anyhow::{Context, Result};

use super::common::CliContext;
use super::ConsoleAction;
use crate::pz::rcon::RconClient;

pub async fn run(action: &ConsoleAction, ctx: &CliContext) -> Result<()> {
    let mut rcon =
        RconClient::connect("127.0.0.1", ctx.config.rcon_port, &ctx.config.rcon_password)
            .context("Cannot connect to RCON — is the server running?")?;
    match action {
        ConsoleAction::Chat { message } => {
            let r = rcon.send_command(&format!("servermsg \"{message}\""))?;
            println!("{r}");
        }
        ConsoleAction::Players => {
            let r = rcon.send_command("players")?;
            println!("{r}");
        }
        ConsoleAction::Kick { player } => {
            let r = rcon.send_command(&format!("kickuser \"{player}\""))?;
            println!("{r}");
        }
        ConsoleAction::Ban { player } => {
            let r = rcon.send_command(&format!("banuser \"{player}\""))?;
            println!("{r}");
        }
        ConsoleAction::Give { player, item } => {
            let r = rcon.send_command(&format!("additem \"{player}\" \"{item}\""))?;
            println!("{r}");
        }
        ConsoleAction::Save => {
            rcon.send_command("save")?;
            println!("World save triggered.");
        }
    }
    Ok(())
}
