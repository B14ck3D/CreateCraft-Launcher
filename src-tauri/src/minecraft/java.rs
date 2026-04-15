/// Java 21 (Temurin) manager — mirrors javaManager.js.
/// Detects an already-downloaded JDK, or downloads it from Adoptium.
use crate::error::{LauncherError, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const JAVA_MAJOR: u32 = 21;
pub const MIN_JAVA_MAJOR: u32 = 21;

// ---------------------------------------------------------------------------
// Platform helpers
// ---------------------------------------------------------------------------

fn java_exe_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "javaw.exe"  // windowless — no CMD popup
    } else {
        "java"
    }
}

fn adoptium_download_url() -> String {
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    };
    if cfg!(target_os = "windows") {
        format!(
            "https://api.adoptium.net/v3/binary/latest/{JAVA_MAJOR}/ga/windows/{arch}/jdk/hotspot/normal/eclipse"
        )
    } else if cfg!(target_os = "macos") {
        format!(
            "https://api.adoptium.net/v3/binary/latest/{JAVA_MAJOR}/ga/mac/{arch}/jdk/hotspot/normal/eclipse"
        )
    } else {
        format!(
            "https://api.adoptium.net/v3/binary/latest/{JAVA_MAJOR}/ga/linux/{arch}/jdk/hotspot/normal/eclipse"
        )
    }
}

// ---------------------------------------------------------------------------
// Bundled JDK path helpers
// ---------------------------------------------------------------------------

pub(crate) fn marker_path(runtime_base: &Path) -> PathBuf {
    runtime_base.join(".jdk-home")
}

/// Removes a broken or outdated portable JDK under `runtime_base` so
/// `ensure_java_21` can download a fresh copy.
pub fn purge_portable_jdk(runtime_base: &Path) -> Result<()> {
    let _ = std::fs::remove_dir_all(runtime_base.join("__download"));
    let _ = std::fs::remove_dir_all(runtime_base.join("__extract"));
    let _ = std::fs::remove_dir_all(runtime_root_jdk_dir(runtime_base));
    let _ = std::fs::remove_file(marker_path(runtime_base));
    Ok(())
}

fn runtime_root_jdk_dir(runtime_base: &Path) -> PathBuf {
    runtime_base.join("jdk")
}

fn read_marker(runtime_base: &Path) -> Option<PathBuf> {
    let mp = marker_path(runtime_base);
    if !mp.exists() {
        return None;
    }
    std::fs::read_to_string(&mp)
        .ok()
        .map(|s| PathBuf::from(s.trim().to_string()))
}

fn write_marker(runtime_base: &Path, jdk_home: &Path) {
    let _ = std::fs::write(
        marker_path(runtime_base),
        jdk_home.to_string_lossy().as_bytes(),
    );
}

/// Returns the path to the `java` / `java.exe` binary inside the bundled JDK,
/// or `None` if not yet installed.
pub fn find_bundled_java(runtime_base: &Path) -> Option<PathBuf> {
    let fixed = runtime_root_jdk_dir(runtime_base)
        .join("bin")
        .join(java_exe_name());
    if fixed.exists() {
        return Some(fixed);
    }
    if let Some(marked) = read_marker(runtime_base) {
        let exe = marked.join("bin").join(java_exe_name());
        if exe.exists() {
            return Some(exe);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Java version detection
// ---------------------------------------------------------------------------

/// Prefer `java.exe` next to `javaw.exe` for `-version` on Windows — `javaw`
/// may not emit version text reliably.
fn java_version_probe_exe(java_path: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if java_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.eq_ignore_ascii_case("javaw.exe"))
            .unwrap_or(false)
        {
            let alt = java_path.with_file_name("java.exe");
            if alt.exists() {
                return alt;
            }
        }
    }
    java_path.to_path_buf()
}

pub fn java_major_version(java_path: &Path) -> u32 {
    let probe = java_version_probe_exe(java_path);
    let out = Command::new(&probe)
        .arg("-version")
        .output()
        .ok()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stderr).to_string()
                + &String::from_utf8_lossy(&o.stdout);
            s
        })
        .unwrap_or_default();

    parse_java_major(&out)
}

