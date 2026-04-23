use crate::error::{LauncherError, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;
pub const REQUIRED_JAVA_MAJOR: u32 = 21;

#[derive(Debug, Clone)]
pub enum JavaRuntimeSource {
    LocalCache,
    SystemJava21,
    Downloaded,
}

#[derive(Debug, Clone)]
pub struct JavaRuntime {
    pub javaw_path: PathBuf,
    pub java_path: PathBuf,
    pub major: u32,
    pub source: JavaRuntimeSource,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct AdoptiumAsset {
    binary: AdoptiumBinary,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct AdoptiumPackage {
    link: String,
}

struct InstallLock {
    path: PathBuf,
}

impl InstallLock {
    fn acquire(path: PathBuf) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| {
                LauncherError::JavaRuntime(format!(
                    "Inny proces właśnie instaluje Java 21 ({e})."
                ))
            })?;
        Ok(Self { path })
    }
}

impl Drop for InstallLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn runtime_java21_dir(app_root: &Path) -> PathBuf {
    app_root.join("runtime").join("java21")
}

fn runtime_ready_marker(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(".ready")
}

fn runtime_install_lock(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(".install.lock")
}

fn javaw_path_for(runtime_dir: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        runtime_dir.join("bin").join("javaw.exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        runtime_dir.join("bin").join("java")
    }
}

fn java_path_for(runtime_dir: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        runtime_dir.join("bin").join("java.exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        runtime_dir.join("bin").join("java")
    }
}

fn parse_java_major(text: &str) -> Option<u32> {
    if let Ok(re) = regex::Regex::new(r#"version "1\.(\d+)\."#) {
        if let Some(cap) = re.captures(text) {
            if let Some(n) = cap.get(1).and_then(|m| m.as_str().parse::<u32>().ok()) {
                return Some(n);
            }
        }
    }
    if let Ok(re) = regex::Regex::new(r#"version "(\d+)"#) {
        if let Some(cap) = re.captures(text) {
            if let Some(n) = cap.get(1).and_then(|m| m.as_str().parse::<u32>().ok()) {
                return Some(n);
            }
        }
    }
    None
}

pub fn probe_java_major(java_path: &Path) -> Result<u32> {
    if !java_path.exists() {
        return Err(LauncherError::JavaRuntime(format!(
            "Nie znaleziono pliku Java: {}",
            java_path.display()
        )));
    }
    let mut cmd = Command::new(java_path);
    cmd.arg("-version");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().map_err(|e| {
        LauncherError::JavaRuntime(format!(
            "Nie można sprawdzić wersji Java ({}): {e}",
            java_path.display()
        ))
    })?;
    let combined =
        String::from_utf8_lossy(&output.stderr).to_string() + &String::from_utf8_lossy(&output.stdout);
    parse_java_major(&combined).ok_or_else(|| {
        LauncherError::JavaRuntime(format!(
            "Nie udało się odczytać wersji Java z outputu {}",
            java_path.display()
        ))
    })
}

fn validate_runtime_dir(runtime_dir: &Path) -> Result<Option<JavaRuntime>> {
    let marker = runtime_ready_marker(runtime_dir);
    let java_path = java_path_for(runtime_dir);
    let javaw_path = javaw_path_for(runtime_dir);
    if !marker.exists() || !java_path.exists() || !javaw_path.exists() {
        return Ok(None);
    }
    let major = probe_java_major(&java_path)?;
    if major != REQUIRED_JAVA_MAJOR {
        return Ok(None);
    }
    Ok(Some(JavaRuntime {
        javaw_path,
        java_path,
        major,
        source: JavaRuntimeSource::LocalCache,
    }))
}

fn collect_system_candidates() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(home) = std::env::var("JAVA_HOME") {
        #[cfg(target_os = "windows")]
        {
            candidates.push(PathBuf::from(&home).join("bin").join("java.exe"));
        }
        #[cfg(not(target_os = "windows"))]
        {
            candidates.push(PathBuf::from(&home).join("bin").join("java"));
        }
    }
    #[cfg(target_os = "windows")]
    {
        let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string());
        let pf86 = std::env::var("ProgramFiles(x86)")
            .unwrap_or_else(|_| "C:\\Program Files (x86)".to_string());
        for root_parent in [
            PathBuf::from(&pf).join("Java"),
            PathBuf::from(&pf).join("Eclipse Adoptium"),
            PathBuf::from(&pf).join("Microsoft"),
            PathBuf::from(&pf86).join("Java"),
        ] {
            if let Ok(entries) = std::fs::read_dir(root_parent) {
                for entry in entries.flatten() {
                    let bin = entry.path().join("bin").join("java.exe");
                    candidates.push(bin);
                }
            }
        }
        let mut wc = Command::new("where");
        wc.arg("java");
        wc.creation_flags(CREATE_NO_WINDOW);
        if let Ok(out) = wc.output() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let p = PathBuf::from(line.trim());
                if p.exists() {
                    candidates.push(p);
                }
            }
        }
    }
    candidates
}

fn resolve_system_java21() -> Result<Option<JavaRuntime>> {
    let candidates = collect_system_candidates();
    for candidate in candidates {
        if !candidate.exists() {
            continue;
        }
        if let Ok(major) = probe_java_major(&candidate) {
            if major == REQUIRED_JAVA_MAJOR {
                let javaw_path = {
                    #[cfg(target_os = "windows")]
                    {
                        let c = candidate.with_file_name("javaw.exe");
                        if c.exists() {
                            c
                        } else {
                            candidate.clone()
                        }
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        candidate.clone()
                    }
                };
                return Ok(Some(JavaRuntime {
                    javaw_path,
                    java_path: candidate,
                    major,
                    source: JavaRuntimeSource::SystemJava21,
                }));
            }
        }
    }
    Ok(None)
}

