/// Encrypted file-based session storage.
///
/// Each session is stored as a binary file:
///   %APPDATA%\CreateCrafts\sessions\session-{uuid}.bin
///
/// Encryption: AES-256-GCM with a key derived from machine identity
/// (COMPUTERNAME + USERNAME → SHA-256).  The same session file
/// cannot be decrypted on a different machine / user account.
///
/// File layout:  MAGIC(6) | NONCE(12) | CIPHERTEXT+TAG(n)
use crate::error::{LauncherError, Result};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

const MAGIC: &[u8; 6] = b"CCSV01";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremiumSession {
    pub uuid: String,
    pub name: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
    pub xuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_token: Option<String>,
}

// ---------------------------------------------------------------------------
// Machine-specific encryption key
// ---------------------------------------------------------------------------

fn machine_key() -> [u8; 32] {
    let host = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown-host".to_string());
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "unknown-user".to_string());

    let mut h = Sha256::new();
    h.update(b"cc-session-v1:");
    h.update(host.to_lowercase().as_bytes());
    h.update(b":");
    h.update(user.to_lowercase().as_bytes());
    h.finalize().into()
}

fn encrypt_session(plaintext: &[u8]) -> Vec<u8> {
    let key_bytes = machine_key();
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, plaintext).expect("AES-GCM encrypt");

    let mut out = Vec::with_capacity(MAGIC.len() + 12 + ciphertext.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    out
}

fn decrypt_session_bytes(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < MAGIC.len() + 12 + 16 {
        return None;
    }
    if !data.starts_with(MAGIC) {
        return None;
    }
    let nonce = Nonce::from_slice(&data[MAGIC.len()..MAGIC.len() + 12]);
    let ciphertext = &data[MAGIC.len() + 12..];

    let key_bytes = machine_key();
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    cipher.decrypt(nonce, ciphertext).ok()
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

fn sanitize_id(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect()
}

fn sessions_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("CreateCrafts")
        .join("sessions")
}

fn session_path(profile_id: &str) -> PathBuf {
    let safe_id = sanitize_id(profile_id);
    sessions_dir().join(format!("session-{safe_id}.bin"))
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn save_session(session: &PremiumSession) -> Result<()> {
    let dir = sessions_dir();
    std::fs::create_dir_all(&dir).map_err(LauncherError::Io)?;
    let path = session_path(&session.uuid);
    let json = serde_json::to_string(session)?;
    let encrypted = encrypt_session(json.as_bytes());
    std::fs::write(&path, encrypted).map_err(LauncherError::Io)
}

pub fn load_session(profile_id: &str) -> Result<Option<PremiumSession>> {
    let path = session_path(profile_id);
    if !path.exists() {
        // Also check legacy plain JSON path
        let legacy = sessions_dir().join(format!("session-{}.json", sanitize_id(profile_id)));
        if legacy.exists() {
            let raw = std::fs::read_to_string(&legacy).map_err(LauncherError::Io)?;
            let session: PremiumSession = serde_json::from_str(&raw)
                .map_err(|e| LauncherError::Session(format!("Błąd odczytu sesji: {e}")))?;
            // Migrate: re-save encrypted, remove plain
            let _ = save_session(&session);
            let _ = std::fs::remove_file(&legacy);
            return Ok(Some(session));
        }
        return Ok(None);
    }
    let data = std::fs::read(&path).map_err(LauncherError::Io)?;
    let plaintext = decrypt_session_bytes(&data)
        .ok_or_else(|| LauncherError::Session(
            "Nie można odszyfrować sesji (inne konto/komputer lub uszkodzony plik)".to_string()
        ))?;
    let session: PremiumSession = serde_json::from_slice(&plaintext)
        .map_err(|e| LauncherError::Session(format!("Błąd odczytu sesji: {e}")))?;
    Ok(Some(session))
}

pub fn delete_session(profile_id: &str) -> Result<()> {
    let path = session_path(profile_id);
    if path.exists() {
        std::fs::remove_file(&path).map_err(LauncherError::Io)?;
    }
    // Also remove legacy plain JSON if present
    let legacy = sessions_dir().join(format!("session-{}.json", sanitize_id(profile_id)));
    if legacy.exists() {
        let _ = std::fs::remove_file(&legacy);
    }
    Ok(())
}

/// Migrate a profile array that may contain inline token objects (from old localStorage).
pub fn migrate_profiles_array(
    profiles: Vec<serde_json::Value>,
    last_profile_id: Option<String>,
) -> (Vec<serde_json::Value>, bool, Option<String>) {
    let mut changed = false;
    let mut new_last = last_profile_id.clone();

    let out: Vec<serde_json::Value> = profiles
        .into_iter()
        .map(|mut p| {
            let profile_type = p.get("type").and_then(|t| t.as_str()).unwrap_or("").to_string();
            if profile_type != "premium" {
                p.as_object_mut().map(|o| o.remove("token"));
                return p;
            }

            let token_obj = match p.get("token") {
                Some(t) if t.is_object() => t.clone(),
                _ => {
                    p.as_object_mut().map(|o| o.remove("token"));
                    return p;
                }
            };

            let uuid = token_obj
                .get("uuid")
                .and_then(|u| u.as_str())
                .map(sanitize_id)
                .unwrap_or_default();

            if uuid.is_empty() {
                p.as_object_mut().map(|o| o.remove("token"));
                return p;
            }

            // Best-effort save encrypted
            if let Ok(json) = serde_json::to_string(&token_obj) {
                let dir = sessions_dir();
                if std::fs::create_dir_all(&dir).is_ok() {
                    let encrypted = encrypt_session(json.as_bytes());
                    let path = dir.join(format!("session-{uuid}.bin"));
                    let _ = std::fs::write(path, encrypted);
                }
            }

            changed = true;

            if let Some(old_id) = p.get("id").and_then(|i| i.as_str()) {
                if let Some(ref last) = new_last {
                    if last == old_id && old_id != uuid {
                        new_last = Some(uuid.clone());
                    }
                }
            }

            p.as_object_mut().map(|o| {
                o.remove("token");
                o.insert("id".to_string(), serde_json::Value::String(uuid));
            });
            p
        })
        .collect();

    (out, changed, new_last)
}
