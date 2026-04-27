use tauri::Emitter;
use crate::commands::auth::ensure_session_valid;
use crate::commands::mods::sync_mods;
use crate::minecraft::assets::{build_mc_client, download_minecraft_files, fetch_version_json, resolve_full_version};
use crate::minecraft::java::{resolve_bundled_java21, REQUIRED_JAVA_MAJOR};
use crate::minecraft::launcher::{build_launch_args, spawn_game, AuthInfo, LaunchConfig};
use crate::minecraft::neoforge::{ensure_neoforge, neoforge_version_json_path, resolve_neoforge_version, MC_VERSION};
use crate::session::store::load_session;
use serde::Deserialize;
use std::path::PathBuf;
#[cfg(not(debug_assertions))]
use tauri::Manager;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartGamePayload {
    pub r#type: String,
    pub offline_name: Option<String>,
    pub profile_id: Option<String>,
    pub ram_size: Option<String>,
    pub zgc_jvm_profile: Option<bool>,
    pub gc_conc_threads: Option<u32>,
}

fn sanitize_gc_conc_threads(raw: Option<u32>) -> u32 {
    raw.unwrap_or(4).clamp(1, 16)
}

fn sanitize_ram_size(raw: Option<String>) -> String {
    let value = raw.unwrap_or_else(|| "6G".to_string()).trim().to_ascii_lowercase();
    if value.is_empty() {
        return "6G".to_string();
    }
    let (num, unit) = match value.chars().last() {
        Some('g') => (&value[..value.len() - 1], "G"),
        Some('m') => (&value[..value.len() - 1], "M"),
        _ => (value.as_str(), "G"),
    };
    if let Ok(parsed) = num.trim().parse::<u32>() {
        let clamped = if unit == "G" {
            parsed.clamp(1, 16)
        } else {
            parsed.clamp(1024, 16384)
        };
        return format!("{clamped}{unit}");
    }
    "6G".to_string()
}

fn default_game_root() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("CreateCrafts")
}

