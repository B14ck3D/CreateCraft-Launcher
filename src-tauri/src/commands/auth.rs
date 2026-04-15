/// Microsoft OAuth + Minecraft auth commands.
/// Replaces msmc npm package + profileStore.js + login-microsoft / profiles-* ipcMain handlers.
///
/// Flow:
///   1. Open Tauri WebviewWindow with MS OAuth URL
///   2. Intercept navigation to get auth code
///   3. Exchange auth code → MS token → Xbox → XSTS → Minecraft token
///   4. Fetch Minecraft profile (UUID, name)
///   5. Save session via keyring
use crate::error::{LauncherError, Result};
use crate::session::store::{delete_session, load_session, save_session, PremiumSession};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::Manager;

// ---------------------------------------------------------------------------
// Returned profile type (matches what App.jsx expects)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileResult {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub label: String,
    pub avatar: String,
}

// ---------------------------------------------------------------------------
// MS OAuth constants
// This uses the public Xbox Live / Minecraft launcher client ID (same as MCLC/msmc).
// ---------------------------------------------------------------------------

const MS_CLIENT_ID: &str = "00000000402b5328";
const MS_REDIRECT_URI: &str = "https://login.live.com/oauth20_desktop.srf";

fn ms_auth_url() -> String {
    format!(
        "https://login.live.com/oauth20_authorize.srf\
?client_id={MS_CLIENT_ID}\
&response_type=code\
&scope=XboxLive.signin%20offline_access\
&redirect_uri={}\
&display=touch\
&prompt=select_account",
        urlencoding::encode(MS_REDIRECT_URI)
    )
}

fn extract_auth_code(url: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    u.query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.to_string())
}

// ---------------------------------------------------------------------------
// Token exchange helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct MsTokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

#[derive(Debug, Deserialize)]
struct XblResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: XblDisplayClaims,
}

#[derive(Debug, Deserialize)]
struct XblDisplayClaims {
    xui: Vec<XblXui>,
}

#[derive(Debug, Deserialize)]
struct XblXui {
    uhs: String,
    #[serde(default)]
    xid: Option<String>,
}

#[derive(Debug, Deserialize)]
struct McTokenResponse {
    access_token: String,
    expires_in: i64,
}

#[derive(Debug, Deserialize)]
struct McProfileResponse {
    id: String,
    name: String,
}

fn build_reqwest_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("CreateCrafts-Launcher/2 (Bl4ck3d)")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(LauncherError::Http)
}

async fn exchange_code_for_ms_token(
    client: &reqwest::Client,
    code: &str,
) -> Result<MsTokenResponse> {
    let params = [
        ("client_id", MS_CLIENT_ID),
        ("code", code),
        ("grant_type", "authorization_code"),
        ("redirect_uri", MS_REDIRECT_URI),
    ];
    let resp = client
        .post("https://login.live.com/oauth20_token.srf")
        .form(&params)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(LauncherError::Auth(format!(
            "Błąd MS token: HTTP {}",
            resp.status()
        )));
    }
    Ok(resp.json::<MsTokenResponse>().await?)
}

async fn refresh_ms_token(
    client: &reqwest::Client,
    refresh_token: &str,
) -> Result<MsTokenResponse> {
    let params = [
        ("client_id", MS_CLIENT_ID),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
        ("redirect_uri", MS_REDIRECT_URI),
    ];
    let resp = client
        .post("https://login.live.com/oauth20_token.srf")
        .form(&params)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(LauncherError::Auth(format!(
            "Błąd odświeżania tokenu MS: HTTP {}",
            resp.status()
        )));
    }
    Ok(resp.json::<MsTokenResponse>().await?)
}

async fn get_xbox_token(
    client: &reqwest::Client,
    ms_access_token: &str,
) -> Result<(String, String)> {
    let body = serde_json::json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={ms_access_token}")
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });
    let resp = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(LauncherError::Auth(format!(
            "Błąd XBL: HTTP {}",
            resp.status()
        )));
    }
    let xbl: XblResponse = resp.json().await?;
    let uhs = xbl
        .display_claims
        .xui
        .first()
        .map(|x| x.uhs.clone())
        .unwrap_or_default();
    Ok((xbl.token, uhs))
}