async fn fetch_temurin_jre21_download_url() -> Result<String> {
    #[cfg(not(target_os = "windows"))]
    {
        return Err(LauncherError::JavaRuntime(
            "Auto-download Java 21 jest obecnie wspierany tylko na Windows.".to_string(),
        ));
    }
    #[cfg(target_os = "windows")]
    {
        let url = "https://api.adoptium.net/v3/assets/latest/21/hotspot?architecture=x64&image_type=jre&os=windows&heap_size=normal&vendor=eclipse";
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(LauncherError::Http)?;
        let assets: Vec<AdoptiumAsset> = client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let first = assets.first().ok_or_else(|| {
            LauncherError::JavaRuntime("Adoptium API nie zwróciło paczki JRE 21.".to_string())
        })?;
        Ok(first.binary.package.link.clone())
    }
}

fn extract_zip_archive(archive: &Path, destination: &Path) -> Result<()> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| LauncherError::Archive(e.to_string()))?;
    std::fs::create_dir_all(destination)?;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| LauncherError::Archive(e.to_string()))?;
        let Some(safe_path) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
            continue;
        };
        let out_path = destination.join(safe_path);
        if entry.name().ends_with('/') {
            std::fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(&out_path)?;
        std::io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

async fn download_temurin_jre21_windows_x64(app_root: &Path) -> Result<PathBuf> {
    let download_url = fetch_temurin_jre21_download_url().await?;
    let tmp_dir = app_root.join("runtime").join("tmp");
    std::fs::create_dir_all(&tmp_dir)?;
    let archive_path = tmp_dir.join("temurin-jre21.zip");
    let part_path = tmp_dir.join("temurin-jre21.zip.part");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(LauncherError::Http)?;
    let response = client
        .get(&download_url)
        .send()
        .await?
        .error_for_status()?;
    let bytes = response.bytes().await?;
    std::fs::write(&part_path, &bytes)?;
    std::fs::rename(&part_path, &archive_path)?;
    Ok(archive_path)
}

fn locate_java_home_after_extract(runtime_dir: &Path) -> Result<PathBuf> {
    let direct_java = java_path_for(runtime_dir);
    if direct_java.exists() {
        return Ok(runtime_dir.to_path_buf());
    }
    let entries = std::fs::read_dir(runtime_dir)?;
    for entry in entries.flatten() {
        let candidate = entry.path();
        if candidate.is_dir() && java_path_for(&candidate).exists() {
            return Ok(candidate);
        }
    }
    Err(LauncherError::JavaRuntime(
        "Nie znaleziono bin/java.exe w pobranym runtime Java 21.".to_string(),
    ))
}

pub async fn ensure_local_java21(app_root: &Path) -> Result<JavaRuntime> {
    let runtime_dir = runtime_java21_dir(app_root);
    if let Some(runtime) = validate_runtime_dir(&runtime_dir)? {
        return Ok(runtime);
    }
    let _lock = InstallLock::acquire(runtime_install_lock(&runtime_dir))?;
    if let Some(runtime) = validate_runtime_dir(&runtime_dir)? {
        return Ok(runtime);
    }

    let archive = download_temurin_jre21_windows_x64(app_root).await?;
    if runtime_dir.exists() {
        std::fs::remove_dir_all(&runtime_dir)?;
    }
    std::fs::create_dir_all(&runtime_dir)?;
    extract_zip_archive(&archive, &runtime_dir)?;

    let java_home = locate_java_home_after_extract(&runtime_dir)?;
    if java_home != runtime_dir {
        let java_home_name = java_home
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or_else(|| LauncherError::JavaRuntime("Niepoprawna ścieżka Java home.".to_string()))?;
        for entry in std::fs::read_dir(&java_home)? {
            let entry = entry?;
            let to = runtime_dir.join(entry.file_name());
            std::fs::rename(entry.path(), to)?;
        }
        let _ = std::fs::remove_dir_all(runtime_dir.join(java_home_name));
    }

    let java_path = java_path_for(&runtime_dir);
    let javaw_path = javaw_path_for(&runtime_dir);
    let major = probe_java_major(&java_path)?;
    if major != REQUIRED_JAVA_MAJOR {
        return Err(LauncherError::JavaRuntime(format!(
            "Pobrano niekompatybilną Java ({}), wymagana jest {}.",
            major, REQUIRED_JAVA_MAJOR
        )));
    }
    std::fs::write(runtime_ready_marker(&runtime_dir), format!("major={major}\n"))?;
    Ok(JavaRuntime {
        javaw_path,
        java_path,
        major,
        source: JavaRuntimeSource::Downloaded,
    })
}

pub async fn resolve_java21_runtime(app_root: &Path) -> Result<JavaRuntime> {
    let runtime_dir = runtime_java21_dir(app_root);
    if let Some(runtime) = validate_runtime_dir(&runtime_dir)? {
        return Ok(runtime);
    }
    if let Some(runtime) = resolve_system_java21()? {
        return Ok(runtime);
    }
    ensure_local_java21(app_root).await
}
