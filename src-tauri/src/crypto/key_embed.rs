use std::path::Path;

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use scrypt::Params as ScryptParams;

const MAGIC: &[u8] = b"CCMK01";
const MIN_KEY_LEN: usize = 16;

/// Kolejność: `LAUNCHER_MODS_API_KEY` (nadpisanie) → `launcher-mods-key.enc` → jawny `launcher-mods-key` (tylko dev).
/// Brak wbudowanego klucza w binarce — jedna ścieżka dystrybucji: plik `.enc` w zasobach (patrz tauri.conf → resources).
pub fn resolve_mods_api_key(resource_dir: &Path) -> Option<String> {
    if let Ok(k) = std::env::var("LAUNCHER_MODS_API_KEY") {
        let t = k.trim().to_string();
        if t.len() >= MIN_KEY_LEN {
            return Some(t);
        }
    }
    load_embedded_mods_api_key(resource_dir)
}

fn embed_material() -> Vec<u8> {
    let mut m = Vec::new();
    m.extend_from_slice(
        &hex::decode("7c9f2e41b8d304a6e51f0c2d8a7b4930").expect("static hex"),
    );
    m.extend_from_slice(b"pl.createcrafts.launcher.mods.embed");
    m.extend_from_slice(
        &hex::decode("b3e8916c4f2a0d5e8c1b7a9d6e0f4c2a").expect("static hex"),
    );
    m
}

fn derive_embed_key() -> [u8; 32] {
    let material = embed_material();
    let salt = b"cc-lmods-embed-salt-v1\x00";
    let params = ScryptParams::new(14, 8, 1, 32).expect("valid scrypt params");
    let mut out = [0u8; 32];
    scrypt::scrypt(&material, salt, &params, &mut out).expect("scrypt");
    out
}

pub fn decrypt_mods_api_key(buf: &[u8]) -> Option<String> {
    if buf.len() < MAGIC.len() + 12 + 16 + 1 {
        return None;
    }
    if !buf.starts_with(MAGIC) {
        return None;
    }

    let pos = MAGIC.len();
    let iv = &buf[pos..pos + 12];
    let tag = &buf[pos + 12..pos + 28];
    let ciphertext = &buf[pos + 28..];

    let mut ct_with_tag = ciphertext.to_vec();
    ct_with_tag.extend_from_slice(tag);

    let key_bytes = derive_embed_key();
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(iv);

    let plaintext = cipher.decrypt(nonce, ct_with_tag.as_slice()).ok()?;
    let s = String::from_utf8(plaintext).ok()?;
    let trimmed = s.trim().to_string();
    if trimmed.len() >= MIN_KEY_LEN {
        Some(trimmed)
    } else {
        None
    }
}

pub fn load_embedded_mods_api_key(resource_dir: &Path) -> Option<String> {
    let enc_path = resource_dir.join("launcher-mods-key.enc");
    if enc_path.exists() {
        if let Ok(buf) = std::fs::read(&enc_path) {
            if let Some(k) = decrypt_mods_api_key(&buf) {
                return Some(k);
            }
        }
    }

    let plain_path = resource_dir.join("launcher-mods-key");
    if plain_path.exists() {
        if let Ok(raw) = std::fs::read_to_string(&plain_path) {
            let trimmed = raw.trim().to_string();
            if trimmed.len() >= MIN_KEY_LEN {
                return Some(trimmed);
            }
        }
    }

    None
}
