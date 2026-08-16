use actix_session::Session;
use actix_web::{get, post, web, HttpResponse, Responder};
use askama::Template;
use serde::Deserialize;

use crate::backup::{create_snapshot, list_snapshots};
use crate::web::handlers::auth::require_auth;
use crate::web::state::AppState;

#[derive(Template)]
#[template(path = "backups.html")]
struct BackupsTemplate {
    snapshots: Vec<(String, String)>, // (filename, size_human)
    message: Option<String>,
}

#[get("/backups")]
pub async fn backups_page(session: Session, state: web::Data<AppState>) -> impl Responder {
    if let Some(r) = require_auth(&session) {
        return r;
    }
    let snaps = list_snapshots(&state.dirs.backups_dir()).unwrap_or_default();
    let snapshots: Vec<(String, String)> = snaps
        .iter()
        .map(|p| {
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let size = p
                .metadata()
                .map(|m| format!("{:.1} MB", m.len() as f64 / 1_048_576.0))
                .unwrap_or_default();
            (name, size)
        })
        .collect();
    let tmpl = BackupsTemplate {
        snapshots,
        message: None,
    };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(tmpl.render().unwrap_or_default())
}

#[derive(Deserialize)]
pub struct CreateBackupForm {
    label: Option<String>,
}

#[post("/backups/create")]
pub async fn backup_create(
    form: web::Form<CreateBackupForm>,
    session: Session,
    state: web::Data<AppState>,
) -> impl Responder {
    if let Some(r) = require_auth(&session) {
        return r;
    }
    let label = form.into_inner().label.filter(|s| !s.is_empty());
    if let Some(l) = &label {
        if let Err(e) = crate::validate::validate_backup_label(l) {
            return HttpResponse::BadRequest().body(e.to_string());
        }
    }
    let (saves_dir, backup_dir, server_name) = {
        let cfg = state.config.read();
        (
            state.dirs.saves_dir(&cfg),
            state.dirs.backups_dir(),
            cfg.server_name.clone(),
        )
    };
    match create_snapshot(&saves_dir, &backup_dir, &server_name, label.as_deref()) {
        Ok(snap) => {
            let filename = snap
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let size = snap.metadata().map(|m| m.len() as i64).ok();
            let db = state.db.lock();
            let _ = db.record_backup(&filename, label.as_deref(), size, &server_name, "web");
            HttpResponse::Found()
                .insert_header(("Location", "/backups?created=1"))
                .finish()
        }
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}
