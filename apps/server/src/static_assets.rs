#[cfg(feature = "web")]
use axum::{body::Body, http::header};
use axum::{
    http::{Method, StatusCode, Uri},
    response::{IntoResponse, Response},
};

#[cfg(feature = "web")]
#[derive(rust_embed::Embed)]
#[folder = "../../apps/server-web/dist/"]
struct WebAssets;

pub async fn serve(method: Method, uri: Uri) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    if uri.path().starts_with("/api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    if uri.path().ends_with('/') && uri.path() != "/" {
        let path = uri.path().trim_end_matches('/');
        if app_route(path.trim_start_matches('/')) {
            let destination = uri
                .query()
                .map_or_else(|| path.to_owned(), |q| format!("{path}?{q}"));
            return axum::response::Redirect::permanent(&destination).into_response();
        }
    }
    embedded(&method, uri.path())
}

fn app_route(path: &str) -> bool {
    matches!(
        path,
        "" | "login" | "devices" | "settings" | "library" | "sync" | "activity"
    )
}

#[cfg(feature = "web")]
fn embedded(method: &Method, path: &str) -> Response {
    let path = path.trim_start_matches('/');
    let asset = WebAssets::get(path).or_else(|| {
        app_route(path)
            .then(|| WebAssets::get("index.html"))
            .flatten()
    });
    let Some(asset) = asset else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = match path.rsplit('.').next() {
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("woff2") => "font/woff2",
        Some("ico") => "image/x-icon",
        Some("json") => "application/json",
        _ => "text/html; charset=utf-8",
    };
    let length = asset.data.len();
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from(asset.data.into_owned())
    };
    let mut response = body.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static(content_type),
    );
    if let Ok(value) = header::HeaderValue::from_str(&length.to_string()) {
        response.headers_mut().insert(header::CONTENT_LENGTH, value);
    }
    response
}

#[cfg(not(feature = "web"))]
fn embedded(_: &Method, _: &str) -> Response {
    StatusCode::NOT_FOUND.into_response()
}
