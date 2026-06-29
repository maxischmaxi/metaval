//! Fetch-Layer: gemeinsamer Vertrag + Laufzeit-Auswahl HTTP vs. Chrome.

pub mod chrome;
pub mod http;

use url::Url;

use crate::cli::Args;
use crate::error::FetchError;

pub use chrome::ChromeFetcher;
pub use http::HttpFetcher;

/// Ergebnis eines Seitenabrufs (nach Redirects).
#[derive(Clone, Debug)]
pub struct FetchedPage {
    pub requested_url: Url,
    pub final_url: Url,
    pub status: u16,
    pub content_type: Option<String>,
    /// `X-Robots-Tag`-Header (HTTP-Pfad; beim Chrome-Pfad i. d. R. `None`).
    pub x_robots_tag: Option<String>,
    pub html: String,
}

/// Gemeinsamer Vertrag aller Fetcher. `async fn` im Trait ist auf Edition 2024
/// stabil; Laufzeit-Dispatch läuft über das `Fetcher`-Enum (nicht `dyn`).
pub trait PageFetcher {
    async fn fetch(&self, url: &Url) -> Result<FetchedPage, FetchError>;
}

/// Laufzeit-Auswahl zwischen HTTP- und Chrome-Fetcher.
pub enum Fetcher {
    Http(HttpFetcher),
    Chrome(ChromeFetcher),
}

impl Fetcher {
    /// Baut den passenden Fetcher anhand der CLI-Argumente.
    pub fn from_args(args: &Args) -> Result<Self, FetchError> {
        if args.render {
            Ok(Self::Chrome(ChromeFetcher::new(args)))
        } else {
            Ok(Self::Http(HttpFetcher::new(args)?))
        }
    }

    pub async fn fetch(&self, url: &Url) -> Result<FetchedPage, FetchError> {
        match self {
            Self::Http(f) => f.fetch(url).await,
            Self::Chrome(f) => f.fetch(url).await,
        }
    }
}
