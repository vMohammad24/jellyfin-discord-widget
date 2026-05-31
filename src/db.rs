use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;

#[derive(Clone)]
pub struct Db {
	pub pool: PgPool,
}

pub struct UserRecord {
	pub discord_id: String,
	pub discord_access_token: String,
	pub discord_refresh_token: String,
	pub jellyfin_url: Option<String>,
	pub jellyfin_user_id: Option<String>,
	pub jellyfin_access_token: Option<String>,
	pub last_active_at: chrono::DateTime<chrono::FixedOffset>,
}

impl Db {
	pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
		let pool = PgPoolOptions::new()
			.max_connections(25)
			.acquire_timeout(Duration::from_secs(3))
			.connect(database_url)
			.await?;

		sqlx::migrate!("./migrations").run(&pool).await?;

		Ok(Self { pool })
	}

	pub async fn get_user(&self, discord_id: &str) -> Result<Option<UserRecord>, sqlx::Error> {
		let user = sqlx::query_as!(
			UserRecord,
			r#"
            SELECT
                discord_id,
                discord_access_token,
                discord_refresh_token,
                jellyfin_url,
                jellyfin_user_id,
                jellyfin_access_token,
                last_active_at
            FROM users
            WHERE discord_id = $1
            "#,
			discord_id
		)
		.fetch_optional(&self.pool)
		.await?;

		Ok(user)
	}

	pub async fn get_users_paginated(
		&self,
		limit: i64,
		after_id: Option<i64>,
	) -> Result<Vec<UserRecord>, sqlx::Error> {
		let users = sqlx::query_as!(
			UserRecord,
			r#"
            SELECT
                discord_id,
                discord_access_token,
                discord_refresh_token,
                jellyfin_url,
                jellyfin_user_id,
                jellyfin_access_token,
                last_active_at
            FROM users
            WHERE
                ($1::bigint IS NULL OR discord_id::bigint > $1)
                AND (
                    last_active_at > NOW() - INTERVAL '24 hours'
                    OR updated_at <= NOW() - INTERVAL '1 hour'
                )
            ORDER BY discord_id::bigint
            LIMIT $2
            "#,
			after_id,
			limit
		)
		.fetch_all(&self.pool)
		.await?;

		Ok(users)
	}

	pub async fn upsert_discord_tokens(
		&self,
		discord_id: &str,
		access_token: &str,
		refresh_token: &str,
	) -> Result<(), sqlx::Error> {
		sqlx::query!(
			r#"
            INSERT INTO users (discord_id, discord_access_token, discord_refresh_token)
            VALUES ($1, $2, $3)
            ON CONFLICT (discord_id) DO UPDATE SET
                discord_access_token = EXCLUDED.discord_access_token,
                discord_refresh_token = EXCLUDED.discord_refresh_token,
                last_active_at = NOW(),
                updated_at = NOW()
            "#,
			discord_id,
			access_token,
			refresh_token
		)
		.execute(&self.pool)
		.await?;

		Ok(())
	}

	pub async fn update_jellyfin_credentials(
		&self,
		discord_id: &str,
		url: &str,
		user_id: &str,
		access_token: &str,
	) -> Result<(), sqlx::Error> {
		sqlx::query!(
			r#"
            UPDATE users SET
                jellyfin_url = $2,
                jellyfin_user_id = $3,
                jellyfin_access_token = $4,
                last_active_at = NOW(),
                updated_at = NOW()
            WHERE discord_id = $1
            "#,
			discord_id,
			url,
			user_id,
			access_token
		)
		.execute(&self.pool)
		.await?;

		Ok(())
	}

	pub async fn update_last_active_at(&self, discord_id: &str) -> Result<(), sqlx::Error> {
		sqlx::query!(
			r#"
            UPDATE users SET
                last_active_at = NOW(),
                updated_at = NOW()
            WHERE discord_id = $1
            "#,
			discord_id
		)
		.execute(&self.pool)
		.await?;

		Ok(())
	}
}
