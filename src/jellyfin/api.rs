use crate::jellyfin::entities::{JellyfinLibraryItemsResponse, JellyfinSession, PlayHistoryItem};
use crate::jellyfin::error::JellyfinError;
use reqwest::Client;
use serde_json::json;

#[derive(Clone)]
pub struct JellyfinClient {
	client: Client,
	pub base_url: String,
}

impl JellyfinClient {
	pub fn new(base_url: String, client: Client) -> Self {
		let clean_url = base_url.split('?').next().unwrap_or(&base_url);
		let clean_url = clean_url.split('#').next().unwrap_or(clean_url);
		let clean_url = clean_url.trim_end_matches('/').to_string();
		Self {
			client,
			base_url: clean_url,
		}
	}

	pub async fn authenticate_user_pass(
		&self,
		username: &str,
		password: &str,
	) -> Result<(String, String), JellyfinError> {
		let url = format!("{}/Users/AuthenticateByName", self.base_url);
		let payload = json!({
			"Username": username,
			"Pw": password
		});

		let res = self
            .client
            .post(&url)
            .header("X-Emby-Authorization", "MediaBrowser Client=\"Jellyfin Discord Widget\", Device=\"Server\", DeviceId=\"jfw\", Version=\"1.0.0\"")
            .json(&payload)
            .send()
            .await?;

		if !res.status().is_success() {
			return Err(JellyfinError::AuthenticationFailed(
				"Invalid credentials".to_string(),
			));
		}

		let data: serde_json::Value = res.json().await?;
		let token = data["AccessToken"].as_str().unwrap_or_default().to_string();
		let user_id = data["User"]["Id"].as_str().unwrap_or_default().to_string();

		Ok((token, user_id))
	}

	pub async fn initiate_quick_connect(&self) -> Result<(String, String), JellyfinError> {
		let url = format!("{}/QuickConnect/Initiate", self.base_url);
		let res = self
            .client
            .post(&url)
            .header(
                "X-Emby-Authorization",
                "MediaBrowser Client=\"Jellyfin Discord Widget\", Device=\"Server\", DeviceId=\"jfw\", Version=\"1.0.0\""
            )
            .send()
            .await?;

		if !res.status().is_success() {
			return Err(JellyfinError::AuthenticationFailed(
				"Quick connect initiation failed".to_string(),
			));
		}

		let data: serde_json::Value = res.json().await?;
		let secret = data["Secret"].as_str().unwrap_or_default().to_string();
		let code = data["Code"].as_str().unwrap_or_default().to_string();

		if secret.is_empty() || code.is_empty() {
			return Err(JellyfinError::AuthenticationFailed(
				"Quick connect initiation response invalid".to_string(),
			));
		}

		Ok((secret, code))
	}

	pub async fn check_quick_connect(
		&self,
		secret: &str,
	) -> Result<Option<(String, String)>, JellyfinError> {
		let check_url = format!("{}/QuickConnect/Connect?Secret={}", self.base_url, secret);
		let res = self.client.get(&check_url).send().await?;

		if !res.status().is_success() {
			return Err(JellyfinError::AuthenticationFailed(
				"Quick connect failed or expired".to_string(),
			));
		}

		let check_data: serde_json::Value = res.json().await?;

		if check_data["Authenticated"].as_bool() != Some(true) {
			return Ok(None);
		}
		let post_url = format!("{}/Users/AuthenticateWithQuickConnect", self.base_url);
		let payload = json!({ "Secret": secret });

		let post_res = self
            .client
            .post(&post_url)
            .header(
                "X-Emby-Authorization",
                "MediaBrowser Client=\"Jellyfin Discord Widget\", Device=\"Server\", DeviceId=\"jfw\", Version=\"1.0.0\""
            )
            .json(&payload)
            .send()
            .await?;

		if !post_res.status().is_success() {
			return Err(JellyfinError::AuthenticationFailed(
				"Failed to finalize quick connect login".to_string(),
			));
		}

		let auth_data: serde_json::Value = post_res.json().await?;
		let token = auth_data["AccessToken"]
			.as_str()
			.unwrap_or_default()
			.to_string();
		let user_id = auth_data["User"]["Id"]
			.as_str()
			.unwrap_or_default()
			.to_string();

		if token.is_empty() || user_id.is_empty() {
			return Err(JellyfinError::AuthenticationFailed(
				"Authentication result missing token or user ID".to_string(),
			));
		}

		Ok(Some((token, user_id)))
	}

