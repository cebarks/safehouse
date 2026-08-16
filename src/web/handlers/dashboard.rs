use actix_session::Session;
use actix_web::{get, web, HttpResponse, Responder};
use askama::Template;

use crate::container;
use crate::web::handlers::auth::require_auth;
use crate::web::state::AppState;

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    running: bool,
    server_name: String,
    player_count: Option<String>,
    recent_log: Vec<String>,
}

#[get("/")]
pub async fn dashboard(session: Session, state: web::Data<AppState>) -> impl Responder {
    if let Some(r) = require_auth(&session) {
        return r;
    }

    let (server_name, rcon_port, rcon_password) = {
        let cfg = state.config.read();
        (
            cfg.server_name.clone(),
            cfg.rcon_port,
            cfg.rcon_password.clone(),
        )
    };

    let running = container::is_running(&state.docker).await;

    let player_count = if running {
        // Use web::block to avoid blocking the tokio worker thread —
        // RconClient uses std::net::TcpStream with blocking I/O.
        let rcon_pass = rcon_password;
        actix_web::web::block(move || {
            crate::pz::rcon::RconClient::connect("127.0.0.1", rcon_port, &rcon_pass)
                .ok()
                .and_then(|mut rcon| rcon.send_command("players").ok())
        })
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    let recent_log = state
        .dirs
        .latest_log(&state.config.read())
        .and_then(|p| crate::pz::logs::tail_lines(&p, 20).ok())
        .unwrap_or_default();

    let tmpl = DashboardTemplate {
        running,
        server_name,
        player_count,
        recent_log,
    };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(tmpl.render().unwrap_or_default())
}
