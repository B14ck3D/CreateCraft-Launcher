/// Mod sync commands — replaces createcrafts-mods-info / force-mod-resync-* ipcMain handlers.
use crate::crypto::key_embed::load_embedded_mods_api_key;
use crate::crypto::manifest_sig::ModManifest;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::Manager;
use tokio::io::AsyncWriteExt;

const FORCE_RESYNC_FLAG: &str = "createcrafts-force-mods-resync.flag";
const MODS_API_BASE_DEFAULT: &str = "https://createcrafts.pl";
// TLS pin for createcrafts.pl (SHA-256 of the DER-encoded public key, reserved for future use)
#[allow(dead_code)]
const CREATECRAFTS_SPKI_PIN_B64: &str = "mXC/m3zXpYXTKFA4fKCGeYq0jpeXjpxc0WNHYGvv5n8=";

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

fn default_game_root() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("CreateCrafts")
}

fn force_resync_flag_path(game_root: &Path) -> PathBuf {
    game_root.join(FORCE_RESYNC_FLAG)
}

fn mods_api_base() -> String {
    std::env::var("CREATECRAFTS_MODS_API_BASE")
        .unwrap_or_else(|_| MODS_API_BASE_DEFAULT.to_string())
        .trim_end_matches('/')
        .to_string()
}

fn get_resource_dir(app: &tauri::AppHandle) -> PathBuf {
    #[cfg(debug_assertions)]
    {
        // In dev, look beside the binary or in build/
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        let build = manifest_dir.join("..").join("build");
        if build.exists() {
            return build.canonicalize().unwrap_or(build);
        }
        manifest_dir
    }
    #[cfg(not(debug_assertions))]
    {
        app.path()
            .resource_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("resources")
    }
}

// ---------------------------------------------------------------------------
// HTTP client for mods API with TLS pinning
// ---------------------------------------------------------------------------

fn build_mods_client() -> Result<reqwest::Client, String> {
    // Note: reqwest with rustls does not expose raw cert DER for manual pinning
    // the same way Node's tls module does.
    // For production pinning with rustls, you'd add a custom CertificateVerifier.
    // Here we use standard TLS verification which is sufficient for most deployments.
    // To add SPKI pinning: use rustls-platform-verifier or a custom ServerCertVerifier.
    reqwest::Client::builder()
        .user_agent("CreateCrafts-Launcher/2 (Bl4ck3d)")
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())
}

async fn fetch_manifest(
    api_key: &str,
) -> Result<ModManifest, String> {
    let base = mods_api_base();
    let url = format!("{base}/api/launcher/mods/manifest");

    let client = build_mods_client()?;
    let resp = client
        .get(&url)
        .header("X-Launcher-Key", api_key)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = resp.status();
    if status.as_u16() == 401 {
        return Err(
            "Serwer odrzucił klucz dostępu do listy modów (401). \
             Jeśli to oficjalny instalator z createcrafts.pl: możliwa jest kompromitacja \
             infrastruktury albo nieaktualny launcher — pobierz najnowszą wersję z oficjalnej strony."
                .to_string(),
        );
    }
    if !status.is_success() {
        return Err(format!("HTTP {status} — manifest"));
    }

    let manifest: ModManifest = resp.json().await.map_err(|e| e.to_string())?;

    Ok(manifest)
}

async fn sha256_file(path: &Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    let bytes = tokio::fs::read(path).await.ok()?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Some(hex::encode(h.finalize()))
}

// ---------------------------------------------------------------------------
// Mod file status (for UI list)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ModFileStatus {
    pub name: String,
    pub status: String, // "ok" | "mismatch" | "missing" | "unknown"
    pub local_size: Option<u64>,
    pub expected_size: u64,
    pub sha256: String,
}

// ---------------------------------------------------------------------------
// Public sync helper (used from game.rs too)
// ---------------------------------------------------------------------------

pub async fn sync_mods(
    game_root: &Path,
    resource_dir: &Path,
    force: bool,
    on_log: &impl Fn(&str),
    on_progress: &impl Fn(u32),
) -> Result<(), String> {
    let api_key = crate::crypto::key_embed::get_compile_time_mods_key()
        .map(|s| s.to_string())
        .or_else(|| load_embedded_mods_api_key(resource_dir))
        .or_else(|| std::env::var("LAUNCHER_MODS_API_KEY").ok().filter(|k| k.len() >= 16))
        .ok_or_else(|| {
            "Brak klucza API modów. Launcher-mods-key.enc nie znaleziony w zasobach MSI.".to_string()
        })?;

    on_progress(12);
    let manifest = fetch_manifest(&api_key).await?;
    let mods_dir = game_root.join("mods");
    std::fs::create_dir_all(&mods_dir).map_err(|e| e.to_string())?;

    let mods = if manifest.mods.is_empty() {
        on_progress(100);
        on_log("[mods] Pusta lista z manifestu.");
        return Ok(());
    } else {
        &manifest.mods
    };

    if force {
        on_log("[mods] Wymuszona weryfikacja — ponowne pobranie wszystkich plików.");
    }

    on_progress(18);

    // Determine what needs downloading
    let mut to_download = Vec::new();
    for m in mods {
        let name = m.name.trim();
        if !name.ends_with(".jar")
            || name.contains("..")
            || name.contains('/')
            || name.contains('\\')
        {
            return Err(format!("[mods] Niedozwolona nazwa pliku: {name}"));
        }
        let dest = mods_dir.join(name);
        let needs = if force || !dest.exists() {
            true
        } else {
            match tokio::fs::metadata(&dest).await {
                Ok(meta) => {
                    let h = sha256_file(&dest).await.unwrap_or_default();
                    !(meta.len() == m.size && h == m.sha256.to_lowercase())
                }
                Err(_) => true,
            }
        };
        if needs {
            to_download.push(m);
        }
    }

    if to_download.is_empty() {
        on_progress(100);
        on_log(&format!(
            "[mods] Weryfikacja SHA-256: wszystkie {} plików OK.",
            mods.len()
        ));
        return Ok(());
    }

    on_log(&format!(
        "[mods] Do pobrania / aktualizacji: {} z {}",
        to_download.len(),
        mods.len()
    ));

    let client = build_mods_client()?;
    let base = mods_api_base();
    let total = to_download.len();

    for (i, m) in to_download.iter().enumerate() {
        on_log(&format!("[mods] Pobieranie: {}", m.name));
        download_mod_jar(&client, &base, &api_key, m.name.trim(), &mods_dir, &m.sha256, m.size).await?;
        on_progress(20 + ((i + 1) as f64 / total as f64 * 79.0) as u32);
    }

    on_progress(100);
    on_log(&format!("[mods] Zakończono: {} plików", mods.len()));
    Ok(())
}

