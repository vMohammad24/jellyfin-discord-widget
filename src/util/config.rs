use base64::Engine;
use sha2::{Digest, Sha512};
use std::env;
use tracing::info;

#[derive(Clone, Debug)]
pub struct Config {
	pub database_url: String,
	pub log_level: String,
	pub session_secret: Vec<u8>,
	pub encryption_key_bytes: Vec<u8>,
	pub discord_client_id: String,
	pub discord_client_secret: String,
	pub discord_bot_token: String,
	pub discord_redirect_uri: String,
	pub discord_scopes: String,
	pub port: u16,
	pub host: String,
}

impl Config {
	pub fn load() -> Result<Self, String> {
		let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());

		let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
			"postgres://jfw:password@localhost:5432/jfw?sslmode=disable".to_string()
		});

		if database_url.trim().is_empty() {
			return Err("DATABASE_URL environment variable is empty".to_string());
		}

		let is_debug = cfg!(debug_assertions);

		let discord_client_id = match env::var("DISCORD_CLIENT_ID") {
			Ok(id) if !id.trim().is_empty() => id,
			_ => {
				if is_debug {
					info!("No DISCORD_CLIENT_ID found, OAuth login will fail until set");
					"".to_string()
				} else {
					return Err(
						"DISCORD_CLIENT_ID environment variable is required in production"
							.to_string(),
					);
				}
			}
		};

		let discord_client_secret = match env::var("DISCORD_CLIENT_SECRET") {
			Ok(secret) if !secret.trim().is_empty() => secret,
			_ => {
				if is_debug {
					info!("No DISCORD_CLIENT_SECRET found, OAuth callback will fail until set");
					"".to_string()
				} else {
					return Err(
						"DISCORD_CLIENT_SECRET environment variable is required in production"
							.to_string(),
					);
				}
			}
		};

		let discord_bot_token = match env::var("DISCORD_BOT_TOKEN") {
			Ok(t) if !t.trim().is_empty() => t,
			_ => {
				if is_debug {
					info!(
						"No DISCORD_BOT_TOKEN found, updating widgets profile will fail until set"
					);
					"".to_string()
				} else {
					return Err(
						"DISCORD_BOT_TOKEN environment variable is required in production"
							.to_string(),
					);
				}
			}
		};

		let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
		let port_str = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
		let port: u16 = port_str
			.parse()
			.map_err(|e| format!("Invalid PORT environment variable '{}': {}", port_str, e))?;

		let discord_redirect_uri = env::var("DISCORD_REDIRECT_URI").unwrap_or_else(|_| {
			let domain = env::var("DOMAIN").expect(
				"DOMAIN environment variable must be set if DISCORD_REDIRECT_URI is missing",
			);

			let clean_domain = domain.trim_end_matches('/');

			if clean_domain.starts_with("http://") || clean_domain.starts_with("https://") {
				format!("{}/auth/discord/callback", clean_domain)
			} else {
				format!("http://{}:{}/auth/discord/callback", clean_domain, port)
			}
		});

		let discord_scopes = env::var("DISCORD_SCOPES")
			.unwrap_or_else(|_| "identify openid sdk.social_layer".to_string());

		let encryption_key = env::var("ENCRYPTION_KEY")
			.map_err(|_| "ENCRYPTION_KEY environment variable is required".to_string())?;
		if encryption_key.trim().is_empty() {
			return Err("ENCRYPTION_KEY environment variable is empty".to_string());
		}

		let key_bytes = base64::engine::general_purpose::STANDARD
			.decode(&encryption_key)
			.map_err(|e| format!("Invalid Base64 in ENCRYPTION_KEY: {}", e))?;
		if key_bytes.len() != 32 {
			return Err(format!(
				"ENCRYPTION_KEY must decode to exactly 32 bytes (256 bits) for AES-256-GCM. Decoded length was {} bytes.",
				key_bytes.len()
			));
		}

		let session_secret = match env::var("SESSION_SECRET") {
			Ok(s) => {
				if s.len() < 64 {
					return Err(format!(
						"SESSION_SECRET is too short ({} chars), it should be at least 64 characters for security",
						s.len()
					));
				}
				let mut hasher = Sha512::new();
				hasher.update(s.as_bytes());
				hasher.finalize().to_vec()
			}
			Err(_) => {
				if is_debug {
					info!(
						"No SESSION_SECRET found, generating temporary random key for development"
					);
					let mut key = vec![0u8; 64];
					use rand::RngExt;
					let mut rng = rand::rng();
					rng.fill(&mut key[..]);
					key
				} else {
					return Err("SESSION_SECRET environment variable is required in production (min 64 chars)".to_string());
				}
			}
		};

		Ok(Self {
			database_url,
			log_level,
			session_secret,
			encryption_key_bytes: key_bytes,
			discord_client_id,
			discord_client_secret,
			discord_bot_token,
			discord_redirect_uri,
			discord_scopes,
			port,
			host,
		})
	}
}
