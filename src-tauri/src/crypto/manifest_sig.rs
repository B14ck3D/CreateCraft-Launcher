/// HMAC-SHA256 mod manifest signature verification.
/// Mirrors launcherModsManifestSignature.js exactly.
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, serde::Deserialize)]
pub struct ModEntry {
    pub name: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct ModManifest {
    pub v: serde_json::Value,
    pub generated: Option<String>,
    pub count: u64,
    pub signature: Option<String>,
    pub mods: Vec<ModEntry>,
}

/// Builds the canonical payload string that must be HMAC'd.
fn build_signature_payload(manifest: &ModManifest) -> crate::Result<String> {
    let generated = manifest.generated.clone().unwrap_or_default();
    let count_str = manifest.count.to_string();

    let mut lines: Vec<String> = manifest
        .mods
        .iter()
        .map(|m| {
            let sha256 = m.sha256.to_lowercase();
            format!("{}\t{}\t{}", m.name, m.size, sha256)
        })
        .collect();

    lines.sort_by(|a, b| a.as_str().cmp(b.as_str()));

    let mut parts = vec!["v1".to_string(), generated, count_str];
    parts.extend(lines);
    Ok(parts.join("\n"))
}

/// Returns `true` when the manifest's `signature` field matches HMAC-SHA256 of the
/// canonical payload using `secret` as the key.
pub fn verify_manifest_signature(manifest: &ModManifest, secret: &str) -> bool {
    let sig = match &manifest.signature {
        Some(s) => s.to_lowercase(),
        None => return false,
    };
    if sig.len() != 64 || !sig.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }

    let payload = match build_signature_payload(manifest) {
        Ok(p) => p,
        Err(_) => return false,
    };

    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    let computed = hex::encode(mac.finalize().into_bytes());

    // Constant-time comparison via hmac's verify_slice is not directly available
    // for hex strings, so compare bytes after hex-decode.
    let computed_bytes = match hex::decode(&computed) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let sig_bytes = match hex::decode(&sig) {
        Ok(b) => b,
        Err(_) => return false,
    };
    use subtle::ConstantTimeEq;
    computed_bytes.ct_eq(&sig_bytes).into()
}
