//! HTML → `PageMetadata`. Strikt **synchron**: scrapers `Html`/`ElementRef` sind
//! `!Send` und dürfen nicht über ein `.await` gehalten werden. Erst alles in owned
//! Daten ziehen, dann zurückgeben — danach laufen die async Bild-Checks.

use std::collections::HashMap;
use std::sync::LazyLock;

use scraper::{Html, Selector};

use crate::fetch::FetchedPage;
use crate::model::{LinkTag, PageMetadata};

macro_rules! sel {
    ($name:ident, $css:expr) => {
        static $name: LazyLock<Selector> = LazyLock::new(|| Selector::parse($css).unwrap());
    };
}

sel!(SEL_TITLE, "title");
sel!(SEL_HTML, "html");
sel!(SEL_META_NAME, "meta[name]");
sel!(SEL_META_PROPERTY, "meta[property]");
sel!(SEL_META_CHARSET, "meta[charset]");
sel!(SEL_META_HTTP_EQUIV, "meta[http-equiv]");
sel!(SEL_LINK, "link[rel]");
sel!(SEL_JSONLD, r#"script[type="application/ld+json"]"#);
sel!(SEL_BODY, "body");

/// Extrahiert normalisierte Metadaten aus einer abgerufenen Seite.
pub fn extract(page: &FetchedPage) -> PageMetadata {
    let doc = Html::parse_document(&page.html);

    let title = doc
        .select(&SEL_TITLE)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string());

    let html_lang = doc
        .select(&SEL_HTML)
        .next()
        .and_then(|e| e.value().attr("lang"))
        .map(str::to_owned);

    let mut meta_named: HashMap<String, Vec<String>> = HashMap::new();
    for el in doc.select(&SEL_META_NAME) {
        if let (Some(name), Some(content)) = (el.value().attr("name"), el.value().attr("content")) {
            insert(&mut meta_named, name, content);
        }
    }

    let mut meta_property: HashMap<String, Vec<String>> = HashMap::new();
    for el in doc.select(&SEL_META_PROPERTY) {
        if let (Some(prop), Some(content)) =
            (el.value().attr("property"), el.value().attr("content"))
        {
            insert(&mut meta_property, prop, content);
        }
    }

    let charset = extract_charset(&doc);

    let mut links = Vec::new();
    for el in doc.select(&SEL_LINK) {
        let v = el.value();
        if let (Some(rel), Some(href)) = (v.attr("rel"), v.attr("href")) {
            links.push(LinkTag {
                rel: rel.trim().to_string(),
                href: href.trim().to_string(),
                hreflang: v.attr("hreflang").map(|s| s.trim().to_string()),
            });
        }
    }

    let mut json_ld = Vec::new();
    let mut json_ld_errors = Vec::new();
    for el in doc.select(&SEL_JSONLD) {
        let raw = el.inner_html();
        if raw.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(&raw) {
            Ok(v) => json_ld.push(v),
            Err(_) => json_ld_errors.push(raw.trim().to_string()),
        }
    }

    // SPA-Heuristik: praktisch keine Kopf-Metadaten und kaum Body-Text.
    let body_text_len = doc
        .select(&SEL_BODY)
        .next()
        .map(|b| b.text().collect::<String>().trim().chars().count())
        .unwrap_or(0);
    let looks_like_spa = title.as_deref().map(str::is_empty).unwrap_or(true)
        && meta_named.is_empty()
        && meta_property.is_empty()
        && json_ld.is_empty()
        && body_text_len < 32;

    PageMetadata {
        final_url: page.final_url.clone(),
        status: page.status,
        content_type: page.content_type.clone(),
        x_robots_tag: page.x_robots_tag.clone(),
        title,
        html_lang,
        meta_named,
        meta_property,
        charset,
        links,
        json_ld,
        json_ld_errors,
        looks_like_spa,
    }
}

/// Fügt einen Wert (getrimmt) unter dem lowercased Key in die Multi-Map ein.
fn insert(map: &mut HashMap<String, Vec<String>>, key: &str, value: &str) {
    map.entry(key.trim().to_ascii_lowercase())
        .or_default()
        .push(value.trim().to_string());
}

