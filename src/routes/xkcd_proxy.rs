use axum::{extract::Query, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct XkcdProxyParams {
    /// The XKCD comic number. If omitted, fetches the latest comic.
    pub num: Option<u32>,
}

/// Proxies requests to xkcd.com/info.0.json to avoid CORS issues on the frontend.
/// Only allows fetching XKCD comic JSON — not an open proxy.
pub async fn xkcd_proxy(
    Query(params): Query<XkcdProxyParams>,
) -> Result<impl IntoResponse, StatusCode> {
    let url = match params.num {
        Some(num) => format!("https://xkcd.com/{num}/info.0.json"),
        None => "https://xkcd.com/info.0.json".to_string(),
    };

    let response = reqwest::get(&url)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    if !response.status().is_success() {
        return Err(StatusCode::BAD_GATEWAY);
    }

    let json: serde_json::Value = response.json().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    Ok(Json(json))
}
