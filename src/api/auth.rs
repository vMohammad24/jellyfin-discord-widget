use crate::discord::entities::{DiscordUser, OAuthTokenResponse};
use crate::jellyfin::manager::JellyfinManager;
use crate::util::config::Config;
use actix_session::Session;
use actix_web::{HttpResponse, Responder, get, web};
use rand::distr::{Alphanumeric, SampleString};
use serde::Deserialize;
use std::sync::Arc;
use tracing::info;

#[derive(Deserialize)]
pub struct DiscordCallback {
	pub code: Option<String>,
	pub state: Option<String>,
	pub error: Option<String>,
}

#[get("/login")]
pub async fn login(session: Session, config: web::Data<Arc<Config>>) -> impl Responder {
	let client_id = &config.discord_client_id;
	let redirect_uri = &config.discord_redirect_uri;
	let scopes = &config.discord_scopes;

	let state: String = Alphanumeric.sample_string(&mut rand::rng(), 32);

	let _ = session.insert("oauth_state", &state);

	let auth_url = format!(
		"https://discord.com/api/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
		client_id,
		urlencoding::encode(redirect_uri),
		urlencoding::encode(scopes),
		state
	);

	web::Redirect::to(auth_url).temporary()
}

#[get("/callback")]
pub async fn callback(
	session: Session,
	query: web::Query<DiscordCallback>,
	db: web::Data<crate::db::Db>,
	config: web::Data<Arc<Config>>,
	jellyfin: web::Data<Arc<JellyfinManager>>,
) -> impl Responder {
	if let Some(err) = &query.error {
		tracing::error!(error = %err, "OAuth callback error parameter");
		return HttpResponse::BadRequest().body(format!("OAuth error: {}", err));
	}

	let Some(code) = &query.code else {
		return HttpResponse::BadRequest().body("Missing code");
	};

	let Some(query_state) = &query.state else {
		return HttpResponse::BadRequest().body("Missing state");
	};

	let session_state: Option<String> = session.get("oauth_state").unwrap_or_default();
	if session_state.as_deref() != Some(query_state) {
		return HttpResponse::BadRequest().body("Invalid state");
	}

	session.remove("oauth_state");

	let client_id = &config.discord_client_id;
	let client_secret = &config.discord_client_secret;
	let redirect_uri = &config.discord_redirect_uri;

	let client = jellyfin.client.clone();

	let params = [
		("client_id", client_id.as_str()),
		("client_secret", client_secret.as_str()),
		("grant_type", "authorization_code"),
		("code", code.as_str()),
		("redirect_uri", redirect_uri.as_str()),
	];

	let token_res = match client
		.post("https://discord.com/api/v10/oauth2/token")
		.form(&params)
		.send()
		.await
	{
		Ok(res) if res.status().is_success() => match res.json::<OAuthTokenResponse>().await {
			Ok(token) => token,
			Err(e) => {
				tracing::error!(error = %e, "Failed to parse token response JSON");
				return HttpResponse::InternalServerError().body("Failed to parse token response");
			}
		},
		Ok(res) => {
			let status = res.status();
			let response_text = res.text().await.unwrap_or_default();
			tracing::error!(
				status = %status,
				response = %response_text,
				"Token exchange failed"
			);
			return HttpResponse::InternalServerError().body("Failed to exchange token");
		}
		Err(e) => {
			tracing::error!(error = %e, "Token exchange request failed");
			return HttpResponse::InternalServerError().body("Failed to exchange token");
		}
	};

	let granted_scopes: Vec<&str> = token_res
		.scope
		.as_ref()
		.map(|s| s.split_whitespace().collect())
		.unwrap_or_default();

	let has_openid = granted_scopes.contains(&"openid");
	let has_identify = granted_scopes.contains(&"identify") || granted_scopes.contains(&"identity");
	let has_social = granted_scopes.contains(&"sdk.social_layer");

	if !has_openid || !has_identify || !has_social {
		tracing::error!(
			"User modified requested scopes. Granted: {:?}",
			token_res.scope
		);
		return HttpResponse::BadRequest()
			.body("Missing required OAuth scopes. Please authorize all requested permissions.");
	}

	let user_res = match client
		.get("https://discord.com/api/v10/users/@me")
		.bearer_auth(&token_res.access_token)
		.send()
		.await
	{
		Ok(res) if res.status().is_success() => match res.json::<DiscordUser>().await {
			Ok(user) => user,
			Err(e) => {
				tracing::error!(error = %e, "Failed to parse user info response JSON");
				return HttpResponse::InternalServerError().body("Failed to parse user info");
			}
		},
		Ok(res) => {
			let status = res.status();
			let response_text = res.text().await.unwrap_or_default();
			tracing::error!(
				status = %status,
				response = %response_text,
				"User info fetch failed"
			);
			return HttpResponse::InternalServerError().body("Failed to fetch user info");
		}
		Err(e) => {
			tracing::error!(error = %e, "User info request failed");
			return HttpResponse::InternalServerError().body("Failed to fetch user info");
		}
	};

	info!(user_id = %user_res.id, "Successfully authenticated Discord user");

	let enc_access = match crate::util::crypto::encrypt_string(&token_res.access_token) {
		Ok(enc) => enc,
		Err(e) => {
			tracing::error!(error = %e, "Failed to encrypt access token");
			return HttpResponse::InternalServerError().body("Internal server error");
		}
	};

	let enc_refresh = match crate::util::crypto::encrypt_string(&token_res.refresh_token) {
		Ok(enc) => enc,
		Err(e) => {
			tracing::error!(error = %e, "Failed to encrypt refresh token");
			return HttpResponse::InternalServerError().body("Internal server error");
		}
	};

	if let Err(e) = db
		.upsert_discord_tokens(&user_res.id, &enc_access, &enc_refresh)
		.await
	{
		tracing::error!(user_id = %user_res.id, error = %e, "Failed to upsert Discord tokens in DB");
		return HttpResponse::InternalServerError().body("Database error");
	}

	let _ = session.insert("discord_id", &user_res.id);

	HttpResponse::Found()
		.append_header(("Location", "/dashboard"))
		.finish()
}

pub fn config(cfg: &mut web::ServiceConfig) {
	cfg.service(web::scope("/auth/discord").service(login).service(callback));
}
