pub mod commands;
pub mod crypto;
pub mod error;
pub mod minecraft;
pub mod session;

pub use error::{LauncherError, Result};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let res = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::auth::login_microsoft,
            commands::auth::delete_premium_session,
            commands::auth::migrate_profiles_from_localstorage,
            commands::auth::mineatar_face_url,
            commands::game::start_game,
            commands::mods::get_mods_info,
            commands::mods::force_mod_resync_next,
            commands::mods::force_mod_resync_pending,
            commands::shell::open_path_in_explorer,
            commands::shell::open_external_url,
            commands::update::get_app_version,
            commands::update::check_launcher_update,
            commands::update::download_and_install_launcher_update,
        ])
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                use tauri::Manager;
                if let Some(w) = app.get_webview_window("main") {
                    w.open_devtools();
                }
            }
            {
                use tauri::image::Image;
                use tauri::Manager;
                const ICON_PNG: &[u8] = include_bytes!("../icons/128x128.png");
                if let Ok(dyn_img) = image::load_from_memory(ICON_PNG) {
                    let rgba_img = dyn_img.to_rgba8();
                    let (w, h) = rgba_img.dimensions();
                    let rgba = rgba_img.into_raw();
                    let icon = Image::new_owned(rgba, w, h);
                    if let Some(win) = app.get_webview_window("main") {
                        let _ = win.set_icon(icon);
                    }
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!());
    if let Err(e) = res {
        eprintln!("CreateCrafts Launcher: {e}");
        std::process::exit(1);
    }
}