	pub async fn verify_token(
		&self,
		access_token: &str,
		expected_user_id: &str,
	) -> Result<(), JellyfinError> {
		let url = format!("{}/Users/Me", self.base_url);
		let res = self
			.client
			.get(&url)
			.header("X-Emby-Token", access_token)
			.send()
			.await?;

		if !res.status().is_success() {
			return Err(JellyfinError::AuthenticationFailed(
				"Token verification failed".to_string(),
			));
		}

		let data: serde_json::Value = res.json().await?;
		let user_id = data["Id"].as_str().unwrap_or_default();

		if user_id != expected_user_id {
			return Err(JellyfinError::AuthenticationFailed(
				"Access token belongs to a different user".to_string(),
			));
		}

		Ok(())
	}

	pub async fn get_stats(
		&self,
		access_token: &str,
		user_id: &str,
	) -> Result<JellyfinSession, JellyfinError> {
		let url = format!("{}/Sessions", self.base_url);
		let res = self
			.client
			.get(&url)
			.header("X-Emby-Token", access_token)
			.send()
			.await?;

		if !res.status().is_success() {
			return Err(JellyfinError::AuthenticationFailed(
				"Failed to fetch sessions".to_string(),
			));
		}

		let sessions: Vec<JellyfinSession> = res.json().await?;
		let session = sessions
			.into_iter()
			.find(|s| s.user_id == user_id && s.now_playing_item.is_some())
			.unwrap_or(JellyfinSession {
				id: "".to_string(),
				user_id: user_id.to_string(),
				now_playing_item: None,
				play_state: None,
			});

		Ok(session)
	}

	pub async fn get_play_history(
		&self,
		access_token: &str,
		user_id: &str,
	) -> Result<Vec<PlayHistoryItem>, JellyfinError> {
		let url = format!(
			"{}/Users/{}/Items?Recursive=true&IncludeItemTypes=Movie,Series,Audio&SortBy=DatePlayed&SortOrder=Descending&Limit=10&Fields=Overview,ParentLogoItemId,ProductionYear,RunTimeTicks,ExternalUrls",
			self.base_url, user_id
		);
		let res = self
			.client
			.get(&url)
			.header("X-Emby-Token", access_token)
			.send()
			.await?;

		if !res.status().is_success() {
			return Err(JellyfinError::AuthenticationFailed(
				"Failed to fetch fallback recently played items".to_string(),
			));
		}

		let response: JellyfinLibraryItemsResponse = res.json().await?;
		let fallback_history = response
			.items
			.into_iter()
			.map(|item| {
				let image_id = item
					.parent_logo_item_id
					.clone()
					.or_else(|| item.series_id.clone())
					.or_else(|| Some(item.id.clone()));
				let image_url = image_id.map(|id| {
					format!(
						"https://wsrv.nl/?w=1024&h=1024&fit=cover&a=attention&url={}/Items/{}/Images/Primary",
						self.base_url, id
					)
				});

				PlayHistoryItem {
					name: item.name,
					last_watched: "".to_string(),
					id: item.id,
					type_: item.type_,
					play_count: 1,
					total_duration: 0,
					overview: item.overview,
					image: image_url,
					year: item.production_year,
					runtime: item.run_time_ticks,
				}
			})
			.collect();

		Ok(fallback_history)
	}
}
