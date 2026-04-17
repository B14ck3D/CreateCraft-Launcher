use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilities {
    pub os: String,
    pub supports_system_jdk_installer: bool,
}

#[tauri::command]
pub async fn platform_capabilities() -> std::result::Result<PlatformCapabilities, String> {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };

    Ok(PlatformCapabilities {
        os: os.to_string(),
        supports_system_jdk_installer: cfg!(target_os = "windows"),
    })
}
