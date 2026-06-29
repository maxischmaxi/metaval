//! Validierungs-Orchestrierung + geteilte URL-Helfer.

pub mod baseline;
pub mod hreflang;
pub mod images;
pub mod opengraph;
pub mod schema_org;
pub mod twitter;

use reqwest::Client;
use url::Url;

use crate::cli::Args;
use crate::model::{Finding, PageMetadata};

/// Führt alle Validatoren aus und sammelt die Findings.
/// Sync-Validatoren liefern Presence/Format; Bild-Erreichbarkeit läuft als
/// separater async Pass über `image_client`.
pub async fn run_all(meta: &PageMetadata, args: &Args, image_client: &Client) -> Vec<Finding> {
    let mut out = baseline::validate(meta); // immer (Minimum-Set, §6.1)
    out.extend(hreflang::validate(meta)); // Internationalisierung gehört zum Basis-Set

    if !args.min_only {
        out.extend(opengraph::validate(meta));
        out.extend(twitter::validate(meta));
        out.extend(schema_org::validate(meta));
    }

    if args.images_enabled() {
        let candidates = images::collect_candidates(meta, args.min_only);
        out.extend(images::check_all(candidates, image_client).await);
    }

    out
}

/// Normalisiert eine URL für Vergleiche (Schema/Host lowercased, Default-Port und
/// abschließenden Slash entfernt, Fragment verworfen).
pub(crate) fn normalize_url(u: &Url) -> String {
    let scheme = u.scheme().to_ascii_lowercase();
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    let mut path = u.path().trim_end_matches('/').to_string();
    if path.is_empty() {
        path = "/".to_string();
    }
    let query = u.query().map(|q| format!("?{q}")).unwrap_or_default();
    match u.port() {
        Some(p) => format!("{scheme}://{host}:{p}{path}{query}"),
        None => format!("{scheme}://{host}{path}{query}"),
    }
}

/// `true`, wenn `s` eine absolute URL ist (relative ⇒ Parse-Fehler).
pub(crate) fn is_absolute_url(s: &str) -> bool {
    Url::parse(s).is_ok()
}

#[cfg(test)]
pub mod test_support {
    use std::collections::HashMap;

    use url::Url;

    use crate::model::PageMetadata;

    /// Baut eine leere `PageMetadata` und wendet `f` darauf an.
    pub fn meta_with(f: impl FnOnce(&mut PageMetadata)) -> PageMetadata {
        let mut m = PageMetadata {
            final_url: Url::parse("https://example.com/").unwrap(),
            status: 200,
            content_type: Some("text/html".to_string()),
            x_robots_tag: None,
            title: None,
            html_lang: None,
            meta_named: HashMap::new(),
            meta_property: HashMap::new(),
            charset: None,
            links: Vec::new(),
            json_ld: Vec::new(),
            json_ld_errors: Vec::new(),
            looks_like_spa: false,
        };
        f(&mut m);
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_trailing_slash_and_default_port() {
        let a = Url::parse("https://example.com/").unwrap();
        let b = Url::parse("https://example.com:443").unwrap();
        assert_eq!(normalize_url(&a), normalize_url(&b));
    }

    #[test]
    fn absolute_detection() {
        assert!(is_absolute_url("https://example.com/x.png"));
        assert!(!is_absolute_url("/x.png"));
        assert!(!is_absolute_url("x.png"));
    }
}
