pub mod commands;
pub mod crypto;
pub mod error;
pub mod minecraft;
pub mod session;

pub use error::{LauncherError, Result};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            // auth
            commands::auth::login_microsoft,
            commands::auth::delete_premium_session,
            commands::auth::migrate_profiles_from_localstorage,
            commands::auth::mineatar_face_url,
            // game
            commands::game::start_game,
            commands::game::install_system_jdk_elevated,
            // mods
            commands::mods::get_mods_info,
            commands::mods::force_mod_resync_next,
            commands::mods::force_mod_resync_pending,
            // shell
            commands::shell::open_path_in_explorer,
            commands::shell::open_external_url,
            // platform
            commands::platform::platform_capabilities,
        ])
        .setup(|_app| {
            #[cfg(debug_assertions)]
            {
                use tauri::Manager;
                let window = _app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Błąd uruchamiania CreateCrafts Launcher");
}
