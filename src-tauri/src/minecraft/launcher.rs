use crate::error::{LauncherError, Result};
use crate::minecraft::assets::{build_classpath, extract_natives, VersionJson};
use crate::minecraft::neoforge::{neoforge_version_id, neoforge_version_json_path};
use crate::session::store::PremiumSession;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct AuthInfo {
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub user_type: String, // "msa" | "legacy"
    pub xuid: Option<String>,
}

impl AuthInfo {
    pub fn offline(name: &str) -> Self {
        let uuid = offline_player_uuid(name);
        Self {
            username: name.to_string(),
            uuid,
            access_token: "0".to_string(),
            user_type: "legacy".to_string(),
            xuid: None,
        }
    }

    pub fn from_session(s: &PremiumSession) -> Self {
        Self {
            username: s.name.clone(),
            uuid: s.uuid.clone(),
            access_token: s.access_token.clone(),
            user_type: "msa".to_string(),
            xuid: s.xuid.clone(),
        }
    }
}

fn offline_player_uuid(player_name: &str) -> String {
    use md5::Digest;
    let input = format!("OfflinePlayer:{player_name}");
    let digest = md5::Md5::digest(input.as_bytes());
    let mut b = digest.to_vec();
    b[6] = (b[6] & 0x0f) | 0x30;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = hex::encode(&b);
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

// JVM performance / OS args (mirrors buildMclcJvmAugments in main.js)

fn jvm_performance_args() -> Vec<String> {
    let cpus = num_cpus::get();
    vec![
        "-XX:+UseG1GC".to_string(),
        "-XX:+ParallelRefProcEnabled".to_string(),
        "-XX:MaxGCPauseMillis=200".to_string(),
        format!(
            "-XX:ConcGCThreads={}",
            (cpus / 2).max(2).min(8)
        ),
        format!("-XX:ParallelGCThreads={cpus}"),
    ]
}

fn windows_os_spoof_args() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        let info = os_info::get();
        let ver = info.version().to_string();
        // Spoof Windows 10 on Windows 11+ so MC doesn't warn
        if ver.starts_with("10.") {
            return vec![
                "-Dos.name=Windows 10".to_string(),
                "-Dos.version=10.0".to_string(),
            ];
        }
    }
    vec![]
}

fn server_connect_args(host: &str, port: &str) -> Vec<String> {
    if host.is_empty() {
        return vec![];
    }
    if port != "25565" && !port.is_empty() {
        vec![
            "--server".to_string(),
            host.to_string(),
            "--port".to_string(),
            port.to_string(),
        ]
    } else {
        vec!["--server".to_string(), host.to_string()]
    }
}

// Version JSON merging (for NeoForge that inheritsFrom base MC)

fn merge_version_jsons(
    base: &mut VersionJson,
    overlay: &VersionJson,
) {
    // Merge libraries (overlay extends base)
    base.libraries.extend(overlay.libraries.clone());
    // Override main class
    base.main_class = overlay.main_class.clone();
    // Merge arguments if present
    if let Some(ov_args) = &overlay.arguments {
        if let Some(base_args) = &mut base.arguments {
            base_args.jvm.extend(ov_args.jvm.clone());
            base_args.game.extend(ov_args.game.clone());
        } else {
            base.arguments = Some(ov_args.clone());
        }
    }
}

pub fn load_neoforge_version(
    game_root: &Path,
    neoforge_version: &str,
) -> Result<VersionJson> {
    let _vid = neoforge_version_id(neoforge_version);
    let nf_json_path = neoforge_version_json_path(game_root, neoforge_version);

    let nf_json_bytes = std::fs::read(&nf_json_path).map_err(|_| {
        LauncherError::Minecraft(format!(
            "Nie znaleziono pliku wersji NeoForge: {}",
            nf_json_path.display()
        ))
    })?;
    let nf: VersionJson = serde_json::from_slice(&nf_json_bytes)?;

    // Load base MC version if inherited
    if let Some(ref base_id) = nf.inherits_from.clone() {
        let base_path = game_root
            .join("versions")
            .join(base_id)
            .join(format!("{base_id}.json"));
        if base_path.exists() {
            let base_bytes = std::fs::read(&base_path)?;
            let mut base: VersionJson = serde_json::from_slice(&base_bytes)?;
            merge_version_jsons(&mut base, &nf);
            return Ok(base);
        }
    }

    Ok(nf)
}

// Argument substitution

fn substitute_arg(
    arg: &str,
    vars: &std::collections::HashMap<&str, String>,
) -> String {
    let mut result = arg.to_string();
    for (k, v) in vars {
        result = result.replace(&format!("${{{k}}}"), v);
    }
    result
}

