use thiserror::Error;

#[derive(Error, Debug)]
pub enum JellyfinError {
	#[error("Authentication failed: {0}")]
	AuthenticationFailed(String),
	#[error("Request failed: {0}")]
	RequestFailed(#[from] reqwest::Error),
}
