use anyhow::Result;

use super::common::CliContext;
use super::{ConfigAction, PresetAction, SandboxAction};
use crate::pz::ini::IniEditor;
use crate::pz::sandbox::SandboxEditor;

pub fn run(action: &ConfigAction, ctx: &CliContext) -> Result<()> {
    match action {
        ConfigAction::Show => show_ini(ctx),
        ConfigAction::Set { key, value } => set_ini(ctx, key, value),
        ConfigAction::Sandbox { action } => sandbox(ctx, action),
        ConfigAction::Preset { action } => preset(ctx, action),
    }
}

fn show_ini(ctx: &CliContext) -> Result<()> {
    let path = ctx.dirs.server_ini(&ctx.config);
    let ini = IniEditor::load(&path)?;
    print!("{}", ini);
    Ok(())
}

fn set_ini(ctx: &CliContext, key: &str, value: &str) -> Result<()> {
    let path = ctx.dirs.server_ini(&ctx.config);
    let mut ini = IniEditor::load(&path)?;
    let old = ini.get(key).map(str::to_owned);
    ini.set(key, value);
    ini.save(&path)?;
    if let Some(old) = old {
        println!("Updated {key}: {old} → {value}");
    } else {
        println!("Set {key}={value}");
    }
    Ok(())
}

fn sandbox(ctx: &CliContext, action: &SandboxAction) -> Result<()> {
    let path = ctx.dirs.sandbox_lua(&ctx.config);
    match action {
        SandboxAction::Show => {
            let s = SandboxEditor::load(&path)?;
            print!("{}", s);
            Ok(())
        }
        SandboxAction::Set { key, value } => {
            let mut s = SandboxEditor::load(&path)?;
            s.set(key, value);
            s.save(&path)?;
            println!("Set {key} = {value}");
            Ok(())
        }
    }
}

fn preset(ctx: &CliContext, action: &PresetAction) -> Result<()> {
    match action {
        PresetAction::List => {
            let profiles = ctx.db.list_mod_profiles()?;
            if profiles.is_empty() {
                println!("No presets saved.");
            }
            for p in profiles {
                println!("  {p}");
            }
        }
        PresetAction::Save { name } => {
            let ini_path = ctx.dirs.server_ini(&ctx.config);
            let ini = IniEditor::load(&ini_path)?;
            let ids = ini.workshop_ids();
            let names = ini.mod_names();
            ctx.db.save_mod_profile(name, None, &ids, &names)?;
            println!("Saved preset '{name}' with {} mods.", ids.len());
        }
        PresetAction::Apply { name } => {
            if let Some((ids, names)) = ctx.db.get_mod_profile(name)? {
                let ini_path = ctx.dirs.server_ini(&ctx.config);
                let mut ini = IniEditor::load(&ini_path)?;
                ini.set_workshop_ids(&ids);
                ini.set_mod_names(&names);
                ini.save(&ini_path)?;
                println!(
                    "Applied preset '{name}' ({} mods). Restart the server to load.",
                    ids.len()
                );
            } else {
                anyhow::bail!("Preset '{name}' not found. Use `safehouse config preset list`.");
            }
        }
    }
    Ok(())
}
