use thiserror::Error;

#[derive(Error, Debug)]
pub enum DiscordError {
	#[error("Authentication failed: {0}")]
	AuthenticationFailed(String),
	#[error("Unauthorized: {0}")]
	Unauthorized(String),
	#[error("Rate limited")]
	RateLimited,
	#[error("Request failed: {0}")]
	RequestFailed(#[from] reqwest::Error),
}
