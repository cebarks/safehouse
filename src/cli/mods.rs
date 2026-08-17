use anyhow::Result;

use super::common::CliContext;
use super::{ModAction, ProfileAction};
use crate::pz::ini::IniEditor;
use crate::pz::mods::{add_mod_to_ini, execute_collection_sync, list_mods, remove_mod_from_ini};
use crate::steam::{fetch_mod_info, parse_collection_id};

pub async fn run(action: &ModAction, ctx: &CliContext) -> Result<()> {
    match action {
        ModAction::List => list(ctx),
        ModAction::Add {
            workshop_id,
            mod_name,
        } => add(ctx, workshop_id, mod_name).await,
        ModAction::Remove { workshop_id } => remove(ctx, workshop_id),
        ModAction::Info { workshop_id } => info(ctx, workshop_id).await,
        ModAction::Sync { collection } => sync(ctx, collection.as_deref()).await,
        ModAction::Profile { action } => profile(ctx, action),
    }
}

fn list(ctx: &CliContext) -> Result<()> {
    let ini_path = ctx.dirs.server_ini(&ctx.config);
    let ini = IniEditor::load(&ini_path)?;
    let mods = list_mods(&ini);
    if mods.is_empty() {
        println!("No mods installed.");
        return Ok(());
    }
    println!("{:<20} Mod Folder", "Workshop ID");
    println!("{}", "-".repeat(40));
    for (id, name) in &mods {
        // Try to show cached title
        let title = ctx
            .db
            .get_cached_mod(id)
            .ok()
            .flatten()
            .map(|m| m.title)
            .unwrap_or_else(|| name.clone());
        println!("{:<20} {} ({})", id, title, name);
    }
    Ok(())
}

async fn add(ctx: &CliContext, workshop_id: &str, mod_name: &str) -> Result<()> {
    let ini_path = ctx.dirs.server_ini(&ctx.config);
    let mut ini = IniEditor::load(&ini_path)?;
    add_mod_to_ini(&mut ini, workshop_id, mod_name)?;
    ini.save(&ini_path)?;

    // Fetch and cache metadata — best effort
    if let Ok(info) = fetch_mod_info(&ctx.http, workshop_id).await {
        println!("Added: {} ({})", info.title, workshop_id);
        let _ = ctx.db.upsert_workshop_mod(&info, Some(mod_name));
    } else {
        println!(
            "Added: {workshop_id} ({mod_name}) [metadata fetch failed, will retry on next list]"
        );
    }
    println!("Restart the server to load the new mod.");
    Ok(())
}

fn remove(ctx: &CliContext, workshop_id: &str) -> Result<()> {
    let ini_path = ctx.dirs.server_ini(&ctx.config);
    let mut ini = IniEditor::load(&ini_path)?;
    let existing_name = ini
        .workshop_ids()
        .into_iter()
        .zip(ini.mod_names())
        .find(|(id, _)| id == workshop_id)
        .map(|(_, name)| name)
        .unwrap_or_default();
    remove_mod_from_ini(&mut ini, workshop_id, &existing_name);
    ini.save(&ini_path)?;
    println!("Removed {workshop_id} from mod list. Restart the server to apply.");
    Ok(())
}

async fn info(ctx: &CliContext, workshop_id: &str) -> Result<()> {
    let info = fetch_mod_info(&ctx.http, workshop_id).await?;
    println!("Title:   {}", info.title);
    println!("ID:      {}", info.workshop_id);
    if let Some(a) = &info.author {
        println!("Author:  {a}");
    }
    if let Some(d) = &info.description {
        let preview: String = d.chars().take(200).collect();
        println!("Description: {preview}...");
    }
    Ok(())
}

async fn sync(ctx: &CliContext, collection_arg: Option<&str>) -> Result<()> {
    // Resolve collection ID: CLI arg > config > error
    let raw = match collection_arg {
        Some(c) => c.to_string(),
        None => ctx
            .config
            .steam_collection_id
            .clone()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No collection specified. Pass a collection ID/URL or set \
                     steam_collection_id in safehouse.toml."
                )
            })?,
    };
    let collection_id = parse_collection_id(&raw)?;
    println!("Fetching collection {collection_id}...");

    let ini_path = ctx.dirs.server_ini(&ctx.config);
    let result = execute_collection_sync(
        &ctx.http,
        &ctx.db,
        &collection_id,
        &ctx.config.server_install_dir,
        &ini_path,
    )
    .await?;

    // Report results
    if !result.added.is_empty() {
        println!("\nAdded ({}):", result.added.len());
        for (id, folders) in &result.added {
            let title = ctx
                .db
                .get_cached_mod(id)
                .ok()
                .flatten()
                .map(|m| m.title)
                .unwrap_or_else(|| "?".to_string());
            println!("  + {id} — {title} ({})", folders.join(", "));
        }
    }
    if !result.removed.is_empty() {
        println!("\nRemoved ({}):", result.removed.len());
        for (id, name) in &result.removed {
            println!("  - {id} ({name})");
        }
    }
    if !result.pending.is_empty() {
        println!(
            "\n⚠ {} mod(s) need a server restart before their folder names can be discovered:",
            result.pending.len()
        );
        for id in &result.pending {
            let title = ctx
                .db
                .get_cached_mod(id)
                .ok()
                .flatten()
                .map(|m| m.title)
                .unwrap_or_else(|| "?".to_string());
            println!("  ? {id} — {title}");
        }
        println!("  Run `safehouse mods sync` again after the server downloads them.");
    }
    println!(
        "\nserver.ini updated — {} mod(s) active. Restart the server to apply.",
        result.total
    );
    Ok(())
}

fn profile(ctx: &CliContext, action: &ProfileAction) -> Result<()> {
    match action {
        ProfileAction::List => {
            let profiles = ctx.db.list_mod_profiles()?;
            if profiles.is_empty() {
                println!("No profiles saved.");
            }
            for name in profiles {
                println!("  {name}");
            }
        }
        ProfileAction::Save { name } => {
            let ini = IniEditor::load(&ctx.dirs.server_ini(&ctx.config))?;
            let ids = ini.workshop_ids();
            let names = ini.mod_names();
            ctx.db.save_mod_profile(name, None, &ids, &names)?;
            println!("Profile '{name}' saved ({} mods).", ids.len());
        }
        ProfileAction::Load { name } => {
            if let Some((ids, names)) = ctx.db.get_mod_profile(name)? {
                let ini_path = ctx.dirs.server_ini(&ctx.config);
                let mut ini = IniEditor::load(&ini_path)?;
                ini.set_workshop_ids(&ids);
                ini.set_mod_names(&names);
                ini.save(&ini_path)?;
                println!(
                    "Loaded profile '{name}' ({} mods). Restart server to apply.",
                    ids.len()
                );
            } else {
                anyhow::bail!("No profile named '{name}'.");
            }
        }
    }
    Ok(())
}
