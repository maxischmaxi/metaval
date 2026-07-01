//! Bild-Erreichbarkeit. Kandidaten sammeln (synchron, rein),
//! dann nebenläufig prüfen (max. 8 parallel via `futures::buffer_unordered`).
//! Findings tragen die Ursprungs-Kategorie/Regel, damit sie im Report dort landen.

use std::collections::{HashMap, HashSet};

use futures::stream::{self, StreamExt};
use reqwest::{Client, StatusCode, header::RANGE};
use serde_json::Value;
use url::Url;

use crate::model::{Category, Finding, PageMetadata, Severity};

/// Maximale parallele Bild-Requests.
const MAX_PARALLEL: usize = 8;

/// Ein zu prüfendes Bild inkl. Ursprung (für Kategorie/Regel des Findings).
#[derive(Clone, Debug)]
pub struct ImageCandidate {
    pub url: Url,
    pub category: Category,
    pub rule: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
enum Reachability {
    Ok,
    WrongType(String),
    BadStatus(u16),
    NetworkError(String),
}

/// Sammelt und löst alle Bild-Kandidaten auf (relativ → absolut gegen `final_url`).
pub fn collect_candidates(meta: &PageMetadata, min_only: bool) -> Vec<ImageCandidate> {
    let mut out = Vec::new();
    let base = &meta.final_url;

    let push = |raw: &str, category: Category, rule: &'static str, out: &mut Vec<ImageCandidate>| {
        if let Ok(url) = base.join(raw.trim()) {
            // Nur netzwerk-prüfbare Schemata; data:/mailto: o. ä. sind inline → überspringen.
            if matches!(url.scheme(), "http" | "https") {
                out.push(ImageCandidate { url, category, rule });
            }
        }
    };

    if !min_only {
        for img in meta.og_all("og:image") {
            push(img, Category::OpenGraph, "og.image.reachable", &mut out);
        }
        for img in meta.og_all("og:image:secure_url") {
            push(img, Category::OpenGraph, "og.image.reachable", &mut out);
        }
        for img in meta.twitter_all("twitter:image") {
            push(img, Category::Twitter, "tw.image.reachable", &mut out);
        }
        let mut ld_images = Vec::new();
        for v in &meta.json_ld {
            harvest_ld_images(v, &mut ld_images);
        }
        for img in ld_images {
            push(&img, Category::SchemaOrg, "ld.image.reachable", &mut out);
        }
    }

    // Favicons / Apple-Touch-Icons immer prüfen.
    for link in &meta.links {
        if link.rel.to_ascii_lowercase().contains("icon") {
            push(&link.href, Category::Images, "icon.reachable", &mut out);
        }
    }

    out
}

/// Prüft alle Kandidaten nebenläufig und erzeugt je Kandidat ein Finding.
pub async fn check_all(candidates: Vec<ImageCandidate>, client: &Client) -> Vec<Finding> {
    if candidates.is_empty() {
        return Vec::new();
    }

    // Jede eindeutige URL nur einmal abrufen.
    let mut seen = HashSet::new();
    let unique: Vec<Url> = candidates
        .iter()
        .filter(|c| seen.insert(c.url.clone()))
        .map(|c| c.url.clone())
        .collect();

    let results: HashMap<Url, Reachability> = stream::iter(unique)
        .map(|u| async move {
            let r = check_one(&u, client).await;
            (u, r)
        })
        .buffer_unordered(MAX_PARALLEL)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect();

    candidates
        .into_iter()
        .map(|c| {
            let reach = results.get(&c.url).cloned().unwrap_or(Reachability::Ok);
            finding_for(&c, &reach)
        })
        .collect()
}

async fn check_one(url: &Url, client: &Client) -> Reachability {
    // 1) HEAD — billig, aber viele Server/CDNs beantworten HEAD falsch
    //    (403/404/405 trotz funktionierendem GET). Deshalb ist *jede*
    //    Nicht-Erfolgs-Antwort nur ein Grund für den GET-Fallback, kein Urteil.
    if let Ok(r) = client.head(url.clone()).send().await {
        let status = r.status();
        if status.is_success()
            && let Some(ct) = content_type(&r)
        {
            return classify(status, Some(ct));
        }
        // Nicht-2xx oder 2xx ohne Content-Type → Fallback.
    }

    // 2) Fallback: GET mit Range (nur erstes Byte). Dessen Antwort zählt.
    match client.get(url.clone()).header(RANGE, "bytes=0-0").send().await {
        Ok(r) => classify(r.status(), content_type(&r)),
        Err(e) => Reachability::NetworkError(e.to_string()),
    }
}

fn classify(status: StatusCode, ct: Option<String>) -> Reachability {
    if !status.is_success() {
        return Reachability::BadStatus(status.as_u16());
    }
    match ct {
        Some(c) if c.trim().to_ascii_lowercase().starts_with("image/") => Reachability::Ok,
        Some(c) => Reachability::WrongType(c),
        None => Reachability::Ok,
    }
}