fn collect_args_from_json(
    args: &[serde_json::Value],
    vars: &std::collections::HashMap<&str, String>,
    features: &std::collections::HashSet<&str>,
) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        match arg {
            serde_json::Value::String(s) => {
                out.push(substitute_arg(s, vars));
            }
            serde_json::Value::Object(obj) => {
                // Check rules
                let rules = obj.get("rules").and_then(|r| r.as_array());
                if let Some(rules) = rules {
                    if !rules_allow(rules, features) {
                        continue;
                    }
                }
                // Collect value(s)
                if let Some(value) = obj.get("value") {
                    match value {
                        serde_json::Value::String(s) => {
                            out.push(substitute_arg(s, vars));
                        }
                        serde_json::Value::Array(arr) => {
                            for v in arr {
                                if let Some(s) = v.as_str() {
                                    out.push(substitute_arg(s, vars));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn rules_allow(rules: &[serde_json::Value], features: &std::collections::HashSet<&str>) -> bool {
    let mut allow = false;
    for rule in rules {
        let action = rule
            .get("action")
            .and_then(|a| a.as_str())
            .unwrap_or("allow");

        let os_match = rule.get("os").map(|os| {
            let name_ok = os
                .get("name")
                .and_then(|n| n.as_str())
                .map(|n| n == current_os_name())
                .unwrap_or(true);
            name_ok
        });

        let feat_match = rule.get("features").map(|f| {
            f.as_object()
                .map(|map| {
                    map.iter().all(|(k, v)| {
                        let want = v.as_bool().unwrap_or(false);
                        features.contains(k.as_str()) == want
                    })
                })
                .unwrap_or(true)
        });

        let matches = os_match.unwrap_or(true) && feat_match.unwrap_or(true);
        if action == "allow" && matches {
            allow = true;
        } else if action == "disallow" && matches {
            allow = false;
        }
    }
    allow
}

fn current_os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

// Full JVM argument assembly

pub struct LaunchConfig {
    pub java_path: PathBuf,
    pub game_root: PathBuf,
    pub auth: AuthInfo,
    pub ram_max: String,
    pub neoforge_version: String,
    pub server_host: String,
    pub server_port: String,
}

pub async fn build_launch_args(
    config: &LaunchConfig,
    version: &VersionJson,
) -> Result<Vec<String>> {
    let game_root = &config.game_root;
    let natives_dir = game_root.join("versions").join(&version.id).join("natives");
    extract_natives(game_root, version, &natives_dir)?;

    let classpath = build_classpath(game_root, version);
    let assets_dir = game_root.join("assets");
    let version_id = version.id.clone();

    let sep = if cfg!(target_os = "windows") { ";" } else { ":" };

    let mut vars: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    vars.insert("natives_directory", natives_dir.to_string_lossy().to_string());
    vars.insert("launcher_name", "CreateCrafts-Launcher".to_string());
    vars.insert("launcher_version", "2.0".to_string());
    vars.insert("classpath", classpath.clone());
    vars.insert("classpath_separator", sep.to_string());
    vars.insert("library_directory", game_root.join("libraries").to_string_lossy().to_string());
    vars.insert("auth_player_name", config.auth.username.clone());
    vars.insert("version_name", version_id.clone());
    vars.insert("game_directory", game_root.to_string_lossy().to_string());
    vars.insert("assets_root", assets_dir.to_string_lossy().to_string());
    vars.insert("game_assets", assets_dir.to_string_lossy().to_string());
    vars.insert("assets_index_name", version.assets.clone());
    vars.insert("auth_uuid", config.auth.uuid.clone());
    vars.insert("auth_access_token", config.auth.access_token.clone());
    vars.insert("auth_session", config.auth.access_token.clone());
    vars.insert("clientid", "createcrafts-launcher".to_string());
    vars.insert("auth_xuid", config.auth.xuid.clone().unwrap_or_default());
    vars.insert("user_type", config.auth.user_type.clone());
    vars.insert("version_type", "release".to_string());
    vars.insert("user_properties", "{}".to_string());
    vars.insert("resolution_width", "925".to_string());
    vars.insert("resolution_height", "530".to_string());

    let features: std::collections::HashSet<&str> = std::collections::HashSet::new();

    let mut args: Vec<String> = Vec::new();

    // Memory
    args.push(format!("-Xmx{}", config.ram_max));
    args.push("-Xms1G".to_string());

    // Performance + OS args
    args.extend(jvm_performance_args());
    args.extend(windows_os_spoof_args());

    // Branding
    args.push("-Dminecraft.launcher.brand=CreateCrafts-Launcher".to_string());
    args.push("-Dminecraft.launcher.name=CreateCrafts (NeoForge)".to_string());

    // Game directory hint
    args.push(format!(
        "-Dminecraft.applet.TargetDirectory={}",
        game_root.to_string_lossy()
    ));

    // JVM args from version JSON
    if let Some(version_args) = &version.arguments {
        let from_json = collect_args_from_json(&version_args.jvm, &vars, &features);
        args.extend(from_json);
    } else {
        // Fallback minimal JVM args for older format
        args.push(format!(
            "-Djava.library.path={}",
            natives_dir.to_string_lossy()
        ));
        args.push("-cp".to_string());
        args.push(classpath.clone());
    }

    // Main class
    args.push(version.main_class.clone());

    if let Some(version_args) = &version.arguments {
        let game_args = collect_args_from_json(&version_args.game, &vars, &features);
        args.extend(game_args);
    } else if let Some(mc_args) = &version.minecraft_arguments {
        for token in mc_args.split_whitespace() {
            args.push(substitute_arg(token, &vars));
        }
    }

    args.extend(server_connect_args(&config.server_host, &config.server_port));

    Ok(args)
}

pub async fn spawn_game(
    java_path: &Path,
    args: &[String],
    game_root: &Path,
    on_log: impl Fn(String) + Send + Sync + 'static,
    on_close: impl FnOnce(Option<i32>) + Send + 'static,
) -> Result<()> {
    let mut cmd = Command::new(java_path);
    cmd.args(args)
        .current_dir(game_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(false);

    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| LauncherError::Java(format!("Nie można uruchomić Java: {e}")))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| LauncherError::Java("Brak stdout procesu Java.".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| LauncherError::Java("Brak stderr procesu Java.".into()))?;

    let on_log = std::sync::Arc::new(on_log);
    let on_log2 = on_log.clone();

    tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            on_log(line);
        }
    });

    tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            on_log2(line);
        }
    });

    tokio::spawn(async move {
        let status = child.wait().await;
        let code = status.ok().and_then(|s| s.code());
        on_close(code);
    });

    Ok(())
}
