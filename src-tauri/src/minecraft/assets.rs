use crate::error::{LauncherError, Result};
use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

const VERSION_MANIFEST: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

// Mojang API types

#[derive(Debug, Deserialize)]
struct VersionManifest {
    versions: Vec<VersionSummary>,
}

#[derive(Debug, Deserialize)]
struct VersionSummary {
    id: String,
    url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DownloadMeta {
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub url: String,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LibraryDownloads {
    pub artifact: Option<DownloadMeta>,
    pub classifiers: Option<std::collections::HashMap<String, DownloadMeta>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LibraryNatives {
    pub windows: Option<String>,
    pub linux: Option<String>,
    pub osx: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OsRule {
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LibraryRule {
    pub action: String,
    pub os: Option<OsRule>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    pub rules: Option<Vec<LibraryRule>>,
    pub natives: Option<LibraryNatives>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct AssetIndexMeta {
    pub id: String,
    pub sha1: String,
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Argument {
    // Can be a plain string or an object with rules
    #[serde(flatten)]
    pub raw: serde_json::Value,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<serde_json::Value>,
    #[serde(default)]
    pub jvm: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct VersionDownloads {
    pub client: DownloadMeta,
}

#[derive(Debug, Deserialize, Clone)]
pub struct VersionJson {
    pub id: String,
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub downloads: Option<VersionDownloads>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(rename = "assetIndex", default)]
    pub asset_index: Option<AssetIndexMeta>,
    #[serde(default)]
    pub assets: String,
    #[serde(rename = "mainClass", default)]
    pub main_class: String,
    pub arguments: Option<Arguments>,
    #[serde(rename = "minecraftArguments")]
    pub minecraft_arguments: Option<String>,
    #[serde(rename = "inheritsFrom")]
    pub inherits_from: Option<String>,
}

// SHA1 verification

fn sha1_hex_of_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha1::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

fn sha1_valid(path: &Path, expected: &str) -> bool {
    sha1_hex_of_file(path)
        .map(|h| h.eq_ignore_ascii_case(expected))
        .unwrap_or(false)
}

// HTTP helpers

pub fn build_mc_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("CreateCrafts-Launcher/2 (Bl4ck3d)")
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(LauncherError::Http)
}

async fn download_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(LauncherError::Minecraft(format!(
            "HTTP {} dla {}",
            resp.status(),
            url
        )));
    }
    Ok(resp.bytes().await?.to_vec())
}

async fn download_file_if_needed(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    expected_sha1: Option<&str>,
) -> Result<()> {
    if dest.exists() {
        if let Some(sha) = expected_sha1 {
            if sha1_valid(dest, sha) {
                return Ok(());
            }
        } else {
            return Ok(());
        }
    }

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp = dest.with_extension("part");
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(LauncherError::Minecraft(format!(
            "HTTP {} dla {}",
            resp.status(),
            url
        )));
    }

    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    drop(file);

    if let Some(sha) = expected_sha1 {
        if !sha1_valid(&tmp, sha) {
            let _ = std::fs::remove_file(&tmp);
            return Err(LauncherError::Minecraft(format!(
                "SHA1 niezgodny dla: {}",
                url
            )));
        }
    }

    std::fs::rename(tmp, dest)?;
    Ok(())
}

// Public API

pub fn version_json_path(game_root: &Path, version_id: &str) -> PathBuf {
    game_root
        .join("versions")
        .join(version_id)
        .join(format!("{version_id}.json"))
}

pub fn client_jar_path(game_root: &Path, version_id: &str) -> PathBuf {
    game_root
        .join("versions")
        .join(version_id)
        .join(format!("{version_id}.jar"))
}

pub async fn fetch_version_json(
    client: &reqwest::Client,
    game_root: &Path,
    version_id: &str,
) -> Result<VersionJson> {
    let dest = version_json_path(game_root, version_id);

    if !dest.exists() {
        // Find URL from manifest
        let manifest_bytes = download_bytes(client, VERSION_MANIFEST).await?;
        let manifest: VersionManifest = serde_json::from_slice(&manifest_bytes)?;
        let entry = manifest
            .versions
            .iter()
            .find(|v| v.id == version_id)
            .ok_or_else(|| {
                LauncherError::Minecraft(format!("Wersja {version_id} nie znaleziona w manifeście"))
            })?;

        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p)?;
        }
        let json_bytes = download_bytes(client, &entry.url).await?;
        std::fs::write(&dest, &json_bytes)?;
    }

    let json: VersionJson = serde_json::from_slice(&std::fs::read(&dest)?)?;
    Ok(json)
}


pub async fn resolve_full_version(
    client: &reqwest::Client,
    game_root: &Path,
    version: VersionJson,
) -> Result<VersionJson> {
    let base_id = match &version.inherits_from {
        Some(id) => id.clone(),
        None => return Ok(version),
    };

    // Fetch base vanilla version JSON (e.g. "1.21.1")
    let base = fetch_version_json(client, game_root, &base_id).await?;

    // Merged: start with base, overlay NeoForge fields
    let mut merged = base;
    merged.id = version.id;
    // Use NeoForge main class (NeoForge sets a different one)
    merged.main_class = version.main_class;
    // Prepend NeoForge libraries so they take precedence in classpath order
    let mut libs = version.libraries;
    libs.extend(merged.libraries);
    merged.libraries = libs;
    // Merge arguments if present
    if let Some(nf_args) = version.arguments {
        match merged.arguments.as_mut() {
            Some(base_args) => {
                // Prepend NeoForge args so they come first
                let mut g = nf_args.game;
                g.extend(std::mem::take(&mut base_args.game));
                base_args.game = g;
                let mut j = nf_args.jvm;
                j.extend(std::mem::take(&mut base_args.jvm));
                base_args.jvm = j;
            }
            None => merged.arguments = Some(nf_args),
        }
    }
    // Keep base minecraftArguments for legacy format fallback
    if merged.minecraft_arguments.is_none() {
        merged.minecraft_arguments = version.minecraft_arguments;
    }
    merged.inherits_from = None;

    Ok(merged)
}

pub async fn download_minecraft_files(
    client: &reqwest::Client,
    game_root: &Path,
    version: &VersionJson,
    on_progress: impl Fn(u32) + Send,
) -> Result<()> {
    let libs_dir = game_root.join("libraries");
    let versions_dir = game_root.join("versions").join(&version.id);
    std::fs::create_dir_all(&versions_dir)?;
    std::fs::create_dir_all(&libs_dir)?;

    // Collect all downloads
    let mut tasks: Vec<(String, PathBuf, Option<String>)> = Vec::new();

    // client.jar — only present in vanilla/full version JSONs
    if let Some(ref dl) = version.downloads {
        let client_jar = client_jar_path(game_root, &version.id);
        tasks.push((
            dl.client.url.clone(),
            client_jar,
            dl.client.sha1.clone(),
        ));
    }

    // libraries applicable to current OS
    let os_name = current_os_name();
    for lib in &version.libraries {
        if !library_applies(lib, &os_name) {
            continue;
        }
        if let Some(downloads) = &lib.downloads {
            if let Some(artifact) = &downloads.artifact {
                if let Some(ref path) = artifact.path {
                    let dest = libs_dir.join(path);
                    tasks.push((artifact.url.clone(), dest, artifact.sha1.clone()));
                }
            }
            // Natives classifier
            if let Some(natives) = &lib.natives {
                let classifier = match os_name.as_str() {
                    "windows" => natives.windows.as_deref(),
                    "linux" => natives.linux.as_deref(),
                    "osx" => natives.osx.as_deref(),
                    _ => None,
                };
                if let Some(cls) = classifier {
                    let cls = cls.replace("${arch}", "64");
                    if let Some(classifiers) = &downloads.classifiers {
                        if let Some(meta) = classifiers.get(&cls) {
                            if let Some(ref path) = meta.path {
                                let dest = libs_dir.join(path);
                                tasks.push((
                                    meta.url.clone(),
                                    dest,
                                    meta.sha1.clone(),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // Download asset index — only present in vanilla/full version JSONs
    let asset_index_path = if let Some(ref ai) = version.asset_index {
        let p = game_root
            .join("assets")
            .join("indexes")
            .join(format!("{}.json", ai.id));
        tasks.push((ai.url.clone(), p.clone(), Some(ai.sha1.clone())));
        Some(p)
    } else {
        None
    };

    let total = tasks.len();
    let mut done = 0usize;

    for (url, dest, sha1) in &tasks {
        download_file_if_needed(client, url, dest, sha1.as_deref()).await?;
        done += 1;
        on_progress(((done as f64 / total as f64) * 80.0) as u32);
    }

    // Download asset objects
    if let Some(ref p) = asset_index_path {
        if p.exists() {
            download_asset_objects(client, game_root, p, &on_progress).await?;
        }
    }

    on_progress(100);
    Ok(())
}

async fn download_asset_objects(
    client: &reqwest::Client,
    game_root: &Path,
    index_path: &Path,
    on_progress: &impl Fn(u32),
) -> Result<()> {
    #[derive(Deserialize)]
    struct AssetIndex {
        objects: std::collections::HashMap<String, AssetObject>,
    }
    #[derive(Deserialize)]
    struct AssetObject {
        hash: String,
        // size: u64,
    }

    let raw = std::fs::read(index_path)?;
    let index: AssetIndex = serde_json::from_slice(&raw)?;
    let objects_dir = game_root.join("assets").join("objects");
    std::fs::create_dir_all(&objects_dir)?;

    let total = index.objects.len();
    let mut done = 0usize;

    for (_name, obj) in &index.objects {
        let prefix = &obj.hash[..2];
        let dest = objects_dir.join(prefix).join(&obj.hash);
        let url = format!(
            "https://resources.download.minecraft.net/{prefix}/{}",
            obj.hash
        );
        download_file_if_needed(client, &url, &dest, Some(&obj.hash)).await?;
        done += 1;
        if done % 50 == 0 {
            on_progress(80 + ((done as f64 / total as f64) * 20.0) as u32);
        }
    }
    Ok(())
}

// Library filtering

fn current_os_name() -> String {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
    .to_string()
}

fn library_applies(lib: &Library, os_name: &str) -> bool {
    let rules = match &lib.rules {
        Some(r) if !r.is_empty() => r,
        _ => return true,
    };

    let mut allow = false;
    for rule in rules {
        let matches_os = rule
            .os
            .as_ref()
            .and_then(|o| o.name.as_deref())
            .map(|name| name == os_name)
            .unwrap_or(true);

        if rule.action == "allow" && matches_os {
            allow = true;
        } else if rule.action == "disallow" && matches_os {
            allow = false;
        }
    }
    allow
}

pub fn build_classpath(game_root: &Path, version: &VersionJson) -> String {
    let sep = if cfg!(target_os = "windows") { ";" } else { ":" };
    let libs_dir = game_root.join("libraries");
    let os_name = current_os_name();
    let mut entries: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for lib in &version.libraries {
        if !library_applies(lib, &os_name) {
            continue;
        }
        if let Some(downloads) = &lib.downloads {
            if let Some(artifact) = &downloads.artifact {
                if let Some(ref path) = artifact.path {
                    let full = libs_dir.join(path);
                    let key = full.to_string_lossy().to_string();
                    if full.exists() && seen.insert(key.clone()) {
                        entries.push(key);
                    }
                }
            }
        }
    }

    let client_jar = client_jar_path(game_root, &version.id);
    let cj_key = client_jar.to_string_lossy().to_string();
    if client_jar.exists() && seen.insert(cj_key.clone()) {
        entries.push(cj_key);
    }

    entries.join(sep)
}

pub fn extract_natives(game_root: &Path, version: &VersionJson, natives_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(natives_dir)?;
    let libs_dir = game_root.join("libraries");
    let os_name = current_os_name();

    for lib in &version.libraries {
        if !library_applies(lib, &os_name) {
            continue;
        }
        let natives = match &lib.natives {
            Some(n) => n,
            None => continue,
        };
        let classifier = match os_name.as_str() {
            "windows" => natives.windows.as_deref(),
            "linux" => natives.linux.as_deref(),
            "osx" => natives.osx.as_deref(),
            _ => None,
        };
        let cls = match classifier {
            Some(c) => c.replace("${arch}", "64"),
            None => continue,
        };
        if let Some(downloads) = &lib.downloads {
            if let Some(classifiers) = &downloads.classifiers {
                if let Some(meta) = classifiers.get(&cls) {
                    if let Some(ref path) = meta.path {
                        let jar = libs_dir.join(path);
                        if jar.exists() {
                            let _ = extract_zip_to(&jar, natives_dir);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn extract_zip_to(archive: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| LauncherError::Archive(e.to_string()))?;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| LauncherError::Archive(e.to_string()))?;
        let name = entry.name().to_string();
        // Skip META-INF
        if name.starts_with("META-INF") || name.ends_with('/') {
            continue;
        }
        let out_path = dest.join(&name);
        if let Some(p) = out_path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut out = std::fs::File::create(&out_path)?;
        std::io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}
