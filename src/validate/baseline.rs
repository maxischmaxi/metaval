//! Baseline-/Minimum-Validierung. Stets aktiv. Regel-IDs `base.*`.

use std::collections::HashSet;

use crate::model::{Category, Finding, PageMetadata, Severity};

use super::{is_absolute_url, normalize_url};

const TITLE_MIN: usize = 10;
const TITLE_MAX: usize = 60;
const DESC_MIN: usize = 50;
const DESC_MAX: usize = 160;

const VALID_ROBOTS: &[&str] = &[
    "index",
    "noindex",
    "follow",
    "nofollow",
    "none",
    "noarchive",
    "nosnippet",
    "noimageindex",
    "notranslate",
    "all",
    "max-snippet",
    "max-image-preview",
    "max-video-preview",
    "unavailable_after",
];

pub fn validate(meta: &PageMetadata) -> Vec<Finding> {
    let mut f = Vec::new();
    let b = Category::Baseline;

    // title
    match meta.title.as_deref().filter(|t| !t.trim().is_empty()) {
        Some(title) => {
            f.push(Finding::new(b, Severity::Pass, "base.title.present", "Title present"));
            let len = title.chars().count();
            if !(TITLE_MIN..=TITLE_MAX).contains(&len) {
                f.push(
                    Finding::new(
                        b,
                        Severity::Warning,
                        "base.title.length",
                        format!("Title length {len} outside the recommended {TITLE_MIN}–{TITLE_MAX} characters"),
                    )
                    .with_detail(title.to_string()),
                );
            } else {
                f.push(Finding::new(b, Severity::Pass, "base.title.length", "Title length within the recommended range"));
            }
        }
        None => f.push(Finding::new(
            b,
            Severity::Error,
            "base.title.present",
            "<title> missing or empty",
        )),
    }

    // Mehrere <title> im <head>? Suchmaschinen wählen dann selbst eines aus.
    if meta.title_count > 1 {
        f.push(Finding::new(
            b,
            Severity::Warning,
            "base.title.unique",
            format!("{} <title> elements found — keep exactly one", meta.title_count),
        ));
    }

    // description
    match meta.named("description") {
        Some(desc) => {
            f.push(Finding::new(b, Severity::Pass, "base.description.present", "Meta description present"));
            let len = desc.chars().count();
            if !(DESC_MIN..=DESC_MAX).contains(&len) {
                f.push(
                    Finding::new(
                        b,
                        Severity::Warning,
                        "base.description.length",
                        format!("Description length {len} outside the recommended {DESC_MIN}–{DESC_MAX} characters"),
                    )
                    .with_detail(desc.to_string()),
                );
            } else {
                f.push(Finding::new(b, Severity::Pass, "base.description.length", "Description length within the recommended range"));
            }
        }
        None => f.push(Finding::new(
            b,
            Severity::Error,
            "base.description.present",
            "<meta name=\"description\"> missing or empty",
        )),
    }

    // Mehrere Descriptions (typisch: Theme + SEO-Plugin injizieren beide eine).
    let descriptions = PageMetadata::all(&meta.meta_named, "description");
    if descriptions.len() > 1 {
        f.push(Finding::new(
            b,
            Severity::Warning,
            "base.description.unique",
            format!(
                "{} <meta name=\"description\"> tags found — keep exactly one",
                descriptions.len()
            ),
        ));
    }

    // charset
    match &meta.charset {
        Some(cs) => {
            f.push(
                Finding::new(b, Severity::Pass, "base.charset.present", "Character set declared")
                    .with_detail(cs.clone()),
            );
            if !cs.eq_ignore_ascii_case("utf-8") {
                f.push(
                    Finding::new(
                        b,
                        Severity::Info,
                        "base.charset.utf8",
                        "Charset is not UTF-8 — UTF-8 is the recommended encoding for the web",
                    )
                    .with_detail(cs.clone()),
                );
            }
        }
        None => f.push(Finding::new(
            b,
            Severity::Error,
            "base.charset.present",
            "No character set declared (<meta charset>)",
        )),
    }

    // viewport
    if meta.named("viewport").is_some() {
        f.push(Finding::new(b, Severity::Pass, "base.viewport.present", "Viewport set"));
    } else {
        f.push(Finding::new(
            b,
            Severity::Warning,
            "base.viewport.present",
            "<meta name=\"viewport\"> missing",
        ));
    }

    // html lang
    match meta.html_lang.as_deref().filter(|l| !l.trim().is_empty()) {
        Some(lang) => f.push(
            Finding::new(b, Severity::Pass, "base.lang.present", "html lang set")
                .with_detail(lang.to_string()),
        ),
        None => f.push(Finding::new(
            b,
            Severity::Warning,
            "base.lang.present",
            "<html lang> not set",
        )),
    }

    // canonical present + matches
    match meta.canonical() {
        Some(canon) => {
            f.push(
                Finding::new(b, Severity::Pass, "base.canonical.present", "Canonical link present")
                    .with_detail(canon.to_string()),
            );
            if !is_absolute_url(canon) {
                f.push(
                    Finding::new(b, Severity::Info, "base.canonical.absolute", "Canonical should be an absolute URL")
                        .with_detail(canon.to_string()),
                );
            }
            let resolved = meta.final_url.join(canon).ok();
            match resolved {
                Some(c) if normalize_url(&c) == normalize_url(&meta.final_url) => {
                    f.push(Finding::new(
                        b,
                        Severity::Pass,
                        "base.canonical.matches",
                        "Canonical matches the final URL",
                    ));
                }
                Some(c) => f.push(
                    Finding::new(
                        b,
                        Severity::Info,
                        "base.canonical.matches",
                        "Canonical differs from the final URL",
                    )
                    .with_detail(format!("canonical={c} final={}", meta.final_url)),
                ),
                None => f.push(
                    Finding::new(
                        b,
                        Severity::Warning,
                        "base.canonical.matches",
                        "Canonical URL not resolvable",
                    )
                    .with_detail(canon.to_string()),
                ),
            }
        }
        None => f.push(Finding::new(
            b,
            Severity::Warning,
            "base.canonical.present",
            "<link rel=\"canonical\"> missing",
        )),
    }

    // Mehrere (widersprüchliche) canonical-Links? Google wählt dann selbst aus.
    let canon_hrefs = meta.canonical_hrefs();
    if canon_hrefs.len() > 1 {
        let distinct: HashSet<String> = canon_hrefs
            .iter()
            .map(|h| {
                meta.final_url
                    .join(h)
                    .map(|u| normalize_url(&u))
                    .unwrap_or_else(|_| (*h).to_string())
            })
            .collect();
        if distinct.len() > 1 {
            f.push(
                Finding::new(
                    b,
                    Severity::Warning,
                    "base.canonical.unique",
                    "Multiple conflicting <link rel=\"canonical\"> found",
                )
                .with_detail(canon_hrefs.join(" | ")),
            );
        }
    }

    // Indexierbarkeit: noindex via <meta robots>, <meta googlebot> oder X-Robots-Tag.
    // Das ist der entscheidende Faktor, ob Google die Seite überhaupt indexiert.
    f.extend(indexability(meta, b));

    // robots (nur falls vorhanden)
    if let Some(robots) = meta.named("robots") {
        let unknown: Vec<&str> = robots
            .split(',')
            .map(|t| t.split(':').next().unwrap_or(t).trim())
            .filter(|t| !t.is_empty())
            .filter(|t| !VALID_ROBOTS.contains(&t.to_ascii_lowercase().as_str()))
            .collect();
        if unknown.is_empty() {
            f.push(
                Finding::new(b, Severity::Info, "base.robots.parse", "robots directives plausible")
                    .with_detail(robots.to_string()),
            );
        } else {
            f.push(
                Finding::new(
                    b,
                    Severity::Info,
                    "base.robots.parse",
                    "unknown robots directive(s)",
                )
                .with_detail(unknown.join(", ")),
            );
        }
    }

    f
}

