pub mod handlers;
pub mod state;

use std::sync::Arc;

use actix_session::config::PersistentSession;
use actix_session::storage::CookieSessionStore;
use actix_session::SessionMiddleware;
use actix_web::cookie::time::Duration as CookieDuration;
use actix_web::cookie::Key;
use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use anyhow::{Context, Result};
use parking_lot::{Mutex, RwLock};
use rust_embed::RustEmbed;

use crate::config::SafehouseConfig;
use crate::db::Database;
use crate::dirs::SafehouseDirs;
use state::AppState;

#[derive(RustEmbed)]
#[folder = "src/assets/"]
struct Assets;

async fn serve_static(path: web::Path<String>) -> HttpResponse {
    match Assets::get(&path) {
        Some(file) => {
            let ct = match path.rsplit('.').next() {
                Some("css") => "text/css",
                Some("js") => "application/javascript",
                _ => "application/octet-stream",
            };
            HttpResponse::Ok()
                .content_type(ct)
                .body(file.data.into_owned())
        }
        None => HttpResponse::NotFound().finish(),
    }
}

/// Start the web server and return a handle for graceful shutdown.
/// Caller should call `handle.stop(true)` to drain in-flight requests.
pub async fn run_server(
    bind: &str,
    port: u16,
    config: SafehouseConfig,
    dirs: SafehouseDirs,
    db: Database,
    docker: bollard::Docker,
) -> Result<actix_web::dev::ServerHandle> {
    let key_bytes = config.session_key_bytes();
    let session_key = Key::from(&key_bytes);
    let state = web::Data::new(AppState {
        db: Arc::new(Mutex::new(db)),
        config: Arc::new(RwLock::new(config)),
        dirs: Arc::new(dirs),
        http: reqwest::Client::new(),
        docker: Arc::new(docker),
    });

    let addr = format!("{bind}:{port}");
    tracing::info!("Safehouse starting on http://{addr}");

    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(middleware::NormalizePath::trim())
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), session_key.clone())
                    .session_lifecycle(
                        PersistentSession::default().session_ttl(CookieDuration::days(7)),
                    )
                    .cookie_secure(false) // safehouse runs on plain HTTP; set true behind TLS proxy
                    .build(),
            )
            .route("/static/{path:.*}", web::get().to(serve_static))
            .service(handlers::auth::login_page)
            .service(handlers::auth::login_submit)
            .service(handlers::auth::logout)
            .service(handlers::dashboard::dashboard)
            .service(handlers::configs::config_page)
            .service(handlers::configs::config_set)
            .service(handlers::mods::mods_page)
            .service(handlers::mods::mods_add)
            .service(handlers::mods::mods_sync)
            .service(handlers::mods::mods_remove)
            .service(handlers::backups::backups_page)
            .service(handlers::backups::backup_create)
            .service(handlers::console::console_page)
            .service(handlers::console::console_exec)
            .service(handlers::logs::logs_page)
    })
    .bind(&addr)
    .with_context(|| format!("cannot bind to {addr}"))?
    .run();
    let handle = server.handle();

    // Spawn the server as a background task so the caller can orchestrate shutdown
    tokio::spawn(server);

    Ok(handle)
}
