use crate::error::{LauncherError, Result};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const NEOFORGE_VERSION_DEFAULT: &str = "21.1.226";
pub const MC_VERSION: &str = "1.21.1";

fn neoforge_installer_url(version: &str) -> String {
    format!(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar"
    )
}

fn cache_dir(game_root: &Path) -> PathBuf {
    game_root.join("launcher-cache")
}

pub fn neoforge_installer_jar_path(game_root: &Path, version: &str) -> PathBuf {
    cache_dir(game_root).join(format!("neoforge-{version}-installer.jar"))
}

fn neoforge_ready_marker_path(game_root: &Path) -> PathBuf {
    cache_dir(game_root).join("neoforge-ready.version")
}

pub fn read_neoforge_ready_version(game_root: &Path) -> Option<String> {
    std::fs::read_to_string(neoforge_ready_marker_path(game_root))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn write_neoforge_ready_version(game_root: &Path, version: &str) {
    let _ = std::fs::create_dir_all(cache_dir(game_root));
    let _ = std::fs::write(neoforge_ready_marker_path(game_root), version);
}

pub fn clear_mclc_forge_cache(game_root: &Path, on_log: &impl Fn(&str)) {
    let p = game_root.join("forge").join(MC_VERSION);
    match std::fs::remove_dir_all(&p) {
        Ok(_) => on_log(&format!("[neoforge] Wyczyszczono cache forge: {}", p.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => on_log(&format!("[neoforge] Czyszczenie cache (opcjonalne): {e}")),
    }
}

pub fn resolve_neoforge_version() -> String {
    std::env::var("CREATECRAFT_NEOFORGE_VERSION")
        .or_else(|_| std::env::var("SUPERSMP_NEOFORGE_VERSION"))
        .unwrap_or_else(|_| NEOFORGE_VERSION_DEFAULT.to_string())
        .trim()
        .to_string()
}

pub fn neoforge_version_id(version: &str) -> String {
    format!("{MC_VERSION}-neoforge-{version}")
}

pub fn neoforge_version_json_path(game_root: &Path, version: &str) -> PathBuf {
    let vid = neoforge_version_id(version);
    game_root
        .join("versions")
        .join(&vid)
        .join(format!("{vid}.json"))
}

fn neoforge_client_jar_path(game_root: &Path, version: &str) -> PathBuf {
    game_root
        .join("libraries")
        .join("net")
        .join("neoforged")
        .join("neoforge")
        .join(version)
        .join(format!("neoforge-{version}-client.jar"))
}

pub fn is_neoforge_installed(game_root: &Path, version: &str) -> bool {
    neoforge_version_json_path(game_root, version).exists()
        && neoforge_client_jar_path(game_root, version).exists()
}

// Installer JAR download

pub async fn ensure_neoforge_installer(
    game_root: &Path,
    version: &str,
    on_log: &impl Fn(&str),
) -> Result<PathBuf> {
    let dest = neoforge_installer_jar_path(game_root, version);
    std::fs::create_dir_all(cache_dir(game_root))?;

    if dest.exists() {
        if let Ok(meta) = std::fs::metadata(&dest) {
            if meta.len() > 100_000 {
                on_log(&format!("[neoforge] Instalator w cache: {}", dest.display()));
                prune_old_installers(game_root, version);
                return Ok(dest);
            }
        }
    }

    let url = neoforge_installer_url(version);
    on_log(&format!("[neoforge] Pobieranie instalatora {version}…"));

    let client = reqwest::Client::builder()
        .user_agent("CreateCrafts-Launcher/2 (Bl4ck3d)")
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(LauncherError::Http)?;

    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        return Err(LauncherError::Minecraft(format!(
            "Pobieranie instalatora NeoForge: HTTP {}",
            resp.status()
        )));
    }

    let bytes = resp.bytes().await?;
    let tmp = dest.with_extension("part");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(tmp, &dest)?;

    on_log(&format!("[neoforge] Pobrano: {}", dest.display()));
    prune_old_installers(game_root, version);
    Ok(dest)
}

fn prune_old_installers(game_root: &Path, keep_version: &str) {
    let keep_name = format!("neoforge-{keep_version}-installer.jar");
    let dir = cache_dir(game_root);
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name != keep_name
                && name.starts_with("neoforge-")
                && name.ends_with("-installer.jar")
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

fn extract_neoforge_version_json(
    installer_jar: &Path,
    game_root: &Path,
    neoforge_version: &str,
    on_log: &impl Fn(&str),
) -> Result<()> {
    let dest = neoforge_version_json_path(game_root, neoforge_version);
    if dest.exists() {
        return Ok(());
    }

    on_log("[neoforge] Wypakowuję version.json z instalatora…");

    let file = std::fs::File::open(installer_jar)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| LauncherError::Archive(format!("ZIP: {e}")))?;

    let mut json_str = String::new();
    {
        let mut entry = zip.by_name("version.json").map_err(|_| {
            LauncherError::Minecraft(
                "version.json nie znaleziony w instalatorze NeoForge".to_string(),
            )
        })?;
        entry.read_to_string(&mut json_str)?;
    }

    let vid = neoforge_version_id(neoforge_version);
    let json_patched =
        if json_str.contains(&format!("\"id\":\"{vid}\""))
            || json_str.contains(&format!("\"id\": \"{vid}\""))
        {
            json_str
        } else {
            let re_id = regex::Regex::new(r#""id"\s*:\s*"[^"]+""#)
                .map_err(|e| LauncherError::Minecraft(format!("regex: {e}")))?;
            re_id
                .replace(&json_str, format!("\"id\":\"{vid}\""))
                .to_string()
        };

    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(&dest, json_patched.as_bytes())?;
    on_log(&format!("[neoforge] version.json zapisany: {}", dest.display()));
    Ok(())
}

fn ensure_launcher_profiles(game_root: &Path) {
    let profiles_path = game_root.join("launcher_profiles.json");
    if !profiles_path.exists() {
        let _ = std::fs::create_dir_all(game_root);
        let _ = std::fs::write(
            &profiles_path,
            r#"{"profiles":{},"selectedProfile":"(default)","clientToken":"CreateCrafts","launcherVersion":{"format":21,"name":"1.0","release":"1.0"}}"#,
        );
    }
}

async fn run_neoforge_installer(
    java_path: &Path,
    installer: &Path,
    game_root: &Path,
    version: &str,
    on_log: &impl Fn(&str),
) -> Result<()> {
    if neoforge_client_jar_path(game_root, version).exists() {
        on_log("[neoforge] Przetworzone pliki już istnieją, pomijam uruchamianie instalatora.");
        return Ok(());
    }

    // NeoForge installer requires launcher_profiles.json to exist in the game root
    ensure_launcher_profiles(game_root);

    on_log("[neoforge] Uruchamiam instalator NeoForge (pierwsze uruchomienie, może potrwać kilka minut)…");

    let mut cmd = tokio::process::Command::new(java_path);
    cmd.args([
        "-jar",
        &installer.to_string_lossy().into_owned(),
        "--installClient",
        &game_root.to_string_lossy().into_owned(),
    ])
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped());

    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    let output = cmd
        .output()
        .await
        .map_err(|e| LauncherError::Java(format!("Uruchamianie instalatora NeoForge: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Log relevant output lines
    for line in stdout.lines().chain(stderr.lines()) {
        if !line.trim().is_empty() {
            on_log(&format!("[neoforge-installer] {line}"));
        }
    }

    if !output.status.success() {
        return Err(LauncherError::Minecraft(format!(
            "Instalator NeoForge zakończył się błędem (kod {:?}).\n{}",
            output.status.code(),
            stderr.lines().rev().take(10).collect::<Vec<_>>().join("\n")
        )));
    }

    if !neoforge_client_jar_path(game_root, version).exists() {
        return Err(LauncherError::Minecraft(format!(
            "Instalator NeoForge zakończył się poprawnie, ale \
             neoforge-{version}-client.jar nie został stworzony. \
             Sprawdź logi instalatora."
        )));
    }

    on_log(&format!("[neoforge] Instalacja zakończona. neoforge-{version}-client.jar gotowy."));
    Ok(())
}

// High-level entry point

pub async fn ensure_neoforge(
    java_path: &Path,
    game_root: &Path,
    on_log: &impl Fn(&str),
    on_progress: &impl Fn(u32),
) -> Result<String> {
    let version = resolve_neoforge_version();
    on_log(&format!("[neoforge] Wersja: {version}"));

    let prev = read_neoforge_ready_version(game_root);

    // Full fast-path: already installed and ready
    if prev.as_deref() == Some(&version) && is_neoforge_installed(game_root, &version) {
        on_log(&format!(
            "[neoforge] NeoForge {version} już zainstalowany, pomijam."
        ));
        on_progress(100);
        return Ok(version);
    }

    if prev.as_deref() != Some(&version) {
        clear_mclc_forge_cache(game_root, on_log);
        on_log(&format!("[neoforge] Nowa wersja ({version}), czyszczenie cache."));
    }

    on_progress(5);
    let installer = ensure_neoforge_installer(game_root, &version, on_log).await?;
    on_progress(20);

    // Extract version.json so the version pipeline can proceed while installer runs
    extract_neoforge_version_json(&installer, game_root, &version, on_log)?;
    on_progress(30);

    // Run the full installer (processor pipeline: remap + patch)
    run_neoforge_installer(java_path, &installer, game_root, &version, on_log).await?;
    on_progress(95);

    write_neoforge_ready_version(game_root, &version);
    on_progress(100);
    Ok(version)
}
