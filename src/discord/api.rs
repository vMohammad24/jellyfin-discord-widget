use crate::discord::error::DiscordError;
use reqwest::Client;
use serde_json::Value;

#[derive(Clone)]
pub struct DiscordClient {
	client: Client,
}

impl DiscordClient {
	pub fn new(client: Client) -> Self {
		Self { client }
	}

	pub async fn patch_profile(
		&self,
		client_id: &str,
		bot_token: &str,
		user_id: &str,
		external_user_id: &str,
		payload: Value,
	) -> Result<(), DiscordError> {
		let url = format!(
			"https://discord.com/api/v10/applications/{}/users/{}/identities/{}/profile",
			client_id, user_id, external_user_id
		);

		let mut attempts = 0;
		loop {
			let res = self
				.client
				.patch(&url)
				.header("Authorization", format!("Bot {}", bot_token))
				.json(&payload)
				.send()
				.await?;

			if res.status().is_client_error() || res.status().is_server_error() {
				let status = res.status().as_u16();
				if status == 429 && attempts < 3 {
					attempts += 1;
					let retry_after = res
						.headers()
						.get("X-RateLimit-Reset-After")
						.and_then(|h| h.to_str().ok())
						.and_then(|s| s.parse::<f64>().ok())
						.unwrap_or(5.0);

					tracing::warn!(
						retry_after = retry_after,
						attempt = attempts,
						"Discord rate limit hit during patch_profile! Sleeping task before retry"
					);
					tokio::time::sleep(std::time::Duration::from_secs_f64(retry_after)).await;
					continue;
				}

				let text = res.text().await.unwrap_or_default();
				if status == 401 {
					return Err(DiscordError::Unauthorized(text));
				}
				if status == 429 {
					return Err(DiscordError::RateLimited);
				}
				return Err(DiscordError::AuthenticationFailed(text));
			}

			tracing::debug!(
				"Successfully patched Discord profile for user {} with payload: {:?}, response: {:?}",
				user_id,
				payload,
				res.text().await.unwrap_or_default()
			);
			break;
		}

		Ok(())
	}

	pub async fn update_widgets(
		&self,
		client_id: &str,
		access_token: &str,
	) -> Result<(), DiscordError> {
		let url = "https://discord.com/api/v10/users/@me/widgets";
		let payload = serde_json::json!({
			"widgets": [
				{
					"data": {
						"type": "application",
						"application_id": client_id
					}
				}
			]
		});

		let mut attempts = 0;
		loop {
			let res = self
				.client
				.put(url)
				.bearer_auth(access_token)
				.json(&payload)
				.send()
				.await?;

			if res.status().is_client_error() || res.status().is_server_error() {
				let status = res.status().as_u16();
				if status == 429 && attempts < 3 {
					attempts += 1;
					let retry_after = res
						.headers()
						.get("Retry-After")
						.and_then(|h| h.to_str().ok())
						.and_then(|s| s.parse::<f64>().ok())
						.unwrap_or(5.0);

					tracing::warn!(
						retry_after = retry_after,
						attempt = attempts,
						"Discord rate limit hit during update_widgets! Sleeping task before retry"
					);
					tokio::time::sleep(std::time::Duration::from_secs_f64(retry_after)).await;
					continue;
				}

				let text = res.text().await.unwrap_or_default();
				if status == 401 {
					return Err(DiscordError::Unauthorized(text));
				}
				if status == 429 {
					return Err(DiscordError::RateLimited);
				}
				return Err(DiscordError::AuthenticationFailed(text));
			}
			break;
		}

		Ok(())
	}
}

impl Default for DiscordClient {
	fn default() -> Self {
		Self::new(Client::new())
	}
}
