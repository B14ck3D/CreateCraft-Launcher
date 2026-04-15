fn main() {
    tauri_build::build();

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();

    // Embed the mods API key at compile time.
    // Priority: LAUNCHER_MODS_API_KEY env var > branding/launcher-mods-key file.
    let key = std::env::var("LAUNCHER_MODS_API_KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| k.len() >= 16)
        .or_else(|| {
            let path = format!("{manifest_dir}/../branding/launcher-mods-key");
            std::fs::read_to_string(&path).ok().and_then(|raw| {
                let t = raw.trim().to_string();
                if t.len() >= 16 { Some(t) } else { None }
            })
        });

    if let Some(k) = key {
        println!("cargo:rustc-env=LAUNCHER_MODS_API_KEY_EMBED={k}");
    }

    println!(
        "cargo:rerun-if-changed={manifest_dir}/../branding/launcher-mods-key"
    );
    println!("cargo:rerun-if-env-changed=LAUNCHER_MODS_API_KEY");
}
