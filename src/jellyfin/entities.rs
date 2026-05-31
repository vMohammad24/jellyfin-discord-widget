use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinPlayState {
	pub position_ticks: Option<i64>,
	pub is_paused: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinSession {
	pub id: String,
	pub user_id: String,
	pub now_playing_item: Option<JellyfinLibraryItem>,
	pub play_state: Option<JellyfinPlayState>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinExternalUrl {
	pub name: Option<String>,
	pub url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinLibraryItem {
	pub id: String,
	pub name: String,
	#[serde(rename = "Type")]
	pub type_: String,
	pub overview: Option<String>,
	pub parent_logo_item_id: Option<String>,
	pub series_id: Option<String>,
	pub production_year: Option<i32>,
	pub run_time_ticks: Option<i64>,
	pub external_urls: Option<Vec<JellyfinExternalUrl>>,
	pub series_name: Option<String>,
	pub artists: Option<Vec<String>>,
	pub album: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinLibraryItemsResponse {
	pub items: Vec<JellyfinLibraryItem>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayHistoryItem {
	pub name: String,
	pub last_watched: String,
	pub id: String,
	pub type_: String,
	pub play_count: i32,
	pub total_duration: i32,
	pub overview: Option<String>,
	pub image: Option<String>,
	pub year: Option<i32>,
	pub runtime: Option<i64>,
}
