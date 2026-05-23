use axum::{
    body::Body,
    extract::Path,
    http::{header, StatusCode},
    response::IntoResponse,
    response::Response,
};

/// Strip version from tauri-action asset names:
///   "openatrading_0.1.0_aarch64.dmg"       -> "openatrading_aarch64.dmg"
///   "openatrading_0.1.0_amd64.deb"          -> "openatrading_amd64.deb"
///   "openatrading_0.1.0_x64-setup.exe"      -> "openatrading_x64-setup.exe"
///   "openatrading_0.1.0_amd64.AppImage"     -> "openatrading_amd64.AppImage"
///   "openatrading-x86_64-unknown-linux-gnu"  -> unchanged (CLI binary, no version)
fn strip_version(name: &str) -> String {
    let parts: Vec<&str> = name.splitn(3, '_').collect();
    if parts.len() == 3
        && parts[1]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        format!("{}_{}", parts[0], parts[2])
    } else {
        name.to_string()
    }
}

/// Proxy install.sh from GitHub
pub async fn serve_install_sh() -> Result<impl IntoResponse, StatusCode> {
    let url = "https://raw.githubusercontent.com/oouxx/openatrading/main/install.sh";
    let client = reqwest::Client::new();
    let mut req = client.get(url);
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    let resp = req.send().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    let status = resp.status();
    let body = resp.text().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    Ok((
        status,
        [("content-type", "text/plain; charset=utf-8")],
        body,
    ))
}

/// Fetch latest release asset list, cached for 5 min
pub async fn fetch_assets() -> Result<Vec<(i64, String)>, ()> {
    tracing::debug!("fetch_assets: fetching latest release assets from GitHub");
    static CACHE: std::sync::Mutex<Option<(std::time::Instant, Vec<(i64, String)>)>> =
        std::sync::Mutex::new(None);

    {
        let cache = CACHE.lock().unwrap();
        if let Some((t, assets)) = cache.as_ref() {
            if t.elapsed() < std::time::Duration::from_secs(300) {
                return Ok(assets.clone());
            }
        }
    }

    let url = "https://api.github.com/repos/oouxx/openatrading/releases/latest";
    let client = reqwest::Client::new();
    let mut req = client.get(url).header("User-Agent", "openatrading-web");
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        req = req.header("Authorization", format!("Bearer {token}"));
    }
    let resp = req.send().await.map_err(|e| {
        tracing::warn!("fetch_assets: GitHub API request failed: {e}");
    })?;
    let json: serde_json::Value = resp.json().await.map_err(|e| {
        tracing::warn!("fetch_assets: failed to parse GitHub API response: {e}");
    })?;

    let mut assets = Vec::new();
    if let Some(arr) = json["assets"].as_array() {
        for a in arr {
            if let (Some(id), Some(name)) = (a["id"].as_i64(), a["name"].as_str()) {
                assets.push((id, name.to_string()));
            }
        }
    }

    tracing::debug!("fetch_assets: found {} assets", assets.len());
    *CACHE.lock().unwrap() = Some((std::time::Instant::now(), assets.clone()));
    Ok(assets)
}

/// Proxy download from GitHub Releases via asset API, fallback to local
///
/// Supports both exact match and version-stripped match:
///   request "openatrading_aarch64.dmg" matches "openatrading_0.1.0_aarch64.dmg"
///   request "openatrading_amd64.deb" matches "openatrading_0.1.0_amd64.deb"
///   request "openatrading_x64-setup.exe" matches "openatrading_0.1.0_x64-setup.exe"
pub async fn serve_download(Path(file): Path<String>) -> Result<impl IntoResponse, StatusCode> {
    if let Ok(assets) = fetch_assets().await {
        tracing::debug!(
            "serve_download: looking for {file} in {} assets",
            assets.len()
        );

        let matched = assets
            .iter()
            .find(|(_, name)| name == &file)
            .or_else(|| assets.iter().find(|(_, name)| strip_version(name) == file));

        if let Some((asset_id, matched_name)) = matched {
            tracing::debug!("serve_download: matched {file} -> {matched_name}");
            let url = format!(
                "https://api.github.com/repos/oouxx/openatrading/releases/assets/{asset_id}"
            );
            let client = reqwest::Client::new();
            let mut req = client
                .get(&url)
                .header("Accept", "application/octet-stream")
                .header("User-Agent", "openatrading-web");
            if let Ok(token) = std::env::var("GITHUB_TOKEN") {
                req = req.header("Authorization", format!("Bearer {token}"));
            }
            if let Ok(resp) = req.send().await {
                tracing::debug!(
                    "serve_download: GitHub asset API responded with {}",
                    resp.status()
                );
                if resp.status().is_success() {
                    let content_type = resp
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("application/octet-stream")
                        .to_string();
                    let body = resp.bytes().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
                    let response = Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, content_type)
                        .header(
                            header::CONTENT_DISPOSITION,
                            format!("attachment; filename=\"{matched_name}\""),
                        )
                        .body(Body::from(body))
                        .unwrap();
                    return Ok(response);
                }
            }
        }
    }

    tracing::debug!("serve_download: {file} not found in GitHub assets, trying local fallback");
    // Fallback to local downloads/
    let local_path = std::path::Path::new("downloads").join(&file);
    if local_path.exists() {
        let content_type = match local_path.extension().and_then(|e| e.to_str()) {
            Some("dmg") => "application/x-apple-diskimage",
            Some("deb") => "application/vnd.debian.binary-package",
            Some("AppImage") => "application/octet-stream",
            _ => "application/octet-stream",
        };
        let data = std::fs::read(&local_path).map_err(|_| StatusCode::NOT_FOUND)?;
        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(data))
            .unwrap();
        return Ok(response);
    }

    tracing::warn!("serve_download: {file} not found (neither GitHub asset nor local file)");
    Err(StatusCode::NOT_FOUND)
}
