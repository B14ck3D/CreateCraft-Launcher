use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub async fn open_path_in_explorer(
    app: tauri::AppHandle,
    dir_path: String,
) -> std::result::Result<serde_json::Value, String> {
    let p = dir_path.trim().to_string();
    if p.is_empty() {
        return Ok(serde_json::json!({ "ok": false, "error": "Brak ścieżki" }));
    }
    // Ensure directory exists
    let _ = std::fs::create_dir_all(&p);

    app.opener()
        .reveal_item_in_dir(&p)
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "ok": true }))
}

#[tauri::command]
pub async fn open_external_url(
    app: tauri::AppHandle,
    url: String,
) -> std::result::Result<serde_json::Value, String> {
    let u = url.trim().to_string();
    if !u.starts_with("http://") && !u.starts_with("https://") {
        return Ok(serde_json::json!({
            "ok": false,
            "error": "Dozwolone są tylko adresy http(s)."
        }));
    }
    app.opener()
        .open_url(&u, None::<&str>)
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "ok": true }))
}
