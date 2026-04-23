use futures_util::StreamExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};
#[cfg(target_os = "windows")]
use std::path::Path;
#[cfg(target_os = "windows")]
use std::process::Command;
#[cfg(not(target_os = "windows"))]
use std::path::PathBuf;
#[cfg(not(target_os = "windows"))]
use tauri_plugin_opener::OpenerExt;

const PANEL_API_BASE_DEFAULT: &str = "https://createcrafts.pl";

fn panel_api_base() -> String {
    std::env::var("CREATECRAFTS_MODS_API_BASE")
        .unwrap_or_else(|_| PANEL_API_BASE_DEFAULT.to_string())
        .trim_end_matches('/')
        .to_string()
}

fn build_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("CreateCrafts-Launcher/2 (update)")
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
struct LauncherMetaResponse {
    version: String,
    sha256: String,
    url: String,
    #[serde(default)]
    notes: String,
}

#[cfg(target_os = "windows")]
fn chunk_contains_nsis_marker(chunk: &[u8]) -> bool {
    let lower: Vec<u8> = chunk.iter().map(|b| b.to_ascii_lowercase()).collect();
    lower.windows(9).any(|w| w == b"nullsoft")
        || lower.windows(10).any(|w| w == b"nsis error")
        || lower.windows(12).any(|w| w == b"nsis.install")
        || lower.windows(4).any(|w| w == b"nsis")
}

#[cfg(target_os = "windows")]
fn exe_looks_like_nsis_installer(path: &Path) -> bool {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};
    const WINDOW: usize = 6 * 1024 * 1024;
    const STEP: u64 = 4 * 1024 * 1024;
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let len = meta.len();
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let mut off = 0u64;
    let scan_cap = len.min(64 * 1024 * 1024);
    while off < scan_cap {
        let _ = file.seek(SeekFrom::Start(off));
        let mut buf = vec![0u8; WINDOW];
        let n = match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        if chunk_contains_nsis_marker(&buf[..n]) {
            return true;
        }
        off = off.saturating_add(STEP);
    }
    false
}

fn resolve_download_url(base: &str, url: &str) -> String {
    let u = url.trim();
    if u.starts_with("http://") || u.starts_with("https://") {
        u.to_string()
    } else if u.starts_with('/') {
        format!("{base}{u}")
    } else {
        format!("{base}/{u}")
    }
}

#[tauri::command]
pub fn get_app_version(app: tauri::AppHandle) -> Result<String, String> {
    Ok(app.package_info().version.to_string())
}

#[tauri::command]
pub async fn check_launcher_update(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let current = app.package_info().version.to_string();
    let base = panel_api_base();
    let meta_url = format!("{base}/api/public/launcher/meta");
    let client = build_http_client()?;
    let resp = client
        .get(&meta_url)
        .send()
        .await
        .map_err(|e| format!("Meta request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Ok(serde_json::json!({
            "ok": false,
            "currentVersion": current,
            "updateAvailable": false,
            "error": format!("HTTP {} - {}", status, body.chars().take(240).collect::<String>()),
        }));
    }

    let meta: LauncherMetaResponse = resp
        .json()
        .await
        .map_err(|e| format!("Invalid meta JSON: {e}"))?;

    let cur_v = semver::Version::parse(&current).ok();
    let rem_v = semver::Version::parse(meta.version.trim()).ok();
    let update_available = match (cur_v, rem_v) {
        (Some(c), Some(r)) => r > c,
        _ => false,
    };

    let download_url = resolve_download_url(&base, &meta.url);

    Ok(serde_json::json!({
        "ok": true,
        "currentVersion": current,
        "remoteVersion": meta.version,
        "updateAvailable": update_available,
        "downloadUrl": download_url,
        "expectedSha256": meta.sha256.to_lowercase(),
        "notes": meta.notes,
        "error": serde_json::Value::Null,
    }))
}

#[tauri::command]
pub async fn download_and_install_launcher_update(
    #[cfg_attr(target_os = "windows", allow(unused_variables))]
    app: tauri::AppHandle,
    download_url: String,
    expected_sha256_hex: String,
) -> Result<serde_json::Value, String> {
    let url = download_url.trim().to_string();
    if !url.starts_with("https://") {
        return Err("Only HTTPS download URLs are allowed.".to_string());
    }

    let want = expected_sha256_hex.trim().to_lowercase();
    if !want.chars().all(|c| c.is_ascii_hexdigit()) || want.len() != 64 {
        return Err("Expected SHA-256 must be 64 hex characters.".to_string());
    }

    let ext = if url.to_lowercase().contains(".exe") {
        ".exe"
    } else {
        ".bin"
    };
    let out_path = std::env::temp_dir().join(format!("CreateCrafts-launcher-update{ext}"));

    let client = build_http_client()?;
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let mut file = tokio::fs::File::create(&out_path)
        .await
        .map_err(|e| format!("Temp file: {e}"))?;
    let mut hasher = Sha256::new();
    let mut stream = resp.bytes_stream();

    use tokio::io::AsyncWriteExt;
    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| format!("Stream: {e}"))?;
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Write: {e}"))?;
    }
    file.flush().await.map_err(|e| e.to_string())?;
    drop(file);

    let got = hex::encode(hasher.finalize());
    if got != want {
        let _ = tokio::fs::remove_file(&out_path).await;
        return Err(format!(
            "SHA-256 mismatch (expected {want}, got {got})."
        ));
    }

    #[cfg(target_os = "windows")]
    {
        if ext != ".exe" {
            return Err("Windows: wymagany plik .exe (instalator).".to_string());
        }
        let path = out_path.as_path();
        if exe_looks_like_nsis_installer(path) {
            Command::new(path)
                .arg("/S")
                .spawn()
                .map_err(|e| format!("Nie udalo sie uruchomic instalatora: {e}"))?;
            std::thread::sleep(std::time::Duration::from_millis(600));
            std::process::exit(0);
        }

        let display = path.display().to_string();
        let select_arg = format!("/select,{}", display);
        let _ = Command::new("explorer.exe").arg(select_arg).spawn();

        return Ok(serde_json::json!({
            "ok": true,
            "ranInstaller": false,
            "manual": true,
            "savedPath": display,
            "message": "Pobrano plik aktualizacji, ale nie wyglada jak instalator NSIS (brak znacznika Nullsoft). Otworzono folder - uruchom plik recznie albo pobierz oficjalny instalator NSIS z createcrafts.pl (build Windows)."
        }));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let p = out_path.to_string_lossy().to_string();
        app.opener()
            .reveal_item_in_dir(PathBuf::from(&p))
            .map_err(|e| format!("Reveal in folder failed: {e}"))?;
        Ok(serde_json::json!({
            "ok": true,
            "ranInstaller": false,
            "savedPath": p,
            "message": "Update downloaded. Install manually; file location was opened in the file manager."
        }))
    }
}
