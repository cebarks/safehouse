use actix_session::Session;
use actix_web::{get, web, HttpResponse, Responder};
use askama::Template;

use crate::web::handlers::auth::require_auth;
use crate::web::state::AppState;

#[derive(Template)]
#[template(path = "logs.html")]
struct LogsTemplate {
    lines: Vec<String>,
}

#[get("/logs")]
pub async fn logs_page(session: Session, state: web::Data<AppState>) -> impl Responder {
    if let Some(r) = require_auth(&session) {
        return r;
    }
    let cfg = state.config.read();
    let lines = state
        .dirs
        .latest_log(&cfg)
        .and_then(|p| crate::pz::logs::tail_lines(&p, 200).ok())
        .unwrap_or_default();
    drop(cfg);
    let tmpl = LogsTemplate { lines };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(tmpl.render().unwrap_or_default())
}
