use std::path::{Path, PathBuf};
use std::process::Command;

pub const JAVA_MAJOR: u32 = 21;
pub const MIN_JAVA_MAJOR: u32 = 21;
pub const JDK21_INFO_URL: &str =
    "https://adoptium.net/temurin/releases/?package=jdk&version=21";

fn java_version_probe_exe(java_path: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if java_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.eq_ignore_ascii_case("javaw.exe"))
            .unwrap_or(false)
        {
            let alt = java_path.with_file_name("java.exe");
            if alt.exists() {
                return alt;
            }
        }
    }
    java_path.to_path_buf()
}

pub fn java_major_version(java_path: &Path) -> u32 {
    let probe = java_version_probe_exe(java_path);
    let out = Command::new(&probe)
        .arg("-version")
        .output()
        .ok()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stderr).to_string()
                + &String::from_utf8_lossy(&o.stdout);
            s
        })
        .unwrap_or_default();

    parse_java_major(&out)
}

fn parse_java_major(text: &str) -> u32 {
    if let Some(cap) = regex::Regex::new(r#"version "1\.(\d+)\."#)
        .ok()
        .and_then(|re| re.captures(text))
    {
        if let Some(n) = cap.get(1).and_then(|m| m.as_str().parse::<u32>().ok()) {
            return n;
        }
    }
    if let Some(cap) = regex::Regex::new(r#"version "(\d+)"#)
        .ok()
        .and_then(|re| re.captures(text))
    {
        if let Some(n) = cap.get(1).and_then(|m| m.as_str().parse::<u32>().ok()) {
            return n;
        }
    }
    0
}

pub fn resolve_system_java() -> (PathBuf, u32) {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(home) = std::env::var("JAVA_HOME") {
        let base = PathBuf::from(&home).join("bin");
        if cfg!(target_os = "windows") {
            candidates.push(base.join("javaw.exe"));
            candidates.push(base.join("java.exe"));
        } else {
            candidates.push(base.join("java"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into());
        let pf86 = std::env::var("ProgramFiles(x86)")
            .unwrap_or_else(|_| "C:\\Program Files (x86)".into());
        for root_parent in [
            PathBuf::from(&pf).join("Java"),
            PathBuf::from(&pf).join("Eclipse Adoptium"),
            PathBuf::from(&pf).join("Microsoft"),
            PathBuf::from(&pf86).join("Java"),
        ] {
            if !root_parent.exists() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(&root_parent) {
                for entry in entries.flatten() {
                    if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        let bin = entry.path().join("bin");
                        candidates.push(bin.join("javaw.exe"));
                        candidates.push(bin.join("java.exe"));
                    }
                }
            }
        }
        for cmd in ["javaw", "java"] {
            if let Ok(out) = Command::new("where").arg(cmd).output() {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    let p = PathBuf::from(line.trim());
                    if p.exists() {
                        candidates.push(p);
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(out) = Command::new("sh").args(["-c", "command -v java"]).output() {
            let p = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim().to_string());
            if p.exists() {
                candidates.push(p);
            }
        }
    }

    let mut best_path = PathBuf::from(if cfg!(target_os = "windows") {
        "javaw"
    } else {
        "java"
    });
    let mut best_major: u32 = 0;

    for c in &candidates {
        if !c.exists() {
            continue;
        }
        let m = java_major_version(c);
        if m > best_major {
            best_major = m;
            best_path = c.clone();
        }
    }

    (best_path, best_major)
}
