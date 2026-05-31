use thiserror::Error;

#[derive(Error, Debug)]
pub enum SyncError {
	#[error("Database error: {0}")]
	Db(#[from] sqlx::Error),
	#[error("Discord error: {0}")]
	Discord(#[from] crate::discord::error::DiscordError),
	#[error("Jellyfin error: {0}")]
	Jellyfin(#[from] crate::jellyfin::error::JellyfinError),
	#[error("Crypto error: {0}")]
	Crypto(#[from] crate::util::crypto::CryptoError),
	#[error("Generic error: {0}")]
	Generic(String),
}
