use std::{net::SocketAddr, path::PathBuf};

#[derive(Clone)]
pub struct ServerConfig {
    pub data_dir: PathBuf,
    pub listen: SocketAddr,
    pub public_origin: String,
}
impl ServerConfig {
    pub fn validate(&self) -> Result<bool, String> {
        let uri: axum::http::Uri = self
            .public_origin
            .parse()
            .map_err(|_| "invalid public origin")?;
        if uri.authority().is_none()
            || uri.authority().is_some_and(|a| a.as_str().contains('@'))
            || uri.path() != "/"
            || uri.query().is_some()
            || self.public_origin.ends_with('/')
        {
            return Err(
                "public origin must contain only scheme and host, without a trailing slash".into(),
            );
        }
        match uri.scheme_str() {
            Some("https") => Ok(true),
            Some("http")
                if self.listen.ip().is_loopback()
                    && matches!(uri.host(), Some("127.0.0.1" | "localhost" | "[::1]")) =>
            {
                Ok(false)
            }
            _ => Err("HTTPS is required except for a loopback-only development server".into()),
        }
    }
}
