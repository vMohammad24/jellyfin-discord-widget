use super::connect::fetch_and_cache_stats;
use crate::jellyfin::manager::JellyfinManager;
use crate::util::session::AuthenticatedUser;
use actix_web::{HttpResponse, Responder, get, web};
use std::sync::Arc;

#[get("/api/jellyfin/status")]
pub async fn get_jellyfin_status(
	user: AuthenticatedUser,
	db: web::Data<crate::db::Db>,
	jellyfin_manager: web::Data<Arc<JellyfinManager>>,
) -> impl Responder {
	let Ok(Some(user_record)) = db.get_user(&user.discord_id).await else {
		return HttpResponse::Ok()
			.body("<div class='status-unconnected'>Not connected to Jellyfin</div>");
	};

	let (Some(url), Some(uid), Some(enc_token)) = (
		user_record.jellyfin_url,
		user_record.jellyfin_user_id,
		user_record.jellyfin_access_token,
	) else {
		return HttpResponse::Ok()
			.body("<div class='status-unconnected'>Not connected to Jellyfin</div>");
	};

	match jellyfin_manager.playback_cache.get(&user.discord_id).await {
		Some(status) => HttpResponse::Ok().body(status.to_html()),
		None => {
			let discord_id = user.discord_id.clone();
			let jellyfin_manager_clone = jellyfin_manager.get_ref().clone();

			tokio::spawn(async move {
				fetch_and_cache_stats(discord_id, url, uid, enc_token, jellyfin_manager_clone)
					.await;
			});

			HttpResponse::Ok().body("<div class='status-loading'>Loading playback status...</div>")
		}
	}
}
