use actix_session::Session;
use actix_web::{get, post, web, HttpResponse, Responder};
use askama::Template;
use serde::Deserialize;

use crate::pz::ini::IniEditor;
use crate::web::handlers::auth::require_auth;
use crate::web::state::AppState;

#[derive(Template)]
#[template(path = "config.html")]
struct ConfigTemplate {
    ini_content: String,
    sandbox_content: String,
    message: Option<String>,
}

#[get("/config")]
pub async fn config_page(session: Session, state: web::Data<AppState>) -> impl Responder {
    if let Some(r) = require_auth(&session) {
        return r;
    }
    let cfg = state.config.read();
    let ini = IniEditor::load(&state.dirs.server_ini(&cfg))
        .map(|e| e.to_string())
        .unwrap_or_default();
    let sandbox = crate::pz::sandbox::SandboxEditor::load(&state.dirs.sandbox_lua(&cfg))
        .map(|e| e.to_string())
        .unwrap_or_default();
    drop(cfg);
    let tmpl = ConfigTemplate {
        ini_content: ini,
        sandbox_content: sandbox,
        message: None,
    };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(tmpl.render().unwrap_or_default())
}

#[derive(Deserialize)]
pub struct SetKeyForm {
    key: String,
    value: String,
    file: String,
}

#[post("/config/set")]
pub async fn config_set(
    form: web::Form<SetKeyForm>,
    session: Session,
    state: web::Data<AppState>,
) -> impl Responder {
    if let Some(r) = require_auth(&session) {
        return r;
    }
    let cfg = state.config.read();
    let result = match form.file.as_str() {
        "ini" => {
            let path = state.dirs.server_ini(&cfg);
            IniEditor::load(&path).and_then(|mut e| {
                e.set(&form.key, &form.value);
                e.save(&path)
            })
        }
        "sandbox" => {
            let path = state.dirs.sandbox_lua(&cfg);
            crate::pz::sandbox::SandboxEditor::load(&path).and_then(|mut e| {
                e.set(&form.key, &form.value);
                e.save(&path)
            })
        }
        _ => Err(anyhow::anyhow!("unknown file")),
    };
    drop(cfg);
    if result.is_ok() {
        HttpResponse::Found()
            .insert_header(("Location", "/config?saved=1"))
            .finish()
    } else {
        HttpResponse::InternalServerError().body("Failed to update config")
    }
}
