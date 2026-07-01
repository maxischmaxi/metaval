//! HTTP-Fetcher auf Basis von reqwest. Folgt Redirects, meldet finale URL/Status.

use std::time::Duration;

use reqwest::header::{self, HeaderMap, HeaderValue};
use reqwest::{Client, redirect};
use url::Url;

use crate::cli::Args;
use crate::error::FetchError;

use super::{FetchedPage, PageFetcher};

/// Fetcher für reinen HTTP-GET. Hält einen wiederverwendbaren `reqwest::Client`.
pub struct HttpFetcher {
    client: Client,
    timeout: u64,
}

impl HttpFetcher {
    pub fn new(args: &Args) -> Result<Self, FetchError> {
        // Browserüblicher Accept-Header: manche Server content-negotiaten und
        // liefern auf reqwests Default (*/*) sonst JSON o. Ä. statt HTML.
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ACCEPT,
            HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
        );
        let client = Client::builder()
            .user_agent(args.effective_user_agent())
            .default_headers(headers)
            .timeout(Duration::from_secs(args.timeout))
            .redirect(redirect::Policy::limited(10))
            .danger_accept_invalid_certs(args.insecure)
            .build()
            .map_err(FetchError::Transport)?;
        Ok(Self {
            client,
            timeout: args.timeout,
        })
    }
}

impl PageFetcher for HttpFetcher {
    async fn fetch(&self, url: &Url) -> Result<FetchedPage, FetchError> {
        let resp = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|e| FetchError::classify(e, self.timeout))?;

        let final_url = resp.url().clone();
        let status = resp.status().as_u16();
        let content_type = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        // Der Header darf mehrfach vorkommen (z. B. `noindex` und `nofollow`
        // getrennt) — alle Instanzen einsammeln, nicht nur die erste.
        let x_robots_values: Vec<&str> = resp
            .headers()
            .get_all("x-robots-tag")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .collect();
        let x_robots_tag =
            (!x_robots_values.is_empty()).then(|| x_robots_values.join(", "));

        // 4xx/5xx sind kein harter Abbruch: Body wird (falls vorhanden) trotzdem
        // geparst, der Status später als Finding gemeldet.
        let html = resp
            .text()
            .await
            .map_err(|e| FetchError::classify(e, self.timeout))?;

        Ok(FetchedPage {
            requested_url: url.clone(),
            final_url,
            status,
            content_type,
            x_robots_tag,
            html,
        })
    }
}
