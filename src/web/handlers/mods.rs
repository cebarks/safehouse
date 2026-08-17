use actix_session::Session;
use actix_web::{get, post, web, HttpResponse, Responder};
use askama::Template;
use serde::Deserialize;

use crate::pz::ini::IniEditor;
use crate::pz::mods::{add_mod_to_ini, execute_collection_sync, list_mods, remove_mod_from_ini};
use crate::steam::{fetch_mod_info, parse_collection_id};
use crate::web::handlers::auth::require_auth;
use crate::web::state::AppState;

#[derive(Template)]
#[template(path = "mods.html")]
struct ModsTemplate {
    mods: Vec<(String, String, String)>, // (workshop_id, folder_name, title)
    profiles: Vec<String>,
    collection_id: String,
    message: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct ModsQuery {
    synced: Option<usize>,
    added: Option<usize>,
    removed: Option<usize>,
    pending: Option<usize>,
}

#[get("/mods")]
pub async fn mods_page(
    session: Session,
    state: web::Data<AppState>,
    query: web::Query<ModsQuery>,
) -> impl Responder {
    if let Some(r) = require_auth(&session) {
        return r;
    }
    let cfg = state.config.read();
    let cfg2 = cfg.clone();
    let ini_path = state.dirs.server_ini(&cfg);
    drop(cfg);
    let ini = IniEditor::load(&ini_path).unwrap_or_else(|_| IniEditor::parse(""));
    let raw_mods = list_mods(&ini);
    let db = state.db.lock();
    let mods: Vec<(String, String, String)> = raw_mods
        .into_iter()
        .map(|(id, name)| {
            let title = db
                .get_cached_mod(&id)
                .ok()
                .flatten()
                .map(|m| m.title)
                .unwrap_or_else(|| name.clone());
            (id, name, title)
        })
        .collect();
    let profiles = db.list_mod_profiles().unwrap_or_default();
    drop(db);
    let collection_id = cfg2.steam_collection_id.clone().unwrap_or_default();
    let message = query.synced.map(|total| {
        format!(
            "Synced {} mod(s) from collection. {} added, {} removed, {} pending download.",
            total,
            query.added.unwrap_or(0),
            query.removed.unwrap_or(0),
            query.pending.unwrap_or(0),
        )
    });
    let tmpl = ModsTemplate {
        mods,
        profiles,
        collection_id,
        message,
    };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(tmpl.render().unwrap_or_default())
}

#[derive(Deserialize)]
pub struct AddModForm {
    workshop_id: String,
    mod_name: String,
}

#[post("/mods/add")]
pub async fn mods_add(
    form: web::Form<AddModForm>,
    session: Session,
    state: web::Data<AppState>,
) -> impl Responder {
    if let Some(r) = require_auth(&session) {
        return r;
    }
    let cfg = state.config.read();
    let ini_path = state.dirs.server_ini(&cfg);
    drop(cfg);
    match IniEditor::load(&ini_path) {
        Ok(mut ini) => match add_mod_to_ini(&mut ini, &form.workshop_id, &form.mod_name) {
            Ok(()) => {
                if let Err(e) = ini.save(&ini_path) {
                    return HttpResponse::InternalServerError().body(e.to_string());
                }
            }
            Err(e) => return HttpResponse::BadRequest().body(e.to_string()),
        },
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    }
    // Fetch metadata best-effort
    if let Ok(info) = fetch_mod_info(&state.http, &form.workshop_id).await {
        let db = state.db.lock();
        let _ = db.upsert_workshop_mod(&info, Some(&form.mod_name));
    }
    HttpResponse::Found()
        .insert_header(("Location", "/mods"))
        .finish()
}

#[derive(Deserialize)]
pub struct RemoveModForm {
    workshop_id: String,
    mod_name: String,
}

#[derive(Deserialize)]
pub struct SyncForm {
    collection: Option<String>,
}

#[allow(clippy::await_holding_lock)] // parking_lot::Mutex is safe across awaits; admin-only endpoint
#[post("/mods/sync")]
pub async fn mods_sync(
    form: web::Form<SyncForm>,
    session: Session,
    state: web::Data<AppState>,
) -> impl Responder {
    if let Some(r) = require_auth(&session) {
        return r;
    }

    let (raw_input, install_dir, ini_path) = {
        let cfg = state.config.read();
        let raw = form
            .collection
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or(cfg.steam_collection_id.as_deref())
            .map(str::to_string);
        let install = cfg.server_install_dir.clone();
        let ini = state.dirs.server_ini(&cfg);
        (raw, install, ini)
    };
    let raw = match raw_input {
        Some(r) => r,
        None => {
            return HttpResponse::BadRequest()
                .body("No collection ID provided and none configured.");
        }
    };
    let collection_id = match parse_collection_id(&raw) {
        Ok(id) => id,
        Err(e) => return HttpResponse::BadRequest().body(e.to_string()),
    };

    let result = {
        let db = state.db.lock();
        match execute_collection_sync(
            &state.http,
            &db,
            &collection_id,
            &install_dir,
            &ini_path,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
        }
    };

    HttpResponse::Found()
        .insert_header((
            "Location",
            format!(
                "/mods?synced={}&added={}&removed={}&pending={}",
                result.total,
                result.added.len(),
                result.removed.len(),
                result.pending.len()
            ),
        ))
        .finish()
}

#[post("/mods/remove")]
pub async fn mods_remove(
    form: web::Form<RemoveModForm>,
    session: Session,
    state: web::Data<AppState>,
) -> impl Responder {
    if let Some(r) = require_auth(&session) {
        return r;
    }
    let cfg = state.config.read();
    let ini_path = state.dirs.server_ini(&cfg);
    drop(cfg);
    if let Ok(mut ini) = IniEditor::load(&ini_path) {
        remove_mod_from_ini(&mut ini, &form.workshop_id, &form.mod_name);
        let _ = ini.save(&ini_path);
    }
    HttpResponse::Found()
        .insert_header(("Location", "/mods"))
        .finish()
}
