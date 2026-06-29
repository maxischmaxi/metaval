//! HTTP-Fetcher auf Basis von reqwest. Folgt Redirects, meldet finale URL/Status.

use std::time::Duration;

use reqwest::{Client, header, redirect};
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
        let client = Client::builder()
            .user_agent(args.effective_user_agent())
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
        let x_robots_tag = resp
            .headers()
            .get("x-robots-tag")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);

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
