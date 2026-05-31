pub mod auth;
pub mod sync;
pub mod ui;

pub fn config(cfg: &mut actix_web::web::ServiceConfig) {
	auth::config(cfg);
	cfg.service(ui::index);
	cfg.service(ui::dashboard);
	cfg.service(ui::connect_jellyfin);
	cfg.service(ui::get_jellyfin_status);
	cfg.service(ui::get_jellyfin_status_sse);
}
