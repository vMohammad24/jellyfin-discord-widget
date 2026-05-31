use std::time::Duration;

use crate::discord::entities::{
	DiscordProfilePatch, UserApplicationProfileData, UserApplicationProfileDynamicField,
};
use crate::jellyfin::entities::{JellyfinSession, PlayHistoryItem};
use serde_json::Value;

pub fn truncate_to_100(s: &str) -> String {
	if s.chars().count() > 100 {
		format!("{}...", s.chars().take(97).collect::<String>())
	} else {
		s.to_string()
	}
}

pub fn build_profile_payload(
	stats: &JellyfinSession,
	history: Vec<PlayHistoryItem>,
	base_url: &str,
) -> Result<Value, String> {
	let playing_id = stats.now_playing_item.as_ref().map(|item| item.id.as_str());

	let mut dynamic_fields: Vec<UserApplicationProfileDynamicField> = history
		.into_iter()
		.filter(|hist_item| Some(hist_item.id.as_str()) != playing_id)
		.take(4)
		.enumerate()
		.flat_map(|(i, hist_item)| {
			let index = i + 1;
			let title = truncate_to_100(&hist_item.name);
			let cover = hist_item
				.image
				.clone()
				.unwrap_or_else(|| "https://github.com/jellyfin.png".to_string());

			let desc = match hist_item.type_.as_str() {
				"Movie" => {
					if let Some(y) = hist_item.year
						&& let Some(r) = hist_item.runtime
					{
						format!(
							"{} • {}",
							y,
							humantime::format_duration(Duration::from_mins(
								Duration::from_nanos((r as u64) * 100).as_secs() / 60
							))
						)
					} else {
						"Movie".to_string()
					}
				}
				"Episode" => "TV Show".to_string(),
				"Audio" => "Music".to_string(),
				"Unknown" => "Idle".to_string(),
				_ => hist_item.type_.clone(),
			};

			let desc = truncate_to_100(&desc);

			vec![
				UserApplicationProfileDynamicField {
					name: format!("last_watched_{}_title", index),
					value: serde_json::Value::String(title),
					r#type: 1,
				},
				UserApplicationProfileDynamicField {
					name: format!("last_watched_{}_cover", index),
					value: serde_json::json!({ "url": cover }),
					r#type: 3,
				},
				UserApplicationProfileDynamicField {
					name: format!("last_watched_{}_desc", index),
					value: serde_json::Value::String(desc),
					r#type: 1,
				},
			]
		})
		.collect();

	if let Some(item) = &stats.now_playing_item {
		let cover_url = format!("{}/Items/{}/Images/Primary", base_url, item.id);
		let now_playing_desc = match item.type_.as_str() {
			"Audio" => {
				let artists = item
					.artists
					.as_ref()
					.map(|a| a.join(", "))
					.unwrap_or_else(|| "Unknown Artist".to_string());
				let album = item
					.album
					.clone()
					.unwrap_or_else(|| "Unknown Album".to_string());
				format!(
					"{} • {} • {}",
					artists,
					album,
					item.production_year.unwrap_or_default()
				)
			}
			"Episode" => item
				.series_name
				.clone()
				.unwrap_or_else(|| "TV Show".to_string()),
			_ => {
				if let Some(y) = item.production_year
					&& let Some(r) = item.run_time_ticks
				{
					format!(
						"{} • {}",
						y,
						humantime::format_duration(Duration::from_mins(
							Duration::from_nanos((r as u64) * 100).as_secs() / 60
						))
					)
				} else {
					item.type_.clone()
				}
			}
		};

		dynamic_fields.push(UserApplicationProfileDynamicField {
			name: "now_playing_title".to_string(),
			value: serde_json::Value::String(truncate_to_100(&item.name)),
			r#type: 1,
		});
		dynamic_fields.push(UserApplicationProfileDynamicField {
			name: "now_playing_cover".to_string(),
			value: serde_json::json!({ "url": cover_url }),
			r#type: 3,
		});
		dynamic_fields.push(UserApplicationProfileDynamicField {
			name: "now_playing_desc".to_string(),
			value: serde_json::Value::String(truncate_to_100(&now_playing_desc)),
			r#type: 1,
		});
	} else {
		let default_image = "https://github.com/jellyfin.png".to_string();

		dynamic_fields.push(UserApplicationProfileDynamicField {
			name: "now_playing_title".to_string(),
			value: serde_json::Value::String(truncate_to_100("Not Playing")),
			r#type: 1,
		});
		dynamic_fields.push(UserApplicationProfileDynamicField {
			name: "now_playing_cover".to_string(),
			value: serde_json::json!({ "url": default_image }),
			r#type: 3,
		});
		dynamic_fields.push(UserApplicationProfileDynamicField {
			name: "now_playing_desc".to_string(),
			value: serde_json::Value::String(truncate_to_100("No active media session")),
			r#type: 1,
		});
	}

	let payload = DiscordProfilePatch {
		username: Some("Jellyfin".to_string()),
		metadata: None,
		data: Some(UserApplicationProfileData {
			primary: None,
			dynamic: Some(dynamic_fields),
		}),
	};

	serde_json::to_value(&payload).map_err(|e| e.to_string())
}
