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
    let arch = adoptium_arch_segment();
    if cfg!(target_os = "windows") {
        format!(
            "https://api.adoptium.net/v3/binary/latest/{JAVA_MAJOR}/ga/windows/{arch}/jdk/hotspot/normal/eclipse?project=jdk"
        )
    } else if cfg!(target_os = "macos") {
        format!(
            "https://api.adoptium.net/v3/binary/latest/{JAVA_MAJOR}/ga/mac/{arch}/jdk/hotspot/normal/eclipse?project=jdk"
        )
    } else {
        format!(
            "https://api.adoptium.net/v3/binary/latest/{JAVA_MAJOR}/ga/linux/{arch}/jdk/hotspot/normal/eclipse?project=jdk"
        )
    }
}

fn adoptium_arch_segment() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    }
}

/// OS + architecture query params for Adoptium `assets` API (fallback resolver).
fn adoptium_assets_query_os_arch() -> (&'static str, &'static str) {
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    };
    if cfg!(target_os = "windows") {
        ("windows", arch)
    } else if cfg!(target_os = "macos") {
        ("mac", arch)
    } else {
        ("linux", arch)
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
    let api_fallback = adoptium_resolve_package_link_from_api().await.ok().flatten();
    let mut candidates: Vec<String> = vec![url.to_string()];
    if let Some(alt) = api_fallback {
        if alt != url {
            candidates.push(alt);
        }
    }

    let mut last_err: Option<LauncherError> = None;
    for attempt in 1u32..=3u32 {
        for u in &candidates {
            match download_with_progress_once(u, dest, on_progress).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_err = Some(e);
                    let _ = tokio::fs::remove_file(dest.with_extension("part")).await;
                }
            }
        }
        if attempt < 3 {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
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
    let mut response = get_adoptium_binary_response(&client, url).await?;

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

    while let Some(chunk) = response.chunk().await.map_err(http_to_java_download_err)? {
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

    verify_jdk_artifact(dest, &tmp)?;
    std::fs::rename(tmp, dest)?;
    Ok(())
}

/// Map reqwest streaming/decompression errors to clearer Polish messages for JDK downloads.
fn http_to_java_download_err(e: reqwest::Error) -> LauncherError {
    let s = e.to_string();
    if s.contains("decoding") || s.contains("Decompress") {
        LauncherError::Java(format!(
            "Pobieranie JDK: problem z odczytem odpowiedzi sieci (często CDN/kompresja). Spróbuj ponownie albo użyj instalatora z ustawień. Technicznie: {s}"
        ))
    } else {
        LauncherError::Http(e)
    }
}

/// Read Adoptium `assets` JSON and return a direct `package.link` for this platform (ZIP/MSI tarball).
async fn adoptium_resolve_package_link_from_api() -> Result<Option<String>> {
    let (os, arch) = adoptium_assets_query_os_arch();
    let url = format!(
        "https://api.adoptium.net/v3/assets/feature_releases/{JAVA_MAJOR}/ga?architecture={arch}&heap_size=normal&image_type=jdk&jvm_impl=hotspot&os={os}&vendor=eclipse&project=jdk"
    );
    validate_adoptium_host(&url)?;
    let client = build_http_client()?;
    let resp = client
        .get(&url)
        .header(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("application/json"),
        )
        .send()
        .await
        .map_err(LauncherError::Http)?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value = resp.json().await.map_err(LauncherError::Http)?;
    // API shape: [ { "binaries": [ { "package": { "link": "https://...zip" } } ] } ]
    let link = v
        .get(0)
        .and_then(|rel| rel.get("binaries"))
        .and_then(|b| b.as_array())
        .and_then(|arr| arr.first())
        .and_then(|bin| bin.get("package"))
        .and_then(|p| p.get("link"))
        .and_then(|l| l.as_str())
        .map(std::string::ToString::to_string);
    if let Some(ref u) = link {
        validate_adoptium_host(u)?;
    }
    Ok(link)
}

fn verify_jdk_artifact(dest: &Path, tmp: &Path) -> Result<()> {
    use std::io::Read;
    let meta = std::fs::metadata(tmp)
        .map_err(|e| LauncherError::Java(format!("Plik pobierania JDK: {e}")))?;
    if meta.len() < 64 * 1024 {
        return Err(LauncherError::Java(
            "Pobrany plik jest zbyt mały — serwer mógł zwrócić błąd zamiast JDK. Sprawdź połączenie.".into(),
        ));
    }
    let mut f = std::fs::File::open(tmp)?;
    let mut buf = [0u8; 8];
    let n = f.read(&mut buf)?;
    if n < 4 {
        return Err(LauncherError::Java("Nie udało się zweryfikować pobranego JDK.".into()));
    }
    let lower = dest.to_string_lossy().to_lowercase();
    if lower.ends_with(".zip") {
        if buf[0] != 0x50 || buf[1] != 0x4b {
            return Err(LauncherError::Java(
                "Pobrany plik nie jest poprawnym archiwum ZIP (JDK). Możliwa blokada sieci lub strona zamiast pliku — spróbuj instalatora MSI w ustawieniach.".into(),
            ));
        }
    } else if lower.ends_with(".msi") {
        let ole = buf[0] == 0xd0 && buf[1] == 0xcf && buf[2] == 0x11 && buf[3] == 0xe0;
        if !ole && (buf[0] == b'<' || buf[0] == b'{') {
            return Err(LauncherError::Java(
                "Zamiast instalatora MSI serwer zwrócił stronę HTML/JSON — sprawdź połączenie lub pobierz JDK ręcznie z adoptium.net.".into(),
            ));
        }
        if !ole {
            return Err(LauncherError::Java(
                "Pobrany plik nie wygląda na instalator MSI JDK.".into(),
            ));
        }
    } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        if buf[0] != 0x1f || buf[1] != 0x8b {
            return Err(LauncherError::Java(
                "Pobrany plik nie jest poprawnym archiwum .tar.gz (JDK).".into(),
            ));
        }
    }
    Ok(())
}

/// GET binary from Adoptium / GitHub CDN without transparent Content-Encoding (avoids reqwest decode errors on large JDK blobs).
async fn get_adoptium_binary_response(
    client: &reqwest::Client,
    url: &str,
) -> Result<reqwest::Response> {
    validate_adoptium_host(url)?;
    static BINARY_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 CreateCrafts-Launcher/2";
    let resp = client
        .get(url)
        .header(
            reqwest::header::ACCEPT_ENCODING,
            reqwest::header::HeaderValue::from_static("identity"),
        )
        .header(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_static(BINARY_UA),
        )
        .header(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("*/*"),
        )
        .send()
        .await
        .map_err(http_to_java_download_err)?;
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
        .user_agent("Mozilla/5.0 (compatible; CreateCrafts-Launcher/2; +https://createcrafts.pl)")
        .connect_timeout(std::time::Duration::from_secs(45))
        .timeout(std::time::Duration::from_secs(300))
        .redirect(reqwest::redirect::Policy::limited(32))
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
        "https://api.adoptium.net/v3/installer/latest/{JAVA_MAJOR}/ga/windows/{arch}/jdk/hotspot/normal/eclipse?project=jdk"
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
