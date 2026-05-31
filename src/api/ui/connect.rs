use crate::jellyfin::manager::{JellyfinManager, PlaybackStatus};
use crate::util::session::AuthenticatedUser;
use actix_web::{HttpResponse, Responder, post, web};
use askama::Template;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Template)]
#[template(path = "quick_connect_status.html")]
struct QuickConnectStatusTemplate {
	url: String,
	secret: String,
	code: String,
}

#[derive(Deserialize)]
pub struct ConnectForm {
	pub method: String,
	pub url: String,
	pub user_id: Option<String>,
	pub access_token: Option<String>,
	pub username: Option<String>,
	pub password: Option<String>,
	pub quick_connect_secret: Option<String>,
	pub quick_connect_code: Option<String>,
}

pub async fn fetch_and_cache_stats(
	discord_id: String,
	url: String,
	uid: String,
	enc_token: String,
	jellyfin_manager: Arc<JellyfinManager>,
) {
	let status = match crate::util::crypto::decrypt_string(&enc_token) {
		Ok(token) => {
			let jellyfin_client =
				crate::jellyfin::api::JellyfinClient::new(url, jellyfin_manager.client.clone());
			match jellyfin_client.get_stats(&token, &uid).await {
				Ok(s) => {
					if let Some(item) = &s.now_playing_item {
						PlaybackStatus::Playing {
							title: item.name.clone(),
							type_: item.type_.clone(),
						}
					} else {
						PlaybackStatus::Idle
					}
				}
				Err(_) => PlaybackStatus::Offline,
			}
		}
		Err(_) => PlaybackStatus::Offline,
	};
	jellyfin_manager
		.playback_cache
		.insert(discord_id.clone(), status.clone())
		.await;

	jellyfin_manager.broadcast_status(&discord_id, status.to_html());
}

#[post("/api/jellyfin/connect")]
pub async fn connect_jellyfin(
	user: AuthenticatedUser,
	form: web::Form<ConnectForm>,
	db: web::Data<crate::db::Db>,
	jellyfin_manager: web::Data<Arc<JellyfinManager>>,
) -> impl Responder {
	let mut form = form.into_inner();

	let validated_url = match crate::util::ssrf::validate_url(&form.url) {
		Ok(url) => url.to_string(),
		Err(e) => {
			return HttpResponse::BadRequest().body(format!("<div class='error'>{}</div>", e));
		}
	};
	form.url = validated_url;

	let client = crate::jellyfin::api::JellyfinClient::new(
		form.url.clone(),
		jellyfin_manager.client.clone(),
	);

	let (token, uid) = match form.method.as_str() {
		"token" => {
			let Some(user_id) = form.user_id.take().filter(|uid| !uid.trim().is_empty()) else {
				return HttpResponse::BadRequest()
					.body("<div class='error'>User ID is required</div>");
			};
			let user_id = user_id.trim().to_string();

			let Some(access_token) = form
				.access_token
				.take()
				.filter(|token| !token.trim().is_empty())
			else {
				return HttpResponse::BadRequest()
					.body("<div class='error'>Access token is required</div>");
			};
			let access_token = access_token.trim().to_string();

			if let Err(e) = client.verify_token(&access_token, &user_id).await {
				return HttpResponse::BadRequest().body(format!(
					"<div class='error'>Verification failed: {}</div>",
					e
				));
			}
			(access_token, user_id)
		}
		"credentials" => {
			let Some(username) = form.username.take().filter(|u| !u.trim().is_empty()) else {
				return HttpResponse::BadRequest()
					.body("<div class='error'>Username is required</div>");
			};
			let Some(password) = form.password.take() else {
				return HttpResponse::BadRequest()
					.body("<div class='error'>Password is required</div>");
			};

			match client
				.authenticate_user_pass(username.trim(), &password)
				.await
			{
				Ok(res) => res,
				Err(e) => {
					return HttpResponse::BadRequest()
						.body(format!("<div class='error'>{}</div>", e));
				}
			}
		}
		"quick_initiate" => match client.initiate_quick_connect().await {
			Ok((secret, code)) => {
				let template = QuickConnectStatusTemplate {
					url: form.url.clone(),
					secret,
					code,
				};
				match template.render() {
					Ok(html) => return HttpResponse::Ok().body(html),
					Err(e) => {
						tracing::error!(error = %e, "Template render failed");
						return HttpResponse::InternalServerError().body("Internal server error");
					}
				}
			}
			Err(e) => {
				return HttpResponse::BadRequest()
					.body(format!("<div class='error'>Initiation failed: {}</div>", e));
			}
		},
		"quick_check" => {
			let Some(secret) = form
				.quick_connect_secret
				.as_ref()
				.filter(|s| !s.trim().is_empty())
			else {
				return HttpResponse::BadRequest()
					.body("<div class='error'>Missing Quick Connect Secret</div>");
			};
			let secret = secret.trim().to_string();
			let code = form.quick_connect_code.clone().unwrap_or_default();

			match client.check_quick_connect(&secret).await {
				Ok(Some((token, uid))) => (token, uid),
				Ok(None) => {
					let template = QuickConnectStatusTemplate {
						url: form.url.clone(),
						secret,
						code,
					};
					match template.render() {
						Ok(html) => return HttpResponse::Ok().body(html),
						Err(e) => {
							tracing::error!(error = %e, "Template render failed");
							return HttpResponse::InternalServerError()
								.body("Internal server error");
						}
					}
				}
				Err(e) => {
					return HttpResponse::BadRequest().body(format!(
						"<div class='error'>Quick connect failed: {}</div>",
						e
					));
				}
			}
		}
		_ => return HttpResponse::BadRequest().body("<div class='error'>Invalid method</div>"),
	};

	let enc_token = match crate::util::crypto::encrypt_string(&token) {
		Ok(enc) => enc,
		Err(e) => {
			tracing::error!(error = %e, "Failed to encrypt token");
			return HttpResponse::InternalServerError()
				.body("<div class='error'>Encryption failed</div>");
		}
	};

	if let Err(e) = db
		.update_jellyfin_credentials(&user.discord_id, &form.url, &uid, &enc_token)
		.await
	{
		tracing::error!(
			user_id = %user.discord_id,
			error = %e,
			"Failed to update Jellyfin credentials in DB"
		);
		return HttpResponse::InternalServerError()
			.body("<div class='error'>Database update failed</div>");
	}

	jellyfin_manager
		.playback_cache
		.invalidate(&user.discord_id)
		.await;

	jellyfin_manager.broadcast_status(
		&user.discord_id,
		"<div class='status-loading'>Loading playback status...</div>".to_string(),
	);

	let discord_id = user.discord_id.clone();
	let jellyfin_manager_clone = jellyfin_manager.get_ref().clone();
	let url_val = form.url.clone();
	tokio::spawn(async move {
		fetch_and_cache_stats(discord_id, url_val, uid, enc_token, jellyfin_manager_clone).await;
	});

	HttpResponse::Ok().body(format!(
		"<div class='success'>Connected to {}!</div>",
		form.url
	))
}
