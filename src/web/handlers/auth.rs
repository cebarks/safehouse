use actix_session::Session;
use actix_web::{get, post, web, HttpResponse, Responder};
use askama::Template;
use serde::Deserialize;

use crate::web::state::AppState;

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    error: Option<String>,
}

#[get("/login")]
pub async fn login_page(session: Session) -> impl Responder {
    if session.get::<String>("user").unwrap_or(None).is_some() {
        return HttpResponse::Found()
            .insert_header(("Location", "/"))
            .finish();
    }
    let tmpl = LoginTemplate { error: None };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(tmpl.render().unwrap_or_default())
}

#[derive(Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
}

#[post("/login")]
pub async fn login_submit(
    form: web::Form<LoginForm>,
    session: Session,
    state: web::Data<AppState>,
) -> impl Responder {
    let db = state.db.lock();
    let ok = db
        .verify_password(&form.username, &form.password)
        .unwrap_or(false);
    drop(db);

    if ok {
        let _ = session.insert("user", &form.username);
        HttpResponse::Found()
            .insert_header(("Location", "/"))
            .finish()
    } else {
        let tmpl = LoginTemplate {
            error: Some("Invalid credentials".to_string()),
        };
        HttpResponse::Unauthorized()
            .content_type("text/html")
            .body(tmpl.render().unwrap_or_default())
    }
}

#[get("/logout")]
pub async fn logout(session: Session) -> impl Responder {
    session.purge();
    HttpResponse::Found()
        .insert_header(("Location", "/login"))
        .finish()
}

/// Middleware helper: redirect to /login if no session user.
pub fn require_auth(session: &Session) -> Option<HttpResponse> {
    if session.get::<String>("user").ok().flatten().is_none() {
        Some(
            HttpResponse::Found()
                .insert_header(("Location", "/login"))
                .finish(),
        )
    } else {
        None
    }
}