fn get_resource_dir(app: &tauri::AppHandle) -> PathBuf {
    #[cfg(debug_assertions)]
    {
        let _ = app;
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

fn get_servers_dat_template(resource_dir: &std::path::Path) -> Option<PathBuf> {
    let p = resource_dir.join("servers.dat");
    if p.exists() { Some(p) } else { None }
}

fn copy_servers_dat(game_root: &std::path::Path, resource_dir: &std::path::Path) {
    if let Some(src) = get_servers_dat_template(resource_dir) {
        let dest = game_root.join("servers.dat");
        let _ = std::fs::copy(src, dest);
    }
}

fn emit_state(app: &tauri::AppHandle, state: &str) {
    let _ = app.emit("launcher-state", state);
}

fn emit_progress(app: &tauri::AppHandle, pct: u32) {
    let _ = app.emit("launcher-progress", pct);
}

fn emit_log(app: &tauri::AppHandle, msg: &str) {
    let _ = app.emit("launcher-log", msg);
}

fn emit_crash(app: &tauri::AppHandle, msg: &str) {
    let _ = app.emit("launcher-crash", msg);
}

fn append_launch_log(log_path: &std::path::Path, msg: &str) {
    use std::io::Write;
    let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ");
    let line = format!("[{ts}] {}\n", msg.replace('\n', " "));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = f.write_all(line.as_bytes());
    }
}

#[tauri::command]
pub async fn start_game(
    app: tauri::AppHandle,
    payload: StartGamePayload,
) -> std::result::Result<(), String> {
    let game_root = default_game_root();
    std::fs::create_dir_all(&game_root).map_err(|e| e.to_string())?;

    let resource_dir = get_resource_dir(&app);
    let log_path = game_root.join("createcrafts-launcher.log");

    macro_rules! log {
        ($msg:expr) => {{
            emit_log(&app, $msg);
            append_launch_log(&log_path, $msg);
        }};
        ($($arg:tt)*) => {{
            let s = format!($($arg)*);
            emit_log(&app, &s);
            append_launch_log(&log_path, &s);
        }};
    }

    macro_rules! crash_and_return {
        ($msg:expr) => {{
            let m = $msg;
            append_launch_log(&log_path, &m);
            emit_crash(&app, &m);
            emit_state(&app, "idle");
            return Err(m);
        }};
    }

    emit_state(&app, "verifying");

    let auth: AuthInfo = match payload.r#type.as_str() {
        "offline" => {
            let nick = payload
                .offline_name
                .unwrap_or_default()
                .trim()
                .to_string();
            if nick.is_empty() {
                crash_and_return!("Brak nicku offline.".to_string());
            }
            AuthInfo::offline(&nick)
        }
        "premium" => {
            let profile_id = payload.profile_id.unwrap_or_default();
            if profile_id.is_empty() {
                crash_and_return!("Brak profileId — zaloguj się ponownie przez Microsoft.".to_string());
            }
            let mut session = match load_session(&profile_id).map_err(|e| e.to_string())? {
                Some(s) => s,
                None => crash_and_return!(
                    "Brak zapisanej sesji premium — zaloguj się ponownie przez Microsoft.".to_string()
                ),
            };
            if let Err(e) = ensure_session_valid(&mut session).await {
                crash_and_return!(format!("Błąd odświeżania tokenu: {e}"));
            }
            let _ = crate::session::store::save_session(&session);
            AuthInfo::from_session(&session)
        }
        other => crash_and_return!(format!("Nieznany typ konta: {other}")),
    };

    emit_state(&app, "checking-java");
    let (java_path, java_exec_path) = match resolve_bundled_java21(&app) {
        Ok(rt) => (rt.javaw_path.clone(), rt.java_path.clone()),
        Err(e) => crash_and_return!(format!(
            "Nie udało się zlokalizować zbundlowanej Java 21: {e}\n\nPełny log: {}",
            log_path.display()
        )),
    };
    emit_progress(&app, 4);

    let use_modpack = std::env::var("CREATECRAFT_DISABLE_MODPACK").as_deref() != Ok("1")
        && std::env::var("SUPERSMP_DISABLE_MODPACK").as_deref() != Ok("1");

    if use_modpack {
        let force_flag = game_root.join("createcrafts-force-mods-resync.flag");
        let force = force_flag.exists();

        emit_state(&app, "checking-files");
        emit_progress(&app, 5);

        let app_clone = app.clone();
        let app_clone2 = app.clone();
        let resource_dir_clone = resource_dir.clone();
        let game_root_clone = game_root.clone();

        let result = sync_mods(
            &game_root_clone,
            &resource_dir_clone,
            force,
            &|msg| {
                emit_log(&app_clone, msg);
                append_launch_log(&log_path, msg);
            },
            &|pct| {
                if pct < 30 {
                    emit_state(&app_clone2, "checking-files");
                } else {
                    emit_state(&app_clone2, "mods-sync");
                }
                emit_progress(&app_clone2, pct);
            },
        )
        .await;

        if let Err(e) = result {
            crash_and_return!(format!(
                "Nie udało się zsynchronizować modów CreateCrafts.\n{e}\n\nPełny log: {}",
                log_path.display()
            ));
        }

        if force {
            let _ = std::fs::remove_file(&force_flag);
        }

        emit_state(&app, "checking-files");
        let _nf_version = match ensure_neoforge(
            &java_exec_path,
            &game_root,
            &|msg| {
                emit_log(&app, msg);
                append_launch_log(&log_path, msg);
            },
            &|pct| emit_progress(&app, pct),
        )
        .await
        {
            Ok(v) => v,
            Err(e) => crash_and_return!(format!("Błąd instalatora NeoForge: {e}\nPełny log: {}", log_path.display())),
        };

        copy_servers_dat(&game_root, &resource_dir);
    }

    emit_state(&app, "downloading");
    emit_progress(&app, 0);

    let http_client = match build_mc_client() {
        Ok(c) => c,
        Err(e) => crash_and_return!(format!("Błąd HTTP klienta: {e}")),
    };

    let version_json = if use_modpack {
        let nf_ver = resolve_neoforge_version();
        let nf_path = neoforge_version_json_path(&game_root, &nf_ver);
        if nf_path.exists() {
            match crate::minecraft::launcher::load_neoforge_version(&game_root, &nf_ver) {
                Ok(v) => v,
                Err(e) => crash_and_return!(format!("Błąd odczytu wersji NeoForge: {e}")),
            }
        } else {
            match fetch_version_json(&http_client, &game_root, MC_VERSION).await {
                Ok(v) => v,
                Err(e) => crash_and_return!(format!("Błąd pobierania wersji MC: {e}")),
            }
        }
    } else {
        match fetch_version_json(&http_client, &game_root, MC_VERSION).await {
            Ok(v) => v,
            Err(e) => crash_and_return!(format!("Błąd pobierania wersji MC: {e}")),
        }
    };

    let full_version_json = match resolve_full_version(&http_client, &game_root, version_json.clone()).await {
        Ok(v) => v,
        Err(e) => crash_and_return!(format!("Błąd rozwiązywania wersji MC: {e}")),
    };

    let app_dl = app.clone();
    let result = download_minecraft_files(
        &http_client,
        &game_root,
        &full_version_json,
        move |pct| emit_progress(&app_dl, pct),
    )
    .await;

    if let Err(e) = result {
        crash_and_return!(format!(
            "Błąd pobierania plików Minecraft: {e}\nPełny log: {}",
            log_path.display()
        ));
    }

    copy_servers_dat(&game_root, &resource_dir);

    let server_host = std::env::var("CREATECRAFT_SERVER_HOST")
        .or_else(|_| std::env::var("SUPERSMP_SERVER_HOST"))
        .unwrap_or_else(|_| "main.createcrafts.pl".to_string());
    let server_port = std::env::var("CREATECRAFT_SERVER_PORT")
        .or_else(|_| std::env::var("SUPERSMP_SERVER_PORT"))
        .unwrap_or_else(|_| "25565".to_string());

    let ram_size = sanitize_ram_size(payload.ram_size);
    let zgc_jvm_profile = payload.zgc_jvm_profile.unwrap_or(false);
    let gc_conc_threads = sanitize_gc_conc_threads(payload.gc_conc_threads);

    let launch_config = LaunchConfig {
        java_path: java_path.clone(),
        game_root: game_root.clone(),
        auth: auth.clone(),
        ram_max: ram_size.clone(),
        zgc_jvm_profile,
        gc_conc_threads,
        neoforge_version: if use_modpack {
            resolve_neoforge_version()
        } else {
            String::new()
        },
        server_host: server_host.clone(),
        server_port: server_port.clone(),
    };

    let args = match build_launch_args(&launch_config, &full_version_json).await {
        Ok(a) => a,
        Err(e) => crash_and_return!(format!("Błąd budowania argumentów JVM: {e}")),
    };

    log!(&format!(
        "Start gry {} ({}) java={} serwer={}:{}",
        MC_VERSION,
        if use_modpack { "NeoForge + CreateCrafts mods" } else { "vanilla" },
        java_path.display(),
        server_host,
        server_port
    ));
    log!(&format!(
        "Java (zbundlowana {}): java={} javaw={}",
        REQUIRED_JAVA_MAJOR,
        java_exec_path.display(),
        java_path.display()
    ));
    log!(&format!("Konfiguracja RAM: {}", ram_size));
    if zgc_jvm_profile {
        log!(&format!("Profil JVM ZGC: wlaczony, ConcGCThreads={gc_conc_threads}"));
    } else {
        log!("Profil JVM ZGC: wylaczony");
    }

    emit_state(&app, "launching");

    let app_log = app.clone();
    let app_close = app.clone();
    let log_path_clone = log_path.clone();

    spawn_game(
        &java_path,
        &args,
        &game_root,
        move |line| {
            emit_log(&app_log, &line);
            append_launch_log(&log_path_clone, &format!("[MC] {line}"));
        },
        move |code| {
            append_launch_log(&log_path, &format!("Proces Minecraft zakończony kodem {code:?}"));
            if code != Some(0) {
                emit_crash(
                    &app_close,
                    &format!(
                        "Minecraft zakończył się kodem {}.\n\nPlik logu: {}",
                        code.map(|c| c.to_string()).unwrap_or_else(|| "?".to_string()),
                        log_path.display()
                    ),
                );
            }
            emit_state(&app_close, "idle");
        },
    )
    .await
    .map_err(|e| {
        emit_crash(&app, &format!("Nie można uruchomić gry: {e}"));
        emit_state(&app, "idle");
        e.to_string()
    })?;

    emit_state(&app, "connected");
    Ok(())
}
