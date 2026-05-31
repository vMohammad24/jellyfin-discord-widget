use crate::db::Db;
use crate::discord::api::DiscordClient;
use crate::jellyfin::manager::JellyfinManager;
use crate::util::config::Config;
use std::sync::Arc;
use tokio::time::{Duration, interval};
use tracing::{debug, error, info};

pub mod error;
pub mod profile;
pub mod user;

pub async fn start_sync_engine(
	db: Db,
	jellyfin: Arc<JellyfinManager>,
	discord: Arc<DiscordClient>,
	config: Arc<Config>,
	mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) {
	info!("Starting background sync engine");
	let mut ticker = interval(Duration::from_secs(60));

	loop {
		tokio::select! {
			_ = ticker.tick() => {
				info!("Sync engine ticking - scanning for users to update");
				jellyfin.cleanup_orphaned_senders();

				let mut last_id: Option<i64> = None;
				let limit = 20;
				let mut join_set = tokio::task::JoinSet::new();

				'user_loop: loop {
					if *shutdown_rx.borrow() {
						info!("Shutdown signal received, aborting user processing loop");
						join_set.abort_all();
						break;
					}

					let users = match db.get_users_paginated(limit, last_id).await {
						Ok(u) => {
							debug!(
								count = u.len(),
								after_id = ?last_id,
								"Successfully fetched users from database"
							);
							u
						}
						Err(e) => {
							error!(error = %e, "Failed to fetch users");
							break;
						}
					};

					if users.is_empty() {
						break;
					}

					let users_len = users.len();
					if let Some(last_user) = users.last() {
						last_id = last_user.discord_id.parse::<i64>().ok();
					}

					for user in users {
						while join_set.len() >= 5 {
							tokio::select! {
								_ = join_set.join_next() => {}
								_ = shutdown_rx.changed() => {
									if *shutdown_rx.borrow() {
										info!("Shutdown signal received during task join, aborting");
										join_set.abort_all();
										break 'user_loop;
									}
								}
							}
						}

						let db_clone = db.clone();
						let jellyfin_clone = jellyfin.clone();
						let discord_clone = discord.clone();
						let config_clone = config.clone();

						join_set.spawn(async move {
							debug!(
								user_id = %user.discord_id,
								last_active = %user.last_active_at,
								"Syncing user"
							);
							if let Err(e) = user::sync_single_user(
								&db_clone,
								&user,
								&jellyfin_clone,
								&discord_clone,
								&config_clone,
							).await {
								error!(user_id = %user.discord_id, error = %e, "Error syncing user");
							}
						});
					}

					if (users_len as i64) < limit {
						break;
					}
				}

				while !join_set.is_empty() {
					tokio::select! {
						_ = join_set.join_next() => {}
						_ = shutdown_rx.changed() => {
							if *shutdown_rx.borrow() {
								info!("Shutdown signal received during final task join, aborting");
								join_set.abort_all();
								break;
							}
						}
					}
				}
			}
			_ = shutdown_rx.changed() => {
				if *shutdown_rx.borrow() {
					info!("Shutdown signal received in idle loop, exiting sync engine");
					break;
				}
			}
		}
	}
}
