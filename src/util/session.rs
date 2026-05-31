use actix_session::SessionExt;
use actix_web::{Error, FromRequest, HttpRequest, dev::Payload, error::ErrorUnauthorized};
use futures_util::future::LocalBoxFuture;

pub struct AuthenticatedUser {
	pub discord_id: String,
}

impl FromRequest for AuthenticatedUser {
	type Error = Error;
	type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

	fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
		let session = req.get_session();

		Box::pin(async move {
			let discord_id = match session.get::<String>("discord_id") {
				Ok(Some(discord_id)) => discord_id,
				_ => return Err(ErrorUnauthorized("Unauthorized")),
			};

			Ok(AuthenticatedUser { discord_id })
		})
	}
}