/// Zeichensatz aus `<meta charset>` oder `<meta http-equiv="Content-Type">`.
fn extract_charset(doc: &Html) -> Option<String> {
    if let Some(cs) = doc
        .select(&SEL_META_CHARSET)
        .next()
        .and_then(|e| e.value().attr("charset"))
    {
        return Some(cs.trim().to_string());
    }
    for el in doc.select(&SEL_META_HTTP_EQUIV) {
        let v = el.value();
        if v.attr("http-equiv")
            .map(|h| h.eq_ignore_ascii_case("content-type"))
            .unwrap_or(false)
            && let Some(content) = v.attr("content")
            && let Some(idx) = content.to_ascii_lowercase().find("charset=")
        {
            let cs = content[idx + "charset=".len()..].trim();
            let cs = cs.split(';').next().unwrap_or(cs).trim();
            if !cs.is_empty() {
                return Some(cs.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    fn page(html: &str) -> FetchedPage {
        FetchedPage {
            requested_url: Url::parse("https://example.com/").unwrap(),
            final_url: Url::parse("https://example.com/").unwrap(),
            status: 200,
            content_type: Some("text/html".to_string()),
            x_robots_tag: None,
            html: html.to_string(),
        }
    }

    #[test]
    fn extracts_full_metadata() {
        let html = r#"<!DOCTYPE html><html lang="de"><head>
            <meta charset="utf-8">
            <title>  Beispiel  </title>
            <meta name="description" content="Eine Beschreibung">
            <meta name="viewport" content="width=device-width">
            <meta property="og:title" content="OG Titel">
            <meta property="og:image" content="https://example.com/a.png">
            <meta property="og:image" content="https://example.com/b.png">
            <meta name="twitter:card" content="summary">
            <link rel="canonical" href="https://example.com/">
            <link rel="icon" href="/favicon.ico">
            </head><body><p>Inhalt der Seite mit ausreichend Text.</p></body></html>"#;
        let m = extract(&page(html));
        assert_eq!(m.title.as_deref(), Some("Beispiel"));
        assert_eq!(m.html_lang.as_deref(), Some("de"));
        assert_eq!(m.charset.as_deref(), Some("utf-8"));
        assert_eq!(m.named("description"), Some("Eine Beschreibung"));
        assert_eq!(m.named("twitter:card"), Some("summary"));
        assert_eq!(m.og("og:title"), Some("OG Titel"));
        // Mehrere og:image müssen erhalten bleiben.
        assert_eq!(PageMetadata::all(&m.meta_property, "og:image").len(), 2);
        assert!(m.has_open_graph());
        assert!(m.has_twitter());
        assert_eq!(m.canonical(), Some("https://example.com/"));
        assert_eq!(m.links.len(), 2);
        assert!(!m.looks_like_spa);
    }

    #[test]
    fn charset_from_http_equiv() {
        let html = r#"<html><head>
            <meta http-equiv="Content-Type" content="text/html; charset=ISO-8859-1">
            <title>x</title></head><body>text</body></html>"#;
        let m = extract(&page(html));
        assert_eq!(m.charset.as_deref(), Some("ISO-8859-1"));
    }

    #[test]
    fn broken_json_ld_is_collected_not_fatal() {
        let html = r#"<html><head><title>x</title>
            <script type="application/ld+json">{ not valid json }</script>
            <script type="application/ld+json">{"@type":"WebSite","name":"X"}</script>
            </head><body>text</body></html>"#;
        let m = extract(&page(html));
        assert_eq!(m.json_ld.len(), 1);
        assert_eq!(m.json_ld_errors.len(), 1);
    }

    #[test]
    fn captures_hreflang_on_alternate_links() {
        let html = r#"<html><head><title>x</title>
            <link rel="alternate" hreflang="de" href="https://example.com/de">
            <link rel="alternate" hreflang="en-US" href="https://example.com/en">
            <link rel="alternate" type="application/rss+xml" href="/feed.xml">
            </head><body>text</body></html>"#;
        let m = extract(&page(html));
        let alts = m.hreflang_alternates();
        assert_eq!(alts.len(), 2);
        assert!(alts.iter().any(|(hl, _)| *hl == "de"));
        assert!(alts.iter().any(|(hl, href)| *hl == "en-US" && *href == "https://example.com/en"));
    }

    #[test]
    fn detects_empty_spa_shell() {
        let html = r#"<html><head></head><body><div id="root"></div></body></html>"#;
        let m = extract(&page(html));
        assert!(m.looks_like_spa);
    }
}