/// Prüft, ob die Seite indexierbar ist (kein `noindex`/`none`), und zwar aus allen
/// Quellen zusammen: `<meta name="robots">`, `<meta name="googlebot">` und dem
/// `X-Robots-Tag`-Response-Header.
fn indexability(meta: &PageMetadata, b: Category) -> Vec<Finding> {
    let mut f = Vec::new();
    let mut tokens: Vec<String> = Vec::new();
    let mut sources: Vec<&str> = Vec::new();

    for key in ["robots", "googlebot"] {
        if let Some(v) = meta.named(key) {
            sources.push(key);
            push_tokens(v, &mut tokens);
        }
    }
    if let Some(v) = &meta.x_robots_tag {
        sources.push("X-Robots-Tag");
        push_tokens(v, &mut tokens);
    }

    let noindex = tokens.iter().any(|t| t == "noindex" || t == "none");
    let nofollow = tokens.iter().any(|t| t == "nofollow" || t == "none");

    if noindex {
        f.push(
            Finding::new(
                b,
                Severity::Warning,
                "base.robots.indexable",
                "Page is set to noindex — search engines will not index it",
            )
            .with_detail(format!("Source: {}", sources.join(", "))),
        );
    } else {
        f.push(Finding::new(b, Severity::Pass, "base.robots.indexable", "Page is indexable (no noindex)"));
    }

    if nofollow && !noindex {
        f.push(Finding::new(
            b,
            Severity::Info,
            "base.robots.follow",
            "nofollow set — links on this page will not be followed",
        ));
    }

    f
}

