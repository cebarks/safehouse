use actix_session::Session;
use actix_web::{get, post, web, HttpResponse, Responder};
use askama::Template;
use serde::Deserialize;

use crate::pz::ini::IniEditor;
use crate::pz::mods::{add_mod_to_ini, list_mods, remove_mod_from_ini};
use crate::steam::fetch_mod_info;
use crate::web::handlers::auth::require_auth;
use crate::web::state::AppState;

#[derive(Template)]
#[template(path = "mods.html")]
struct ModsTemplate {
    mods: Vec<(String, String, String)>, // (workshop_id, folder_name, title)
    profiles: Vec<String>,
    message: Option<String>,
}

#[get("/mods")]
pub async fn mods_page(session: Session, state: web::Data<AppState>) -> impl Responder {
    if let Some(r) = require_auth(&session) {
        return r;
    }
    let cfg = state.config.read();
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
    let tmpl = ModsTemplate {
        mods,
        profiles,
        message: None,
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
    if let Ok(mut ini) = IniEditor::load(&ini_path) {
        add_mod_to_ini(&mut ini, &form.workshop_id, &form.mod_name);
        let _ = ini.save(&ini_path);
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