async fn get_xsts_token(
    client: &reqwest::Client,
    xbox_token: &str,
) -> Result<(String, String, Option<String>)> {
    let body = serde_json::json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbox_token]
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    });
    let resp = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&body)
        .send()
        .await?;
    if resp.status().as_u16() == 401 {
        return Err(LauncherError::Auth(
            "Konto Xbox nie ma licencji Minecraft lub wymaga weryfikacji przez Xbox.".into(),
        ));
    }
    if !resp.status().is_success() {
        return Err(LauncherError::Auth(format!(
            "Błąd XSTS: HTTP {}",
            resp.status()
        )));
    }
    let xsts: XblResponse = resp.json().await?;
    let uhs = xsts
        .display_claims
        .xui
        .first()
        .map(|x| x.uhs.clone())
        .unwrap_or_default();
    let xuid = xsts
        .display_claims
        .xui
        .first()
        .and_then(|x| x.xid.clone());
    Ok((xsts.token, uhs, xuid))
}

async fn get_minecraft_token(
    client: &reqwest::Client,
    xsts_token: &str,
    uhs: &str,
) -> Result<(String, i64)> {
    let body = serde_json::json!({
        "identityToken": format!("XBL3.0 x={uhs};{xsts_token}")
    });
    let resp = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(LauncherError::Auth(format!(
            "Błąd tokenu Minecraft: HTTP {}",
            resp.status()
        )));
    }
    let mc: McTokenResponse = resp.json().await?;
    Ok((mc.access_token, mc.expires_in))
}

async fn get_minecraft_profile(
    client: &reqwest::Client,
    mc_access_token: &str,
) -> Result<McProfileResponse> {
    let resp = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(mc_access_token)
        .send()
        .await?;
    if resp.status().as_u16() == 404 {
        return Err(LauncherError::Auth(
            "To konto nie posiada zakupionego Minecraft Java Edition.".into(),
        ));
    }
    if !resp.status().is_success() {
        return Err(LauncherError::Auth(format!(
            "Błąd profilu MC: HTTP {}",
            resp.status()
        )));
    }
    Ok(resp.json::<McProfileResponse>().await?)
}

fn mineatar_url(uuid: &str) -> String {
    // Normalize UUID (strip dashes, re-add)
    let hex: String = uuid.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() == 32 {
        let canonical = format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        );
        format!("https://api.mineatar.io/face/{canonical}?scale=4")
    } else {
        String::new()
    }
}

// ---------------------------------------------------------------------------
// Full MS auth flow: open window → get code → exchange → profile
// ---------------------------------------------------------------------------

async fn do_ms_auth(app: &tauri::AppHandle) -> Result<PremiumSession> {
    let auth_url = ms_auth_url();
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    let tx = Arc::new(Mutex::new(Some(tx)));

    let tx_clone = tx.clone();
    let auth_window = tauri::WebviewWindowBuilder::new(
        app,
        "auth",
        tauri::WebviewUrl::External(auth_url.parse().map_err(|e| {
            LauncherError::Auth(format!("Błąd URL autoryzacji: {e}"))
        })?),
    )
    .title("Logowanie Microsoft — CreateCrafts")
    .inner_size(480.0, 680.0)
    .resizable(false)
    .on_navigation(move |url| {
        let url_str = url.to_string();
        if url_str.contains("login.live.com/oauth20_desktop.srf") {
            if let Some(code) = extract_auth_code(&url_str) {
                if let Ok(mut guard) = tx_clone.lock() {
                    if let Some(sender) = guard.take() {
                        let _ = sender.send(code);
                    }
                }
            }
        }
        true
    })
    .build()
    .map_err(|e| LauncherError::Auth(format!("Nie można otworzyć okna logowania: {e}")))?;

    let code = rx
        .await
        .map_err(|_| LauncherError::Auth("Logowanie anulowane przez użytkownika.".into()))?;

    let _ = auth_window.close();

    let http = build_reqwest_client()?;
    let ms_token = exchange_code_for_ms_token(&http, &code).await?;
    let (xbox_token, _uhs) = get_xbox_token(&http, &ms_token.access_token).await?;
    let (xsts_token, uhs, xuid) = get_xsts_token(&http, &xbox_token).await?;
    let (mc_token, mc_expires_in) = get_minecraft_token(&http, &xsts_token, &uhs).await?;
    let profile = get_minecraft_profile(&http, &mc_token).await?;

    let expires_at = chrono::Utc::now().timestamp() + mc_expires_in;
    let session = PremiumSession {
        uuid: format_uuid(&profile.id),
        name: profile.name,
        access_token: mc_token,
        refresh_token: ms_token.refresh_token,
        expires_at,
        xuid,
        client_token: None,
    };

    Ok(session)
}

