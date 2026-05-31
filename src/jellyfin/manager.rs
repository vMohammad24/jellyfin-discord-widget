use askama::Template;
use dashmap::DashMap;
use moka::future::Cache;
use reqwest::Client;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PlaybackStatus {
	Playing { title: String, type_: String },
	Idle,
	Offline,
}

#[derive(Template)]
#[template(path = "playback_status.html")]
struct PlaybackStatusTemplate<'a> {
	status: &'a PlaybackStatus,
}

impl PlaybackStatus {
	pub fn to_html(&self) -> String {
		let template = PlaybackStatusTemplate { status: self };
		template.render().unwrap_or_else(|e| {
			tracing::error!(error = %e, "Failed to render PlaybackStatusTemplate");
			"Error rendering status".to_string()
		})
	}
}

pub struct JellyfinManager {
	pub client: Client,
	pub user_cache: Cache<String, String>,
	pub playback_cache: Cache<String, PlaybackStatus>,
	pub payload_cache: Cache<String, [u8; 32]>,
	pub status_senders: DashMap<String, tokio::sync::watch::Sender<String>>,
}

impl JellyfinManager {
	pub fn new(client: Client) -> Self {
		Self {
			client,
			user_cache: Cache::builder()
				.time_to_live(Duration::from_secs(60 * 5))
				.build(),
			playback_cache: Cache::builder()
				.time_to_live(Duration::from_secs(60 * 5))
				.build(),
			payload_cache: Cache::builder()
				.time_to_live(Duration::from_secs(60 * 60 * 24))
				.build(),
			status_senders: DashMap::new(),
		}
	}

	pub fn get_status_rx(&self, discord_id: &str) -> tokio::sync::watch::Receiver<String> {
		self.status_senders
			.entry(discord_id.to_string())
			.or_insert_with(|| {
				let (tx, _rx) = tokio::sync::watch::channel("".to_string());
				tx
			})
			.value()
			.subscribe()
	}

	pub fn broadcast_status(&self, discord_id: &str, html: String) {
		if let Some(tx) = self.status_senders.get(discord_id) {
			let _ = tx.send(html);
		}
	}

	pub fn cleanup_orphaned_senders(&self) {
		self.status_senders
			.retain(|_, sender| sender.receiver_count() > 0);
	}
}
