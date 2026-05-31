use reqwest::dns::{Name, Resolve};
use std::error::Error as StdError;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use url::{Host, Url};

pub struct SafeResolver;

pub fn is_ip_safe(ip: IpAddr) -> bool {
	match ip {
		IpAddr::V4(v4) => {
			!(v4.is_loopback()
				|| v4.is_private()
				|| v4.is_link_local()
				|| v4.is_broadcast()
				|| v4.is_documentation()
				|| v4.is_unspecified())
		}
		IpAddr::V6(v6) => {
			let segments = v6.segments();
			let is_unique_local = (segments[0] & 0xfe00) == 0xfc00;
			let is_ipv4_mapped = segments[0..5] == [0, 0, 0, 0, 0] && segments[5] == 0xffff;

			!(v6.is_loopback()
				|| v6.is_unspecified()
				|| (segments[0] & 0xff00) == 0xfe00
				|| is_unique_local
				|| is_ipv4_mapped)
		}
	}
}

pub fn validate_url(url_str: &str) -> Result<Url, String> {
	let url = Url::parse(url_str).map_err(|e| format!("Invalid URL: {}", e))?;

	if url.scheme() != "http" && url.scheme() != "https" {
		return Err("Only http and https schemes are allowed".to_string());
	}

	match url.host() {
		Some(Host::Ipv4(v4)) => {
			if !is_ip_safe(IpAddr::V4(v4)) {
				return Err(
					"Direct access to private/local IPv4 addresses is forbidden".to_string()
				);
			}
		}
		Some(Host::Ipv6(v6)) => {
			if !is_ip_safe(IpAddr::V6(v6)) {
				return Err(
					"Direct access to private/local IPv6 addresses is forbidden".to_string()
				);
			}
		}
		Some(Host::Domain(_)) => {}
		None => return Err("URL missing host".to_string()),
	}

	Ok(url)
}

impl Resolve for SafeResolver {
	fn resolve(
		&self,
		name: Name,
	) -> Pin<
		Box<
			dyn Future<
					Output = Result<
						Box<dyn Iterator<Item = SocketAddr> + Send>,
						Box<dyn StdError + Send + Sync>,
					>,
				> + Send,
		>,
	> {
		let fut = async move {
			let addrs = tokio::net::lookup_host(format!("{}:0", name.as_str()))
				.await
				.map_err(|e| Box::new(e) as Box<dyn StdError + Send + Sync>)?;

			let filtered: Vec<SocketAddr> = addrs.filter(|addr| is_ip_safe(addr.ip())).collect();

			let iter: Box<dyn Iterator<Item = SocketAddr> + Send> = Box::new(filtered.into_iter());
			Ok(iter)
		};
		Box::pin(fut)
	}
}