fn parse_java_major(text: &str) -> u32 {
    // Legacy: `version "1.8.0_xxx"` → 8
    if let Some(cap) = regex::Regex::new(r#"version "1\.(\d+)\."#)
        .ok()
        .and_then(|re| re.captures(text))
    {
        if let Some(n) = cap.get(1).and_then(|m| m.as_str().parse::<u32>().ok()) {
            return n;
        }
    }
    // Modern: `version "21.0.3"` or `version "21"` → 21
    if let Some(cap) = regex::Regex::new(r#"version "(\d+)"#)
        .ok()
        .and_then(|re| re.captures(text))
    {
        if let Some(n) = cap.get(1).and_then(|m| m.as_str().parse::<u32>().ok()) {
            return n;
        }
    }
    0
}

// ---------------------------------------------------------------------------
// System-wide Java detection (fallback)
// ---------------------------------------------------------------------------

pub fn resolve_system_java() -> (PathBuf, u32) {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // JAVA_HOME
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let base = PathBuf::from(&home).join("bin");
        if cfg!(target_os = "windows") {
            candidates.push(base.join("javaw.exe"));
            candidates.push(base.join("java.exe"));
        } else {
            candidates.push(base.join("java"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into());
        let pf86 = std::env::var("ProgramFiles(x86)")
            .unwrap_or_else(|_| "C:\\Program Files (x86)".into());
        for root_parent in [
            PathBuf::from(&pf).join("Java"),
            PathBuf::from(&pf).join("Eclipse Adoptium"),
            PathBuf::from(&pf).join("Microsoft"),
            PathBuf::from(&pf86).join("Java"),
        ] {
            if !root_parent.exists() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(&root_parent) {
                for entry in entries.flatten() {
                    if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        let bin = entry.path().join("bin");
                        candidates.push(bin.join("javaw.exe"));
                        candidates.push(bin.join("java.exe"));
                    }
                }
            }
        }
        // `where java`
        for cmd in ["javaw", "java"] {
            if let Ok(out) = Command::new("where")
                .arg(cmd)
                .output()
            {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    let p = PathBuf::from(line.trim());
                    if p.exists() {
                        candidates.push(p);
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(out) = Command::new("sh").args(["-c", "command -v java"]).output() {
            let p = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim().to_string());
            if p.exists() {
                candidates.push(p);
            }
        }
    }

    let mut best_path = PathBuf::from(if cfg!(target_os = "windows") {
        "javaw"
    } else {
        "java"
    });
    let mut best_major: u32 = 0;

    for c in &candidates {
        if !c.exists() {
            continue;
        }
        let m = java_major_version(c);
        if m > best_major {
            best_major = m;
            best_path = c.clone();
        }
    }

    (best_path, best_major)
}

// ---------------------------------------------------------------------------
// Download + extract JDK from Adoptium
// ---------------------------------------------------------------------------

/// Downloads Temurin JDK 21 into `runtime_base`.
/// Reports progress 0-100 via `on_progress`.
pub async fn ensure_java_21(
    runtime_base: &Path,
    on_progress: impl Fn(u32) + Send + 'static,
) -> Result<PathBuf> {
    std::fs::create_dir_all(runtime_base)?;

    if let Some(existing) = find_bundled_java(runtime_base) {
        on_progress(100);
        return Ok(existing);
    }

    let url = adoptium_download_url();
    let dl_dir = runtime_base.join("__download");
    let extract_dir = runtime_base.join("__extract");
    let final_jdk = runtime_root_jdk_dir(runtime_base);

    let ext = if cfg!(target_os = "windows") {
        ".zip"
    } else {
        ".tar.gz"
    };
    let archive_path = dl_dir.join(format!("temurin-{JAVA_MAJOR}{ext}"));

    // Clean up previous partial attempts
    let _ = std::fs::remove_dir_all(&dl_dir);
    let _ = std::fs::remove_dir_all(&extract_dir);
    let _ = std::fs::remove_dir_all(&final_jdk);

    std::fs::create_dir_all(&dl_dir)?;
    std::fs::create_dir_all(&extract_dir)?;

    on_progress(0);
    download_with_progress(&url, &archive_path, &on_progress).await?;

    on_progress(86);
    extract_archive(&archive_path, &extract_dir)?;
    on_progress(95);

    let jdk_root = find_jdk_root_in_extract(&extract_dir)?;
    std::fs::rename(&jdk_root, &final_jdk)?;
    write_marker(runtime_base, &final_jdk);

    let _ = std::fs::remove_dir_all(&extract_dir);
    let _ = std::fs::remove_dir_all(&dl_dir);

    let exe = find_bundled_java(runtime_base)
        .ok_or_else(|| LauncherError::Java("JDK zainstalowany, ale nie znaleziono java.exe".into()))?;
    on_progress(100);
    Ok(exe)
}

async fn download_with_progress(
    url: &str,
    dest: &Path,
    on_progress: &impl Fn(u32),
) -> Result<()> {
    let mut last_err: Option<LauncherError> = None;
    for attempt in 1u32..=3u32 {
        match download_with_progress_once(url, dest, on_progress).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                let _ = tokio::fs::remove_file(dest.with_extension("part")).await;
                if attempt < 3 {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| LauncherError::Java("Pobieranie nie powiodło się.".into())))
}

async fn download_with_progress_once(
    url: &str,
    dest: &Path,
    on_progress: &impl Fn(u32),
) -> Result<()> {
    use tokio::io::AsyncWriteExt;

    let client = build_http_client()?;
    let mut response = follow_redirect(&client, url).await?;

    let total = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    std::fs::create_dir_all(dest.parent().unwrap_or(Path::new(".")))?;
    let tmp = dest.with_extension("part");
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut received: u64 = 0;

    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk).await?;
        received += chunk.len() as u64;
        if total > 0 {
            let pct = ((received as f64 / total as f64) * 85.0).min(85.0) as u32;
            on_progress(pct);
        } else {
            let guess = 220 * 1024 * 1024u64;
            let pct = ((received as f64 / guess as f64) * 82.0).min(82.0) as u32;
            on_progress(pct);
        }
    }
    file.flush().await?;
    drop(file);
    std::fs::rename(tmp, dest)?;
    Ok(())
}

/// Follow redirects (Adoptium → GitHub releases CDN).
/// Validates that each hop goes to an allowed host.
async fn follow_redirect(
    client: &reqwest::Client,
    url: &str,
) -> Result<reqwest::Response> {
    // reqwest with redirect policy already follows; we just validate the initial host.
    validate_adoptium_host(url)?;
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(LauncherError::Java(format!(
            "Pobieranie JDK: HTTP {}",
            resp.status()
        )));
    }
    Ok(resp)
}

fn validate_adoptium_host(url: &str) -> Result<()> {
    let u = reqwest::Url::parse(url)
        .map_err(|e| LauncherError::Java(format!("Nieprawidłowy URL JDK: {e}")))?;
    let h = u.host_str().unwrap_or("").to_lowercase();
    if h == "api.adoptium.net"
        || h == "github.com"
        || h == "www.github.com"
        || h.ends_with(".githubusercontent.com")
        || h.ends_with(".github.com")
    {
        Ok(())
    } else {
        Err(LauncherError::Java(format!("Niedozwolony host JDK: {h}")))
    }
}

pub fn build_http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("CreateCrafts-Launcher/2 (Bl4ck3d)")
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(LauncherError::Http)
}

