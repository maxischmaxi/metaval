//! Fehlertypen. Alle `FetchError`-Varianten führen zu Exit-Code 2 (Tool-/Fetch-Problem).
//! Inhaltliche Validierungs-Findings sind hingegen kein Fehler, sondern Daten.

use thiserror::Error;

/// Fehler beim Abruf der Seite (Netzwerk/Chrome). Stets Exit-Code 2.
#[derive(Debug, Error)]
pub enum FetchError {
    #[error("ungültige URL: {0}")]
    InvalidUrl(String),

    #[error("Host nicht erreichbar: {0}")]
    Connect(String),

    #[error("Request-Timeout nach {0}s überschritten")]
    Timeout(u64),

    #[error("TLS-Fehler (ggf. mit --insecure erneut versuchen): {0}")]
    Tls(String),

    #[error("HTTP-Transportfehler: {0}")]
    Transport(#[source] reqwest::Error),

    #[error("Chrome konnte nicht gestartet werden (--chrome-path setzen oder Chrome installieren): {0}")]
    ChromeLaunch(String),

    #[error("Chrome-Navigationsfehler: {0}")]
    Chrome(String),
}

impl FetchError {
    /// Klassifiziert einen `reqwest::Error` in die passende Variante.
    pub fn classify(err: reqwest::Error, timeout: u64) -> Self {
        if err.is_timeout() {
            Self::Timeout(timeout)
        } else if is_tls_error(&err) {
            Self::Tls(err.to_string())
        } else if err.is_connect() {
            Self::Connect(err.to_string())
        } else {
            Self::Transport(err)
        }
    }
}

/// Heuristik: durchläuft die Fehler-Quellenkette nach TLS-/Zertifikat-Hinweisen.
fn is_tls_error(err: &reqwest::Error) -> bool {
    let mut source: Option<&dyn std::error::Error> = Some(err);
    while let Some(e) = source {
        let msg = e.to_string().to_ascii_lowercase();
        if msg.contains("certificate") || msg.contains("tls") || msg.contains("handshake") {
            return true;
        }
        source = e.source();
    }
    false
}

/// App-übergreifender Fehler. Jede Variante ⇒ Exit-Code 2.
#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Fetch(#[from] FetchError),

    #[error("{0}")]
    Other(String),
}
