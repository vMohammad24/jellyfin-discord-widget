use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserApplicationProfilePrimaryData {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub rank_name: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub highest_rank: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub featured_played_character: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserApplicationProfileDynamicField {
	pub name: String,
	pub value: Value,
	pub r#type: u8, // 1 for TEXT, 2 for NUMBER, 3 for IMAGE
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserApplicationProfileData {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub primary: Option<UserApplicationProfilePrimaryData>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub dynamic: Option<Vec<UserApplicationProfileDynamicField>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiscordProfilePatch {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub username: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub metadata: Option<Value>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub data: Option<UserApplicationProfileData>,
}

#[derive(Deserialize, Debug)]
pub struct OAuthTokenResponse {
	pub access_token: String,
	pub refresh_token: String,
	pub scope: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct DiscordUser {
	pub id: String,
}