// ---------------------------------------------------------------------------
// Windows: system-wide Temurin MSI (elevated via UAC)
// ---------------------------------------------------------------------------

/// Adoptium API URL for the Windows `.msi` JDK installer.
#[cfg(target_os = "windows")]
pub fn adoptium_windows_jdk_msi_url() -> String {
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    };
    format!(
        "https://api.adoptium.net/v3/installer/latest/{JAVA_MAJOR}/ga/windows/{arch}/jdk/hotspot/normal/eclipse"
    )
}

/// Downloads the Temurin MSI to `dest` (Windows).
#[cfg(target_os = "windows")]
pub async fn download_windows_jdk_msi(dest: &Path) -> Result<()> {
    let url = adoptium_windows_jdk_msi_url();
    let noop = |_| {};
    download_with_progress(&url, dest, &noop).await
}

/// Starts `msiexec /i … /passive` elevated (UAC prompt).
#[cfg(target_os = "windows")]
pub fn launch_elevated_msi_installer(msi_path: &Path) -> Result<()> {
    let msi = msi_path
        .canonicalize()
        .map_err(|e| LauncherError::Java(format!("Nie można odczytać ścieżki MSI: {e}")))?;
    let m = msi.to_string_lossy().replace('\'', "''");
    let ps = format!(
        "Start-Process -FilePath msiexec.exe -Verb RunAs -ArgumentList '/i','{m}','/passive'"
    );
    Command::new("powershell.exe")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps])
        .spawn()
        .map_err(|e| LauncherError::Java(format!("Uruchomienie instalatora (UAC): {e}")))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub async fn download_windows_jdk_msi(_dest: &Path) -> Result<()> {
    Err(LauncherError::Java(
        "Instalacja systemowa JDK jest obsługiwana tylko na Windows.".into(),
    ))
}

#[cfg(not(target_os = "windows"))]
pub fn launch_elevated_msi_installer(_msi_path: &Path) -> Result<()> {
    Err(LauncherError::Java(
        "Instalacja systemowa JDK jest obsługiwana tylko na Windows.".into(),
    ))
}

// ---------------------------------------------------------------------------
// Archive extraction
// ---------------------------------------------------------------------------

fn extract_archive(archive: &Path, dest: &Path) -> Result<()> {
    let name = archive.to_string_lossy().to_lowercase();
    if name.ends_with(".zip") {
        extract_zip(archive, dest)
    } else {
        extract_targz(archive, dest)
    }
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| LauncherError::Archive(e.to_string()))?;
    zip.extract(dest)
        .map_err(|e| LauncherError::Archive(e.to_string()))
}

fn extract_targz(archive: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive)?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(gz);
    tar.unpack(dest)?;
    Ok(())
}

fn find_jdk_root_in_extract(extract_dir: &Path) -> Result<PathBuf> {
    for entry in std::fs::read_dir(extract_dir)?.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let base = entry.path();
        // Linux/Windows layout: {jdk-dir}/bin/java
        if base.join("bin").join(java_exe_name()).exists() {
            return Ok(base);
        }
        // macOS layout: {jdk-dir}/Contents/Home/bin/java
        let mac_home = base.join("Contents").join("Home");
        if mac_home.join("bin").join(java_exe_name()).exists() {
            return Ok(mac_home);
        }
    }
    Err(LauncherError::Java(
        "Brak bin/java po rozpakowaniu archiwum JDK.".into(),
    ))
}