/// Zerlegt einen robots-/X-Robots-Tag-Wert in einzelne, lowercased Direktiv-Tokens.
fn push_tokens(value: &str, out: &mut Vec<String>) {
    for t in value.split([',', ' ', '\t']) {
        let t = t.trim().to_ascii_lowercase();
        if !t.is_empty() {
            out.push(t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LinkTag;
    use crate::validate::test_support::meta_with;

    fn rule_sev(f: &[Finding], rule: &str) -> Option<Severity> {
        f.iter().find(|x| x.rule == rule).map(|x| x.severity)
    }

    #[test]
    fn missing_title_and_description_are_errors() {
        let m = meta_with(|_| {});
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "base.title.present"), Some(Severity::Error));
        assert_eq!(rule_sev(&f, "base.description.present"), Some(Severity::Error));
        assert_eq!(rule_sev(&f, "base.charset.present"), Some(Severity::Error));
    }

    #[test]
    fn good_baseline_passes() {
        let m = meta_with(|m| {
            m.title = Some("Eine gute Seitenüberschrift".to_string());
            m.charset = Some("utf-8".to_string());
            m.html_lang = Some("de".to_string());
            m.meta_named.insert(
                "description".to_string(),
                vec!["Eine ausreichend lange Beschreibung dieser Seite, die im empfohlenen Bereich liegt.".to_string()],
            );
            m.meta_named
                .insert("viewport".to_string(), vec!["width=device-width".to_string()]);
        });
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "base.title.present"), Some(Severity::Pass));
        assert_eq!(rule_sev(&f, "base.description.present"), Some(Severity::Pass));
        assert_eq!(rule_sev(&f, "base.title.length"), Some(Severity::Pass));
        assert_eq!(rule_sev(&f, "base.viewport.present"), Some(Severity::Pass));
    }

    #[test]
    fn short_title_warns_on_length() {
        let m = meta_with(|m| m.title = Some("Kurz".to_string()));
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "base.title.length"), Some(Severity::Warning));
    }

    #[test]
    fn indexable_by_default() {
        let m = meta_with(|_| {});
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "base.robots.indexable"), Some(Severity::Pass));
    }

    #[test]
    fn noindex_meta_robots_is_warning() {
        let m = meta_with(|m| {
            m.meta_named
                .insert("robots".to_string(), vec!["noindex, nofollow".to_string()]);
        });
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "base.robots.indexable"), Some(Severity::Warning));
    }

    #[test]
    fn robots_none_counts_as_noindex() {
        let m = meta_with(|m| {
            m.meta_named.insert("robots".to_string(), vec!["none".to_string()]);
        });
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "base.robots.indexable"), Some(Severity::Warning));
    }

    #[test]
    fn x_robots_tag_header_noindex_detected() {
        let m = meta_with(|m| {
            m.x_robots_tag = Some("googlebot: noindex".to_string());
        });
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "base.robots.indexable"), Some(Severity::Warning));
    }

    #[test]
    fn duplicate_titles_and_descriptions_warn() {
        let m = meta_with(|m| {
            m.title = Some("Ein völlig normaler Seitentitel".to_string());
            m.title_count = 2;
            m.meta_named.insert(
                "description".to_string(),
                vec!["Beschreibung eins".to_string(), "Beschreibung zwei".to_string()],
            );
        });
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "base.title.unique"), Some(Severity::Warning));
        assert_eq!(rule_sev(&f, "base.description.unique"), Some(Severity::Warning));
    }

    #[test]
    fn non_utf8_charset_is_info() {
        let m = meta_with(|m| m.charset = Some("ISO-8859-1".to_string()));
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "base.charset.present"), Some(Severity::Pass));
        assert_eq!(rule_sev(&f, "base.charset.utf8"), Some(Severity::Info));

        let m = meta_with(|m| m.charset = Some("UTF-8".to_string()));
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "base.charset.utf8"), None);
    }

    #[test]
    fn multiple_conflicting_canonicals_warn() {
        let m = meta_with(|m| {
            m.links.push(LinkTag {
                rel: "canonical".to_string(),
                href: "https://example.com/a".to_string(),
                hreflang: None,
            });
            m.links.push(LinkTag {
                rel: "canonical".to_string(),
                href: "https://example.com/b".to_string(),
                hreflang: None,
            });
        });
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "base.canonical.unique"), Some(Severity::Warning));
    }
}
