use anyhow::Result;
use clap::Parser;
use safehouse::cli::{self, Cli, Command};
use safehouse::logging;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    logging::init(cli.verbose);

    match &cli.command {
        Command::Setup {
            install_dir,
            admin_password,
        } => {
            cli::setup::run(
                install_dir.as_deref(),
                admin_password.as_deref(),
                cli.data_dir.as_deref(),
            )
            .await
        }
        Command::Server { action } => {
            let ctx = cli::common::resolve_context(&cli)?;
            cli::server::run(action, &ctx).await
        }
        Command::Config { action } => {
            let ctx = cli::common::resolve_context(&cli)?;
            cli::config::run(action, &ctx)
        }
        Command::Mods { action } => {
            let ctx = cli::common::resolve_context(&cli)?;
            cli::mods::run(action, &ctx).await
        }
        Command::Backup { action } => {
            let ctx = cli::common::resolve_context(&cli)?;
            cli::backup::run(action, &ctx).await
        }
        Command::Console { action } => {
            let ctx = cli::common::resolve_context(&cli)?;
            cli::console::run(action, &ctx).await
        }
        Command::Webhook { url, test } => {
            let ctx = cli::common::resolve_context(&cli)?;
            cli::webhook::run(url.as_deref(), *test, &ctx).await
        }
        Command::Serve { bind, port } => cli::serve::run(bind.as_deref(), *port, &cli).await,
    }
}
