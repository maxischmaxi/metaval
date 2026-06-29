//! Normalisierte Datenmodelle: extrahierte Seiten-Metadaten + Validierungs-Findings.

use std::collections::HashMap;

use serde::Serialize;
use url::Url;

/// Schweregrad eines Findings. Deklarationsreihenfolge entspricht `PLAN.md §6`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
    Pass,
}

impl Severity {
    /// Numerischer Rang: höher = schwerer. Für Threshold-Vergleiche.
    fn rank(self) -> u8 {
        match self {
            Self::Error => 3,
            Self::Warning => 2,
            Self::Info => 1,
            Self::Pass => 0,
        }
    }

    /// `true`, wenn dieser Schweregrad mindestens so schwer wie `threshold` ist.
    pub fn at_least(self, threshold: Severity) -> bool {
        self.rank() >= threshold.rank()
    }

    /// Symbol für die pretty-Ausgabe.
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Error => "✗",
            Self::Warning => "⚠",
            Self::Info => "ℹ",
            Self::Pass => "✓",
        }
    }
}

/// Validierungs-Kategorie (= Quelle der Regel).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Baseline,
    Hreflang,
    OpenGraph,
    Twitter,
    SchemaOrg,
    Images,
    Fetch,
}

impl Category {
    /// Stabile Sortier-/Gruppierungsreihenfolge für den Report.
    pub fn order(self) -> u8 {
        match self {
            Self::Baseline => 0,
            Self::Hreflang => 1,
            Self::OpenGraph => 2,
            Self::Twitter => 3,
            Self::SchemaOrg => 4,
            Self::Images => 5,
            Self::Fetch => 6,
        }
    }

    /// Anzeigename für die pretty-Ausgabe.
    pub fn title(self) -> &'static str {
        match self {
            Self::Baseline => "Baseline",
            Self::Hreflang => "Internationalization (hreflang)",
            Self::OpenGraph => "Open Graph",
            Self::Twitter => "Twitter Cards",
            Self::SchemaOrg => "schema.org / JSON-LD",
            Self::Images => "Images",
            Self::Fetch => "Fetch",
        }
    }
}

/// Ein einzelnes Validierungs-Ergebnis.
#[derive(Clone, Debug, Serialize)]
pub struct Finding {
    pub category: Category,
    pub severity: Severity,
    /// Stabile Regel-ID, z. B. `"og.image.present"`.
    pub rule: String,
    /// Menschenlesbare Meldung.
    pub message: String,
    /// Optionaler Detailwert (konkreter Wert / URL / Statuscode).
    pub detail: Option<String>,
}

impl Finding {
    pub fn new(
        category: Category,
        severity: Severity,
        rule: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            category,
            severity,
            rule: rule.into(),
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// `<link rel=... href=...>`-Tag.
#[derive(Clone, Debug)]
pub struct LinkTag {
    pub rel: String,
    pub href: String,
    /// `hreflang`-Attribut (nur bei `rel="alternate"`-Sprachvarianten gesetzt).
    pub hreflang: Option<String>,
}

/// Aus dem HTML extrahierte, normalisierte Metadaten.
#[derive(Clone, Debug)]
pub struct PageMetadata {
    pub final_url: Url,
    pub status: u16,
    pub content_type: Option<String>,
    /// `X-Robots-Tag`-Response-Header (für die Indexierbarkeits-Prüfung).
    pub x_robots_tag: Option<String>,
    pub title: Option<String>,
    pub html_lang: Option<String>,
    /// `<meta name=... content=...>` — Mehrfachwerte je Key (lowercased).
    pub meta_named: HashMap<String, Vec<String>>,
    /// `<meta property=... content=...>` — Mehrfachwerte je Key (lowercased).
    pub meta_property: HashMap<String, Vec<String>>,
    pub charset: Option<String>,
    pub links: Vec<LinkTag>,
    /// Erfolgreich geparste `ld+json`-Blöcke.
    pub json_ld: Vec<serde_json::Value>,
    /// Roh-Text der `ld+json`-Blöcke, die nicht geparst werden konnten.
    pub json_ld_errors: Vec<String>,
    /// Heuristik: leerer `<body>`/`<head>`-Inhalt ⇒ evtl. SPA ohne `--render`.
    pub looks_like_spa: bool,
}

impl PageMetadata {
    /// Erster Wert für `key` in `map`, falls vorhanden und nicht leer.
    pub fn first<'a>(map: &'a HashMap<String, Vec<String>>, key: &str) -> Option<&'a str> {
        map.get(key)
            .and_then(|v| v.first())
            .map(String::as_str)
            .filter(|s| !s.trim().is_empty())
    }

    /// Alle Werte für `key` in `map`.
    pub fn all<'a>(map: &'a HashMap<String, Vec<String>>, key: &str) -> &'a [String] {
        map.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Erster `og:*`-Property-Wert.
    pub fn og(&self, key: &str) -> Option<&str> {
        Self::first(&self.meta_property, key)
    }

    /// Erster `name=`-Meta-Wert (twitter:*, description, …).
    pub fn named(&self, key: &str) -> Option<&str> {
        Self::first(&self.meta_named, key)
    }

    /// `true`, wenn die Seite mindestens ein `og:`-Tag besitzt.
    pub fn has_open_graph(&self) -> bool {
        self.meta_property.keys().any(|k| k.starts_with("og:"))
    }

    /// `true`, wenn die Seite mindestens ein `twitter:`-Tag besitzt.
    pub fn has_twitter(&self) -> bool {
        self.meta_named.keys().any(|k| k.starts_with("twitter:"))
    }

    /// `<link rel="canonical">`-Href, falls vorhanden.
    pub fn canonical(&self) -> Option<&str> {
        self.links
            .iter()
            .find(|l| l.rel.eq_ignore_ascii_case("canonical"))
            .map(|l| l.href.as_str())
    }

    /// Alle `<link rel="canonical">`-Hrefs (für die Eindeutigkeits-Prüfung).
    pub fn canonical_hrefs(&self) -> Vec<&str> {
        self.links
            .iter()
            .filter(|l| l.rel.eq_ignore_ascii_case("canonical"))
            .map(|l| l.href.as_str())
            .collect()
    }

    /// `rel="alternate"`-Sprachvarianten als `(hreflang-Wert, href)`.
    pub fn hreflang_alternates(&self) -> Vec<(&str, &str)> {
        self.links
            .iter()
            .filter(|l| l.rel.split_whitespace().any(|r| r.eq_ignore_ascii_case("alternate")))
            .filter_map(|l| l.hreflang.as_deref().map(|hl| (hl.trim(), l.href.as_str())))
            .filter(|(hl, _)| !hl.is_empty())
            .collect()
    }

    /// `true`, wenn der Content-Type auf ein HTML-Dokument hindeutet.
    pub fn is_html(&self) -> bool {
        match &self.content_type {
            None => true,
            Some(ct) => {
                let ct = ct.to_ascii_lowercase();
                ct.is_empty()
                    || ct.starts_with("text/html")
                    || ct.starts_with("application/xhtml+xml")
            }
        }
    }
}