fn content_type(r: &reqwest::Response) -> Option<String> {
    r.headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}

fn finding_for(c: &ImageCandidate, reach: &Reachability) -> Finding {
    let url = c.url.as_str().to_string();
    match reach {
        Reachability::Ok => {
            Finding::new(c.category, Severity::Pass, c.rule, "Image reachable").with_detail(url)
        }
        Reachability::WrongType(ct) => Finding::new(
            c.category,
            Severity::Warning,
            c.rule,
            "Image reachable, but Content-Type is not image/*",
        )
        .with_detail(format!("{ct} — {url}")),
        Reachability::BadStatus(s) => Finding::new(
            c.category,
            Severity::Error,
            c.rule,
            format!("Image not reachable (status {s})"),
        )
        .with_detail(url),
        Reachability::NetworkError(e) => {
            Finding::new(c.category, Severity::Error, c.rule, "Image not reachable")
                .with_detail(format!("{e} — {url}"))
        }
    }
}

/// Sammelt Bild-URLs aus JSON-LD-Feldern `image`/`logo`/`thumbnailUrl` (rekursiv).
fn harvest_ld_images(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Array(a) => a.iter().for_each(|x| harvest_ld_images(x, out)),
        Value::Object(o) => {
            for key in ["image", "logo", "thumbnailUrl"] {
                if let Some(val) = o.get(key) {
                    collect_image_urls(val, out);
                }
            }
            for val in o.values() {
                harvest_ld_images(val, out);
            }
        }
        _ => {}
    }
}

fn collect_image_urls(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.clone()),
        Value::Array(a) => a.iter().for_each(|x| collect_image_urls(x, out)),
        Value::Object(o) => {
            if let Some(Value::String(u)) = o.get("url") {
                out.push(u.clone());
            } else if let Some(Value::String(u)) = o.get("@id") {
                out.push(u.clone());
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::test_support::meta_with;

    #[test]
    fn resolves_relative_and_dedupes_by_url() {
        let m = meta_with(|m| {
            m.meta_property.insert(
                "og:image".to_string(),
                vec!["/img/a.png".to_string(), "https://example.com/img/a.png".to_string()],
            );
            m.meta_named
                .insert("twitter:image".to_string(), vec!["https://cdn.example.com/b.png".to_string()]);
        });
        let cands = collect_candidates(&m, false);
        // Beide og:image lösen zur selben absoluten URL auf.
        let abs: Vec<_> = cands.iter().map(|c| c.url.as_str()).collect();
        assert!(abs.contains(&"https://example.com/img/a.png"));
        assert!(abs.contains(&"https://cdn.example.com/b.png"));
        // Kandidaten bleiben (3), Dedup erst beim Netz-Fetch.
        assert_eq!(cands.len(), 3);
    }

    #[test]
    fn harvest_picks_up_jsonld_logo() {
        let m = meta_with(|m| {
            m.json_ld.push(serde_json::json!({
                "@type":"Organization","name":"X",
                "logo":{"@type":"ImageObject","url":"https://example.com/logo.png"}
            }));
        });
        let cands = collect_candidates(&m, false);
        assert!(cands.iter().any(|c| c.url.as_str() == "https://example.com/logo.png"
            && c.rule == "ld.image.reachable"));
    }

    #[test]
    fn classify_rules() {
        assert_eq!(
            classify(StatusCode::OK, Some("image/png".to_string())),
            Reachability::Ok
        );
        assert_eq!(
            classify(StatusCode::OK, Some("text/html".to_string())),
            Reachability::WrongType("text/html".to_string())
        );
        assert_eq!(classify(StatusCode::OK, None), Reachability::Ok);
        assert_eq!(classify(StatusCode::NOT_FOUND, None), Reachability::BadStatus(404));
        // 206 Partial Content (ranged GET) gilt als Erfolg.
        assert_eq!(
            classify(StatusCode::PARTIAL_CONTENT, Some("image/webp".to_string())),
            Reachability::Ok
        );
    }

    #[test]
    fn skips_non_http_schemes() {
        let m = meta_with(|m| {
            m.links.push(crate::model::LinkTag {
                rel: "icon".to_string(),
                href: "data:,".to_string(),
                hreflang: None,
            });
        });
        assert!(collect_candidates(&m, false).is_empty());
    }

    #[test]
    fn min_only_collects_only_icons() {
        let m = meta_with(|m| {
            m.meta_property
                .insert("og:image".to_string(), vec!["https://example.com/a.png".to_string()]);
            m.links.push(crate::model::LinkTag {
                rel: "icon".to_string(),
                href: "/favicon.ico".to_string(),
                hreflang: None,
            });
        });
        let cands = collect_candidates(&m, true);
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].rule, "icon.reachable");
    }
}