fn format_uuid(hex_id: &str) -> String {
    // Mojang returns UUID without dashes; normalise.
    let h: String = hex_id.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if h.len() == 32 {
        format!(
            "{}-{}-{}-{}-{}",
            &h[0..8],
            &h[8..12],
            &h[12..16],
            &h[16..20],
            &h[20..32]
        )
    } else {
        hex_id.to_string()
    }
}

/// Validates + refreshes the session if the access token has expired.
pub async fn ensure_session_valid(session: &mut PremiumSession) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    // Refresh if token expires within 5 minutes
    if session.expires_at - now > 300 {
        return Ok(());
    }

    let http = build_reqwest_client()?;
    let ms_token = refresh_ms_token(&http, &session.refresh_token).await?;

    let (xbox_token, _) = get_xbox_token(&http, &ms_token.access_token).await?;
    let (xsts_token, uhs, xuid) = get_xsts_token(&http, &xbox_token).await?;
    let (mc_token, mc_expires_in) = get_minecraft_token(&http, &xsts_token, &uhs).await?;

    session.access_token = mc_token;
    session.refresh_token = ms_token.refresh_token;
    session.expires_at = now + mc_expires_in;
    if xuid.is_some() {
        session.xuid = xuid;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Opens the Microsoft login window, exchanges tokens, saves session, returns profile.
#[tauri::command]
pub async fn login_microsoft(app: tauri::AppHandle) -> std::result::Result<ProfileResult, String> {
    let session = do_ms_auth(&app).await.map_err(|e| e.to_string())?;
    save_session(&session).map_err(|e| e.to_string())?;
    let avatar = mineatar_url(&session.uuid);
    Ok(ProfileResult {
        id: session.uuid.clone(),
        name: session.name.clone(),
        r#type: "premium".to_string(),
        label: session.name.clone(),
        avatar,
    })
}

/// Deletes the stored session for a profile.
#[tauri::command]
pub async fn delete_premium_session(
    profile_id: String,
) -> std::result::Result<serde_json::Value, String> {
    delete_session(&profile_id).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "ok": true }))
}

/// Migrates inline token objects from the old localStorage profile array to keyring.
#[tauri::command]
pub async fn migrate_profiles_from_localstorage(
    raw_json: Option<String>,
    last_profile_id: Option<String>,
) -> std::result::Result<serde_json::Value, String> {
    let raw = match raw_json {
        Some(r) => r,
        None => return Ok(serde_json::json!({ "ok": true, "changed": false })),
    };
    let profiles: Vec<serde_json::Value> = serde_json::from_str(&raw)
        .map_err(|e| format!("Błąd parsowania profili: {e}"))?;

    let (out, changed, new_last_id) =
        crate::session::store::migrate_profiles_array(profiles, last_profile_id);

    Ok(serde_json::json!({
        "ok": true,
        "profilesJson": serde_json::to_string(&out).unwrap_or_default(),
        "changed": changed,
        "newLastProfileId": new_last_id,
    }))
}

/// Returns a Mineatar face URL + normalised UUID for a player name or UUID.
#[tauri::command]
pub async fn mineatar_face_url(
    offline_name: Option<String>,
    uuid: Option<String>,
) -> std::result::Result<serde_json::Value, String> {
    let canonical = if let Some(u) = uuid.filter(|s| !s.is_empty()) {
        let hex: String = u.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        if hex.len() == 32 {
            Some(format!(
                "{}-{}-{}-{}-{}",
                &hex[0..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..32]
            ))
        } else {
            None
        }
    } else if let Some(name) = offline_name.filter(|s| !s.is_empty()) {
        Some(offline_uuid(&name))
    } else {
        None
    };

    match canonical {
        Some(u) => Ok(serde_json::json!({
            "url": mineatar_url(&u),
            "playerUuid": u,
        })),
        None => Ok(serde_json::json!({ "url": null, "playerUuid": null })),
    }
}

fn offline_uuid(player_name: &str) -> String {
    use md5::Digest;
    let input = format!("OfflinePlayer:{player_name}");
    let digest = md5::Md5::digest(input.as_bytes());
    let mut b = digest.to_vec();
    b[6] = (b[6] & 0x0f) | 0x30;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = hex::encode(&b);
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}
