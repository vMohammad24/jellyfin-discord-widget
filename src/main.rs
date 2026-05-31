use actix_session::{SessionMiddleware, storage::CookieSessionStore};
use actix_web::cookie::Key;
use actix_web::{App, HttpServer, web};
use dotenvy::dotenv;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

mod api;
mod db;
mod discord;
mod jellyfin;
mod util;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	dotenv().ok();

	let config = match util::config::Config::load() {
		Ok(cfg) => cfg,
		Err(e) => {
			eprintln!("Configuration error: {}", e);
			std::process::exit(1);
		}
	};

	if let Err(e) = util::crypto::init_cipher(&config.encryption_key_bytes) {
		eprintln!("Failed to initialize cipher: {}", e);
		std::process::exit(1);
	}

	let subscriber = FmtSubscriber::builder()
		.with_env_filter(EnvFilter::new(&config.log_level))
		.finish();
	let _ = tracing::subscriber::set_global_default(subscriber);

	info!("Starting Jellyfin Discord Widget server");

	info!("Connecting to database...");
	let db = match db::Db::new(&config.database_url).await {
		Ok(d) => d,
		Err(e) => {
			tracing::error!("Failed to connect to database: {}", e);
			std::process::exit(1);
		}
	};

	let secret_key = Key::from(&config.session_secret);
	let config_arc = Arc::new(config);

	let client = match reqwest::Client::builder()
		.timeout(std::time::Duration::from_secs(10))
		.connect_timeout(std::time::Duration::from_secs(5))
		.pool_max_idle_per_host(10)
		.dns_resolver(Arc::new(util::ssrf::SafeResolver))
		.build()
	{
		Ok(c) => c,
		Err(e) => {
			tracing::error!("Failed to build global HTTP client: {}", e);
			std::process::exit(1);
		}
	};

	let jellyfin = jellyfin::manager::JellyfinManager::new(client.clone());
	let discord = discord::api::DiscordClient::new(client);

	let db_data = web::Data::new(db.clone());
	let config_arc_data = web::Data::new(config_arc.clone());
	let jellyfin_arc = Arc::new(jellyfin);
	let discord_arc = Arc::new(discord);

	let jellyfin_data = web::Data::new(jellyfin_arc.clone());
	let discord_data = web::Data::new(discord_arc.clone());

	let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
	let shutdown_rx_data = web::Data::new(shutdown_rx.clone());

	let sync_handle = tokio::spawn(api::sync::start_sync_engine(
		db.clone(),
		jellyfin_arc.clone(),
		discord_arc.clone(),
		config_arc.clone(),
		shutdown_rx,
	));

	let bind_addr = format!("{}:{}", config_arc.host, config_arc.port);
	info!("Starting server on {}", bind_addr);

	let server = HttpServer::new(move || {
		App::new()
			.app_data(db_data.clone())
			.app_data(config_arc_data.clone())
			.app_data(jellyfin_data.clone())
			.app_data(discord_data.clone())
			.app_data(shutdown_rx_data.clone())
			.wrap(
				SessionMiddleware::builder(CookieSessionStore::default(), secret_key.clone())
					.cookie_secure(false)
					.build(),
			)
			.configure(api::config)
	})
	.bind(&bind_addr)?
	.disable_signals()
	.run();

	let server_handle = server.handle();
	let server_task = tokio::spawn(server);

	tokio::select! {
		_ = tokio::signal::ctrl_c() => {
			info!("Ctrl+C received, shutting down gracefully...");
		}
	}

	info!("Signaling background sync engine and SSE streams to shut down...");
	let _ = shutdown_tx.send(true);

	server_handle.stop(true).await;

	let _ = server_task.await;
	let _ = sync_handle.await;
	info!("Jellyfin Discord Widget shut down cleanly.");

	Ok(())
}
