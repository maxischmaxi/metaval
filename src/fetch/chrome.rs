//! Headless-Chrome-Fetcher (chromiumoxide / CDP). Rendert JS-injizierte Metadaten.
//!
//! Status, finale URL und MIME-Typ stammen direkt aus
//! `page.wait_for_navigation_response()` — kein manuelles `Network.responseReceived`-Wiring.

use std::time::Duration;

use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use url::Url;

use crate::cli::Args;
use crate::error::FetchError;

use super::{FetchedPage, PageFetcher};

/// Grace-Delay nach `wait_for_navigation`, damit SPA-injizierte Metadaten landen
/// (chromiumoxide 0.9 hat keinen dedizierten Network-Idle-Call).
const RENDER_GRACE: Duration = Duration::from_millis(500);

/// Fetcher, der eine Seite mit System-Chrome rendert. Baut den Browser erst beim
/// `fetch()`, damit Start-/Autodetect-Fehler sauber als `FetchError` durchschlagen.
pub struct ChromeFetcher {
    chrome_path: Option<String>,
    timeout: u64,
    insecure: bool,
}

impl ChromeFetcher {
    pub fn new(args: &Args) -> Self {
        Self {
            chrome_path: args.chrome_path.clone(),
            timeout: args.timeout,
            insecure: args.insecure,
        }
    }

    fn build_config(&self) -> Result<BrowserConfig, FetchError> {
        let mut builder = BrowserConfig::builder()
            .no_sandbox()
            .arg("--disable-dev-shm-usage")
            .request_timeout(Duration::from_secs(self.timeout));

        if let Some(path) = &self.chrome_path {
            builder = builder.chrome_executable(path);
        }
        if self.insecure {
            builder = builder.arg("--ignore-certificate-errors");
        }

        // build() liefert Err(String); ein Autodetect-Miss (kein Chrome gefunden) landet hier.
        builder.build().map_err(FetchError::ChromeLaunch)
    }
}

impl PageFetcher for ChromeFetcher {
    async fn fetch(&self, url: &Url) -> Result<FetchedPage, FetchError> {
        let config = self.build_config()?;

        // Ein ungültiger expliziter --chrome-path schlägt erst hier (beim Spawn) fehl.
        let (mut browser, mut handler) = Browser::launch(config)
            .await
            .map_err(|e| FetchError::ChromeLaunch(e.to_string()))?;

        // Handler MUSS gepollt werden, sonst blockiert jede Page-Operation.
        let handler_task = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if event.is_err() {
                    break;
                }
            }
        });

        // Eigentliche Navigation; Ergebnis sammeln, Fehler aber erst nach Shutdown werfen.
        // Hartes Gesamt-Timeout als Sicherheitsnetz, falls Chrome/CDP doch hängt
        // (request_timeout greift nicht für jeden Hänger).
        let hard_limit = Duration::from_secs(self.timeout.saturating_add(10));
        let result = match tokio::time::timeout(hard_limit, navigate(&browser, url)).await {
            Ok(r) => r,
            Err(_) => Err(FetchError::Timeout(self.timeout)),
        };

        // Sauberer Shutdown: close() bittet Chrome zu beenden, wait() reaped den Prozess.
        let _ = browser.close().await;
        let _ = browser.wait().await;
        let _ = handler_task.await;

        result
    }
}

/// Führt Navigation + HTML-Extraktion innerhalb einer laufenden Browser-Session aus.
async fn navigate(browser: &Browser, url: &Url) -> Result<FetchedPage, FetchError> {
    let page = browser
        .new_page("about:blank")
        .await
        .map_err(|e| FetchError::Chrome(e.to_string()))?;

    page.goto(url.as_str())
        .await
        .map_err(|e| FetchError::Chrome(e.to_string()))?;

    // `wait_for_navigation_response` liefert `Option<Arc<HttpRequest>>` (ArcHttpRequest).
    let nav = page
        .wait_for_navigation_response()
        .await
        .map_err(|e| FetchError::Chrome(e.to_string()))?;

    // Status/URL/MIME aus der Hauptnavigations-Response. None (Cache/Subframe) =
    // "unbekannt" → defensiv als gerendert (200) werten, statt zu scheitern.
    let response = nav.as_ref().and_then(|req| req.response.as_ref());
    let status = response.map(|r| r.status as u16).unwrap_or(200);
    let content_type = response.map(|r| r.mime_type.clone());
    let final_url = response
        .and_then(|r| Url::parse(&r.url).ok())
        .unwrap_or_else(|| url.clone());

    // Grace, damit JS-injizierte Metadaten im DOM sind, dann gerendertes HTML lesen.
    tokio::time::sleep(RENDER_GRACE).await;
    let html = page
        .content()
        .await
        .map_err(|e| FetchError::Chrome(e.to_string()))?;

    Ok(FetchedPage {
        requested_url: url.clone(),
        final_url,
        status,
        content_type,
        // Response-Header sind über CDP zwar verfügbar, werden hier aber bewusst
        // nicht ausgewertet; die X-Robots-Tag-Prüfung deckt der HTTP-Pfad ab.
        x_robots_tag: None,
        html,
    })
}
