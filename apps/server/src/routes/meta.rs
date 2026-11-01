use crate::error::RequestContext;
use axum::{Extension, response::Response};
use memedock_api_contracts::VersionInfo;
use memedock_domain::version::ProtocolVersion;

/// Public identity of the running build. It carries no account or library
/// data, so the login page and the console can read it before authentication.
pub async fn version(Extension(ctx): Extension<RequestContext>) -> Response {
    ctx.respond(
        async {
            Ok(VersionInfo {
                version: env!("CARGO_PKG_VERSION").to_owned(),
                protocol_version: ProtocolVersion::CURRENT.get(),
                debug: cfg!(debug_assertions),
            })
        }
        .await,
    )
}