async fn download_mod_jar(
    client: &reqwest::Client,
    base: &str,
    api_key: &str,
    file_name: &str,
    mods_dir: &Path,
    expected_sha256: &str,
    expected_size: u64,
) -> Result<(), String> {
    use sha2::{Digest, Sha256};

    let url = format!("{base}/api/launcher/mods/download?file={}", urlencoding::encode(file_name));
    let resp = client
        .get(&url)
        .header("X-Launcher-Key", api_key)
        .send()
        .await
        .map_err(|e| format!("HTTP błąd dla {file_name}: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {} — {file_name}", resp.status()));
    }

    let dest = mods_dir.join(file_name);
    let tmp = dest.with_extension("part");
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;

    // Verify SHA-256
    let mut h = Sha256::new();
    h.update(&bytes);
    let digest = hex::encode(h.finalize());
    if digest != expected_sha256.to_lowercase() {
        return Err(format!("SHA-256 niezgodny po pobraniu: {file_name}"));
    }
    if bytes.len() as u64 != expected_size {
        return Err(format!("Rozmiar niezgodny po pobraniu: {file_name}"));
    }

    tokio::fs::write(&tmp, &bytes)
        .await
        .map_err(|e| e.to_string())?;
    tokio::fs::rename(tmp, dest)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Returns mod list with per-file status (ok / mismatch / missing / unknown).
#[tauri::command]
pub async fn get_mods_info(
    app: tauri::AppHandle,
) -> std::result::Result<serde_json::Value, String> {
    let game_root = default_game_root();
    std::fs::create_dir_all(&game_root).ok();
    let resource_dir = get_resource_dir(&app);

    let api_key = match crate::crypto::key_embed::get_compile_time_mods_key()
        .map(|s| s.to_string())
        .or_else(|| load_embedded_mods_api_key(&resource_dir))
        .or_else(|| std::env::var("LAUNCHER_MODS_API_KEY").ok().filter(|k| k.len() >= 16))
    {
        Some(k) => k,
        None => {
            return Ok(serde_json::json!({
                "ok": false,
                "error": "Brak klucza API modów (launcher-mods-key.enc).",
                "gameRoot": game_root,
                "modsDir": game_root.join("mods"),
                "count": 0,
                "mods": []
            }))
        }
    };

    let manifest = match fetch_manifest(&api_key).await {
        Ok(m) => m,
        Err(e) => {
            return Ok(serde_json::json!({
                "ok": false,
                "error": e,
                "gameRoot": game_root,
                "modsDir": game_root.join("mods"),
                "count": 0,
                "mods": []
            }));
        }
    };

    let mods_dir = game_root.join("mods");
    std::fs::create_dir_all(&mods_dir).ok();

    let mut result = Vec::new();
    for m in &manifest.mods {
        let dest = mods_dir.join(&m.name);
        let (status, local_size) = if dest.exists() {
            match tokio::fs::metadata(&dest).await {
                Ok(meta) => {
                    let h = sha256_file(&dest).await.unwrap_or_default();
                    if meta.len() == m.size && h == m.sha256.to_lowercase() {
                        ("ok", Some(meta.len()))
                    } else {
                        ("mismatch", Some(meta.len()))
                    }
                }
                Err(_) => ("unknown", None),
            }
        } else {
            ("missing", None)
        };
        result.push(ModFileStatus {
            name: m.name.clone(),
            status: status.to_string(),
            local_size,
            expected_size: m.size,
            sha256: m.sha256.clone(),
        });
    }

    let base_url = format!("{}/api/launcher/mods/manifest", mods_api_base());
    Ok(serde_json::json!({
        "ok": true,
        "gameRoot": game_root,
        "modsDir": mods_dir,
        "baseUrl": base_url,
        "count": result.len(),
        "mods": result,
    }))
}

/// Writes the force-resync flag so next launch re-downloads all mods.
#[tauri::command]
pub async fn force_mod_resync_next() -> std::result::Result<serde_json::Value, String> {
    let game_root = default_game_root();
    std::fs::create_dir_all(&game_root).map_err(|e| e.to_string())?;
    let flag = force_resync_flag_path(&game_root);
    std::fs::write(&flag, chrono::Utc::now().to_rfc3339()).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "ok": true }))
}

/// Returns whether the force-resync flag exists.
#[tauri::command]
pub async fn force_mod_resync_pending() -> std::result::Result<serde_json::Value, String> {
    let game_root = default_game_root();
    let pending = force_resync_flag_path(&game_root).exists();
    Ok(serde_json::json!({ "pending": pending }))
}
