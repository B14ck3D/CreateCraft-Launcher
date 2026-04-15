use thiserror::Error;

#[derive(Debug, Error)]
pub enum LauncherError {
    #[error("Błąd I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error("Błąd HTTP: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Błąd JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Błąd sesji: {0}")]
    Session(String),

    #[error("Błąd autoryzacji: {0}")]
    Auth(String),

    #[error("Błąd Java: {0}")]
    Java(String),

    #[error("Błąd Minecraft: {0}")]
    Minecraft(String),

    #[error("Błąd archiwum: {0}")]
    Archive(String),

    #[error("Błąd kryptograficzny: {0}")]
    Crypto(String),

    #[error("{0}")]
    Other(String),
}

// Tauri commands must return serde-serializable errors.
impl serde::Serialize for LauncherError {
    fn serialize<S: serde::Serializer>(
        &self,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, LauncherError>;

impl From<anyhow::Error> for LauncherError {
    fn from(e: anyhow::Error) -> Self {
        LauncherError::Other(e.to_string())
    }
}

