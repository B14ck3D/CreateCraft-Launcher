use crate::error::{LauncherError, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(not(debug_assertions))]
use tauri::path::BaseDirectory;
#[cfg(not(debug_assertions))]
use tauri::Manager;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub const REQUIRED_JAVA_MAJOR: u32 = 21;

#[derive(Debug, Clone)]
pub struct JavaRuntime {
    pub javaw_path: PathBuf,
    pub java_path: PathBuf,
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

fn probe_java_major(java_path: &Path) -> Result<u32> {
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
            "Nie można uruchomić zbundlowanej Javy ({}): {e}",
            java_path.display()
        ))
    })?;
    let combined = String::from_utf8_lossy(&output.stderr).to_string()
        + &String::from_utf8_lossy(&output.stdout);
    parse_java_major(&combined).ok_or_else(|| {
        LauncherError::JavaRuntime(format!(
            "Nie udało się odczytać wersji zbundlowanej Javy ({})",
            java_path.display()
        ))
    })
}

#[cfg(debug_assertions)]
fn bundled_jre_root_dev() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("jre21")
}

#[cfg(not(debug_assertions))]
fn bundled_jre_root_release(app: &tauri::AppHandle) -> Result<PathBuf> {
    let r = app.path().resolve("resources/jre21", BaseDirectory::Resource).map_err(|e| {
        LauncherError::JavaRuntime(format!(
            "Nie udało się zlokalizować zbundlowanego JRE 21 (PathResolver): {e}"
        ))
    })?;
    if !r.is_dir() {
        return Err(LauncherError::JavaRuntime(format!(
            "Zbundlowany JRE nie jest katalogiem: {}",
            r.display()
        )));
    }
    Ok(r)
}

fn javaw_in_jre_root(root: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        root.join("bin").join("javaw.exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        root.join("bin").join("java")
    }
}

fn java_in_jre_root(root: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        root.join("bin").join("java.exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        root.join("bin").join("java")
    }
}

pub fn resolve_bundled_java21(app: &tauri::AppHandle) -> Result<JavaRuntime> {
    #[cfg(debug_assertions)]
    let _ = app;

    let jre_root = {
        #[cfg(debug_assertions)]
        {
            let r = bundled_jre_root_dev();
            if !r.is_dir() {
                return Err(LauncherError::JavaRuntime(format!(
                    "W trybie dev brakuje zbundlowanego JRE ({}) — umieść JRE 21 w src-tauri/jre21",
                    r.display()
                )));
            }
            r
        }
        #[cfg(not(debug_assertions))]
        {
            bundled_jre_root_release(app)?
        }
    };

    let java_path = java_in_jre_root(&jre_root);
    let javaw_path = javaw_in_jre_root(&jre_root);

    if !javaw_path.exists() {
        return Err(LauncherError::JavaRuntime(format!(
            "Brak zbundlowanego javaw: {}",
            javaw_path.display()
        )));
    }
    if !java_path.exists() {
        return Err(LauncherError::JavaRuntime(format!(
            "Brak zbundlowanego java: {}",
            java_path.display()
        )));
    }

    let major = probe_java_major(&java_path)?;
    if major != REQUIRED_JAVA_MAJOR {
        return Err(LauncherError::JavaRuntime(format!(
            "Zbundlowana Java ma nieobsługiwaną wersję ({major}), wymagane jest {REQUIRED_JAVA_MAJOR}."
        )));
    }

    Ok(JavaRuntime {
        javaw_path,
        java_path,
    })
}
