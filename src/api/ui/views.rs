use crate::util::session::AuthenticatedUser;
use actix_session::Session;
use actix_web::{HttpResponse, Responder, get, web};
use askama::Template;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate;

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
	pub discord_id: String,
	pub jellyfin_url: String,
}

#[get("/")]
pub async fn index(session: Session, db: web::Data<crate::db::Db>) -> impl Responder {
	if let Ok(Some(discord_id)) = session.get::<String>("discord_id") {
		match db.get_user(&discord_id).await {
			Ok(Some(_)) => {
				return HttpResponse::Found()
					.append_header(("Location", "/dashboard"))
					.finish();
			}
			Ok(None) => {
				let _ = session.remove("discord_id");
			}
			Err(e) => {
				tracing::error!(error = %e, "Database query failed in index handler");
			}
		}
	}

	match IndexTemplate.render() {
		Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
		Err(e) => {
			tracing::error!(error = %e, "Template render failed");
			HttpResponse::InternalServerError().body("Failed to render page")
		}
	}
}

#[get("/dashboard")]
pub async fn dashboard(user: AuthenticatedUser, db: web::Data<crate::db::Db>) -> impl Responder {
	let jellyfin_url = match db.get_user(&user.discord_id).await {
		Ok(Some(u)) => u
			.jellyfin_url
			.unwrap_or_else(|| "Not connected".to_string()),
		_ => "Not connected".to_string(),
	};

	let template = DashboardTemplate {
		discord_id: user.discord_id,
		jellyfin_url,
	};

	match template.render() {
		Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
		Err(e) => {
			tracing::error!(error = %e, "Template render failed");
			HttpResponse::InternalServerError().body("Failed to render page")
		}
	}
}
