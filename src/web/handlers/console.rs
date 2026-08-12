use actix_session::Session;
use actix_web::{get, post, web, HttpResponse, Responder};
use askama::Template;
use serde::Deserialize;

use crate::web::handlers::auth::require_auth;
use crate::web::state::AppState;

#[derive(Template)]
#[template(path = "console.html")]
struct ConsoleTemplate {
    output: Option<String>,
}

#[get("/console")]
pub async fn console_page(session: Session) -> impl Responder {
    if let Some(r) = require_auth(&session) {
        return r;
    }
    let tmpl = ConsoleTemplate { output: None };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(tmpl.render().unwrap_or_default())
}

#[derive(Deserialize)]
pub struct RconForm {
    command: String,
}

#[post("/console/exec")]
pub async fn console_exec(
    form: web::Form<RconForm>,
    session: Session,
    state: web::Data<AppState>,
) -> impl Responder {
    if let Some(r) = require_auth(&session) {
        return r;
    }
    let (port, pass) = {
        let cfg = state.config.read();
        (cfg.rcon_port, cfg.rcon_password.clone())
    };
    // Use web::block to avoid blocking the tokio worker thread —
    // RconClient uses std::net::TcpStream with blocking I/O.
    let command = form.into_inner().command;
    let output = actix_web::web::block(move || {
        match crate::pz::rcon::RconClient::connect("127.0.0.1", port, &pass) {
            Ok(mut rcon) => rcon
                .send_command(&command)
                .unwrap_or_else(|e| e.to_string()),
            Err(e) => format!("RCON error: {e}"),
        }
    })
    .await
    .unwrap_or_else(|e| format!("Internal error: {e}"));
    // HTMX partial response
    HttpResponse::Ok()
        .content_type("text/plain")
        .body(output)
}
