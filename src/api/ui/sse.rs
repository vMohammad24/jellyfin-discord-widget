use super::connect::fetch_and_cache_stats;
use crate::jellyfin::manager::JellyfinManager;
use crate::util::session::AuthenticatedUser;
use actix_web::{Responder, get, web};
use actix_web_lab::sse;
use std::sync::Arc;

#[get("/api/jellyfin/status/sse")]
pub async fn get_jellyfin_status_sse(
	user: AuthenticatedUser,
	db: web::Data<crate::db::Db>,
	jellyfin_manager: web::Data<Arc<JellyfinManager>>,
	shutdown_rx: web::Data<tokio::sync::watch::Receiver<bool>>,
) -> impl Responder {
	let user_record = match db.get_user(&user.discord_id).await {
		Ok(Some(u)) => u,
		_ => {
			let (tx, rx) = tokio::sync::mpsc::channel(2);
			let _ = tx
				.send(
					sse::Data::new(
						"<div class='status-unconnected'>Not connected to Jellyfin</div>",
					)
					.event("message")
					.into(),
				)
				.await;
			return sse::Sse::from_infallible_receiver(rx);
		}
	};

	let (url, uid, enc_token) = match (
		user_record.jellyfin_url,
		user_record.jellyfin_user_id,
		user_record.jellyfin_access_token,
	) {
		(Some(url), Some(uid), Some(enc_token)) => (url, uid, enc_token),
		_ => {
			let (tx, rx) = tokio::sync::mpsc::channel(2);
			let _ = tx
				.send(
					sse::Data::new(
						"<div class='status-unconnected'>Not connected to Jellyfin</div>",
					)
					.event("message")
					.into(),
				)
				.await;
			return sse::Sse::from_infallible_receiver(rx);
		}
	};

	let (tx, rx) = tokio::sync::mpsc::channel(10);

	let initial_html = match jellyfin_manager.playback_cache.get(&user.discord_id).await {
		Some(status) => status.to_html(),
		None => {
			let discord_id = user.discord_id.clone();
			let jellyfin_manager_clone = jellyfin_manager.get_ref().clone();
			tokio::spawn(async move {
				fetch_and_cache_stats(discord_id, url, uid, enc_token, jellyfin_manager_clone)
					.await;
			});

			"<div class='status-loading'>Loading playback status...</div>".to_string()
		}
	};

	let _ = tx
		.send(sse::Data::new(initial_html).event("message").into())
		.await;

	let mut status_rx = jellyfin_manager.get_status_rx(&user.discord_id);
	let mut shutdown_rx_clone = shutdown_rx.get_ref().clone();

	tokio::spawn(async move {
		loop {
			tokio::select! {
				res = status_rx.changed() => {
					if res.is_err() {
						break;
					}
					let html = status_rx.borrow().clone();
					if !html.is_empty() && tx
						.send(sse::Data::new(html).event("message").into())
						.await
						.is_err()
					{
						break;
					}
				}
				change_res = shutdown_rx_clone.changed() => {
					if change_res.is_err() || *shutdown_rx_clone.borrow() {
						break;
					}
				}
			}
		}
	});

	sse::Sse::from_infallible_receiver(rx)
}
