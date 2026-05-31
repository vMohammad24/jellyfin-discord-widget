use crate::db::Db;
use crate::discord::api::DiscordClient;
use crate::jellyfin::manager::{JellyfinManager, PlaybackStatus};
use crate::util::config::Config;
use crate::util::crypto::decrypt_string;
use tracing::{debug, error};

use super::error::SyncError;
use super::profile::build_profile_payload;

pub async fn refresh_discord_token(
	db: &Db,
	http_client: &reqwest::Client,
	config: &Config,
	discord_id: &str,
	enc_refresh_token: &str,
) -> Result<String, SyncError> {
	let dec_refresh = decrypt_string(enc_refresh_token)?;

	let params = [
		("client_id", config.discord_client_id.as_str()),
		("client_secret", config.discord_client_secret.as_str()),
		("grant_type", "refresh_token"),
		("refresh_token", dec_refresh.as_str()),
	];

	let res = http_client
		.post("https://discord.com/api/v10/oauth2/token")
		.form(&params)
		.send()
		.await
		.map_err(crate::discord::error::DiscordError::RequestFailed)?;

	if !res.status().is_success() {
		let status = res.status();
		let text = res.text().await.unwrap_or_default();
		return Err(SyncError::Generic(format!(
			"Failed to refresh Discord token (status {}): {}",
			status, text
		)));
	}

	let token_res = res
		.json::<crate::discord::entities::OAuthTokenResponse>()
		.await
		.map_err(crate::discord::error::DiscordError::RequestFailed)?;
	let new_enc_access = crate::util::crypto::encrypt_string(&token_res.access_token)?;
	let new_enc_refresh = crate::util::crypto::encrypt_string(&token_res.refresh_token)?;

	db.upsert_discord_tokens(discord_id, &new_enc_access, &new_enc_refresh)
		.await?;

	Ok(token_res.access_token)
}

pub async fn sync_single_user(
	db: &Db,
	user: &crate::db::UserRecord,
	jellyfin: &JellyfinManager,
	discord: &DiscordClient,
	config: &Config,
) -> Result<(), SyncError> {
	let url = match &user.jellyfin_url {
		Some(u) if !u.trim().is_empty() => u,
		_ => return Ok(()),
	};
	let uid = match &user.jellyfin_user_id {
		Some(id) if !id.trim().is_empty() => id,
		_ => return Ok(()),
	};
	let enc_token = match &user.jellyfin_access_token {
		Some(t) if !t.trim().is_empty() => t,
		_ => return Ok(()),
	};

	let token = decrypt_string(enc_token)?;
	let mut discord_access = decrypt_string(&user.discord_access_token)?;

	let jellyfin_client =
		crate::jellyfin::api::JellyfinClient::new(url.clone(), jellyfin.client.clone());

	jellyfin
		.user_cache
		.insert(user.discord_id.clone(), uid.clone())
		.await;

	debug!(
		user_id = %user.discord_id,
		url = %url,
		"Fetching Jellyfin stats"
	);

	let old_status = jellyfin.playback_cache.get(&user.discord_id).await;

	let stats_res = jellyfin_client.get_stats(&token, uid).await;
	let stats = match stats_res {
		Ok(s) => {
			let status = if let Some(item) = &s.now_playing_item {
				let _ = db.update_last_active_at(&user.discord_id).await;
				PlaybackStatus::Playing {
					title: item.name.clone(),
					type_: item.type_.clone(),
				}
			} else {
				PlaybackStatus::Idle
			};

			let status_changed = match &old_status {
				Some(old) => old != &status,
				None => true,
			};

			if status_changed {
				jellyfin
					.playback_cache
					.insert(user.discord_id.clone(), status.clone())
					.await;

				jellyfin.broadcast_status(&user.discord_id, status.to_html());
			}
			s
		}
		Err(e) => {
			let status = PlaybackStatus::Offline;
			let status_changed = match &old_status {
				Some(old) => old != &status,
				None => true,
			};

			if status_changed {
				jellyfin
					.playback_cache
					.insert(user.discord_id.clone(), status.clone())
					.await;

				jellyfin.broadcast_status(&user.discord_id, status.to_html());
			}
			return Err(SyncError::Jellyfin(e));
		}
	};

	if let Err(e) = discord
		.update_widgets(&config.discord_client_id, &discord_access)
		.await
	{
		if matches!(e, crate::discord::error::DiscordError::Unauthorized(_)) {
			debug!(
				user_id = %user.discord_id,
				"Discord token expired/unauthorized, attempting refresh..."
			);
			match refresh_discord_token(
				db,
				&jellyfin.client,
				config,
				&user.discord_id,
				&user.discord_refresh_token,
			)
			.await
			{
				Ok(new_access) => {
					discord_access = new_access;
					if let Err(retry_err) = discord
						.update_widgets(&config.discord_client_id, &discord_access)
						.await
					{
						debug!(
							user_id = %user.discord_id,
							error = %retry_err,
							"Failed to ensure profile widget is active after token refresh"
						);
					} else {
						debug!(
							user_id = %user.discord_id,
							"Successfully updated widgets after token refresh"
						);
					}
				}
				Err(refresh_err) => {
					error!(
						user_id = %user.discord_id,
						error = %refresh_err,
						"Failed to refresh Discord token"
					);
				}
			}
		} else {
			debug!(
				user_id = %user.discord_id,
				error = %e,
				"Failed to ensure profile widget is active"
			);
		}
	}

	let history = jellyfin_client.get_play_history(&token, uid).await?;
	let payload_value = build_profile_payload(&stats, history, &jellyfin_client.base_url)
		.map_err(SyncError::Generic)?;

	let payload_string =
		serde_json::to_string(&payload_value).map_err(|e| SyncError::Generic(e.to_string()))?;

	use sha2::{Digest, Sha256};
	let mut hasher = Sha256::new();
	hasher.update(payload_string.as_bytes());
	let hash_result = hasher.finalize();
	let mut payload_hash = [0u8; 32];
	payload_hash.copy_from_slice(&hash_result);

	if matches!(jellyfin.payload_cache.get(&user.discord_id).await, Some(cached_hash) if cached_hash == payload_hash)
	{
		debug!(
			user_id = %user.discord_id,
			"Profile payload unchanged, skipping Discord patch"
		);
		return Ok(());
	}

	debug!(
		user_id = %user.discord_id,
		"Patching Discord profile"
	);

	discord
		.patch_profile(
			&config.discord_client_id,
			&config.discord_bot_token,
			&user.discord_id,
			uid,
			payload_value,
		)
		.await?;

	jellyfin
		.payload_cache
		.insert(user.discord_id.clone(), payload_hash)
		.await;

	debug!(
		user_id = %user.discord_id,
		"Successfully patched Discord profile"
	);

	Ok(())
}
