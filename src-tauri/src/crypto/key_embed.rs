/// Returns the mods API key baked into the binary at compile time (via build.rs → cargo:rustc-env).
/// This is the highest-priority source and requires no bundled files.
pub fn get_compile_time_mods_key() -> Option<&'static str> {
    let k = option_env!("LAUNCHER_MODS_API_KEY_EMBED").unwrap_or("");
    if k.len() >= 16 { Some(k) } else { None }
}

/// AES-256-GCM decryption of the embedded mod API key.
/// Mirrors launcherModsKeyEmbed.js exactly so the same branding/launcher-mods-key.enc
/// produced by `embed-mods-api-key.cjs` works unchanged.
///
/// Binary layout: MAGIC(6) | IV(12) | AUTH_TAG(16) | CIPHERTEXT(n)
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use scrypt::Params as ScryptParams;

const MAGIC: &[u8] = b"CCMK01";
const MIN_KEY_LEN: usize = 16;

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
    // Matches JS: crypto.scryptSync(material, salt, 32, { N:16384, r:8, p:1 })
    let salt = b"cc-lmods-embed-salt-v1\x00";
    // log2(16384) = 14; output length = 32 bytes
    let params = ScryptParams::new(14, 8, 1, 32).expect("valid scrypt params");
    let mut out = [0u8; 32];
    scrypt::scrypt(&material, salt, &params, &mut out).expect("scrypt");
    out
}

/// Decrypts a `launcher-mods-key.enc` buffer.
/// Returns `None` if the buffer is invalid, corrupted, or decrypted key is too short.
pub fn decrypt_mods_api_key(buf: &[u8]) -> Option<String> {
    // Minimum: MAGIC(6) + IV(12) + TAG(16) + at least 1 byte ciphertext
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

    // aes-gcm expects ciphertext || tag
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

/// Resolves the resource path for `launcher-mods-key.enc` or plain `launcher-mods-key`.
/// Returns the decrypted/plain key string, or None.
pub fn load_embedded_mods_api_key(resource_dir: &std::path::Path) -> Option<String> {
    // Try plain text file first (dev convenience, gitignored)
    let plain_path = resource_dir.join("launcher-mods-key");
    if plain_path.exists() {
        if let Ok(raw) = std::fs::read_to_string(&plain_path) {
            let trimmed = raw.trim().to_string();
            if trimmed.len() >= MIN_KEY_LEN {
                return Some(trimmed);
            }
        }
    }

    // Try encrypted blob
    let enc_path = resource_dir.join("launcher-mods-key.enc");
    if enc_path.exists() {
        if let Ok(buf) = std::fs::read(&enc_path) {
            return decrypt_mods_api_key(&buf);
        }
    }

    None
}
