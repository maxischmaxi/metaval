//! Internationalisierung: `rel="alternate"`-hreflang/x-default-Validierung.
//! Regel-IDs `hreflang.*`. Prüft Werte (BCP-47-Subset), Absolutheit der URLs,
//! x-default, Selbstverweis, Konflikte und Canonical-Konsistenz — also genau die
//! Dinge, an denen mehrsprachige Seiten in der Google-Suche scheitern.

use std::collections::{HashMap, HashSet};

use crate::model::{Category, Finding, PageMetadata, Severity};

use super::{is_absolute_url, normalize_url};

/// Einordnung eines hreflang-Werts.
enum HreflangKind {
    Valid,
    XDefault,
    Invalid(String),
}

pub fn validate(meta: &PageMetadata) -> Vec<Finding> {
    let h = Category::Hreflang;
    let mut f = Vec::new();
    let alts = meta.hreflang_alternates();

    // Ohne hreflang-Alternates ist alles in Ordnung — sie sind nur für
    // mehrsprachige Seiten relevant. Nur ein Info, kein Mangel.
    if alts.is_empty() {
        f.push(Finding::new(
            h,
            Severity::Info,
            "hreflang.present",
            "No hreflang alternates present (only needed for multilingual pages)",
        ));
        return f;
    }
    f.push(
        Finding::new(h, Severity::Pass, "hreflang.present", "hreflang alternates present")
            .with_detail(format!("{} entry/entries", alts.len())),
    );

    // 1) Werte + Absolutheit prüfen.
    let mut any_invalid = false;
    let mut any_relative = false;
    let mut has_x_default = false;
    for &(val, href) in &alts {
        match classify_hreflang(val) {
            HreflangKind::XDefault => has_x_default = true,
            HreflangKind::Valid => {}
            HreflangKind::Invalid(reason) => {
                any_invalid = true;
                f.push(
                    Finding::new(
                        h,
                        Severity::Warning,
                        "hreflang.value.valid",
                        format!("invalid hreflang value '{val}': {reason}"),
                    )
                    .with_detail(href.to_string()),
                );
            }
        }
        if !is_absolute_url(href) {
            any_relative = true;
            f.push(
                Finding::new(
                    h,
                    Severity::Warning,
                    "hreflang.absolute",
                    "hreflang URL is not absolute (Google requires full URLs)",
                )
                .with_detail(format!("{val} → {href}")),
            );
        }
    }
    if !any_invalid {
        f.push(Finding::new(h, Severity::Pass, "hreflang.value.valid", "all hreflang values plausible"));
    }
    if !any_relative {
        f.push(Finding::new(h, Severity::Pass, "hreflang.absolute", "all hreflang URLs are absolute"));
    }

    // 2) x-default (für Standard-/Sprachauswahlseite empfohlen).
    if has_x_default {
        f.push(Finding::new(h, Severity::Pass, "hreflang.x_default", "x-default present"));
    } else {
        f.push(Finding::new(
            h,
            Severity::Info,
            "hreflang.x_default",
            "no x-default — recommended for a language-selection/default page",
        ));
    }

    // 3) Konflikte: gleicher hreflang-Wert → unterschiedliche Ziel-URLs.
    let mut by_value: HashMap<String, HashSet<String>> = HashMap::new();
    for &(val, href) in &alts {
        let target = resolve(meta, href);
        by_value.entry(val.to_ascii_lowercase()).or_default().insert(target);
    }
    let mut any_conflict = false;
    for (val, targets) in &by_value {
        if targets.len() > 1 {
            any_conflict = true;
            let mut list: Vec<&String> = targets.iter().collect();
            list.sort();
            let detail = list.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
            f.push(
                Finding::new(
                    h,
                    Severity::Warning,
                    "hreflang.conflict",
                    format!("conflicting hreflang entries for '{val}'"),
                )
                .with_detail(detail),
            );
        }
    }
    if !any_conflict {
        f.push(Finding::new(h, Severity::Pass, "hreflang.conflict", "no conflicting hreflang entries"));
    }

    // 4) Selbstverweis: jede Sprachvariante soll sich selbst listen.
    let self_norm = normalize_url(&meta.final_url);
    let self_ref = alts.iter().any(|&(_, href)| resolve(meta, href) == self_norm);
    if self_ref {
        f.push(Finding::new(h, Severity::Pass, "hreflang.self_reference", "Page references itself via hreflang"));
    } else {
        f.push(Finding::new(
            h,
            Severity::Info,
            "hreflang.self_reference",
            "no hreflang self-reference found (each language variant should list itself)",
        ));
    }

    // 5) Canonical-Konsistenz: Canonical soll auf die eigene Seite zeigen, nicht
    //    auf eine andere Sprachvariante — sonst hebelt es hreflang aus.
    if let Some(canon) = meta.canonical()
        && let Ok(canon_abs) = meta.final_url.join(canon)
    {
        let canon_norm = normalize_url(&canon_abs);
        if canon_norm != self_norm {
            let points_to_alt = alts.iter().any(|&(_, href)| resolve(meta, href) == canon_norm);
            if points_to_alt {
                f.push(
                    Finding::new(
                        h,
                        Severity::Warning,
                        "hreflang.canonical_consistency",
                        "Canonical points to a different language variant — breaks hreflang (each variant should canonicalize to itself)",
                    )
                    .with_detail(canon_norm),
                );
            }
        }
    }

    f
}

/// Löst `href` relativ zu `final_url` auf und normalisiert ihn für Vergleiche.
fn resolve(meta: &PageMetadata, href: &str) -> String {
    meta.final_url
        .join(href)
        .map(|u| normalize_url(&u))
        .unwrap_or_else(|_| href.to_string())
}

/// Validiert einen hreflang-Wert gegen ein BCP-47-Subset (Google-Praxis):
/// Sprache (2–3 Buchstaben), optional Script (4 Buchstaben) und/oder Region
/// (2 Buchstaben bzw. 3 Ziffern). `x-default` ist gültig.
fn classify_hreflang(value: &str) -> HreflangKind {
    let v = value.trim();
    if v.eq_ignore_ascii_case("x-default") {
        return HreflangKind::XDefault;
    }
    if v.is_empty() {
        return HreflangKind::Invalid("empty".to_string());
    }
    // Häufiger Fehler: Unterstrich statt Bindestrich (z. B. en_US).
    if v.contains('_') {
        return HreflangKind::Invalid("'_' used instead of '-' (correct e.g. en-US)".to_string());
    }

    let mut parts = v.split('-');
    let lang = parts.next().unwrap_or("");
    if !(2..=3).contains(&lang.len()) || !lang.chars().all(|c| c.is_ascii_alphabetic()) {
        return HreflangKind::Invalid(format!("'{lang}' is not a valid language code"));
    }
    for sub in parts {
        let is_region_alpha = sub.len() == 2 && sub.chars().all(|c| c.is_ascii_alphabetic());
        let is_region_num = sub.len() == 3 && sub.chars().all(|c| c.is_ascii_digit());
        let is_script = sub.len() == 4 && sub.chars().all(|c| c.is_ascii_alphabetic());
        if !(is_region_alpha || is_region_num || is_script) {
            return HreflangKind::Invalid(format!("subtag '{sub}' unknown"));
        }
    }
    HreflangKind::Valid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LinkTag;
    use crate::validate::test_support::meta_with;

    fn alt(hreflang: &str, href: &str) -> LinkTag {
        LinkTag {
            rel: "alternate".to_string(),
            href: href.to_string(),
            hreflang: Some(hreflang.to_string()),
        }
    }

    fn rule_sev(f: &[Finding], rule: &str) -> Option<Severity> {
        f.iter().find(|x| x.rule == rule).map(|x| x.severity)
    }

    fn rule_count(f: &[Finding], rule: &str, sev: Severity) -> usize {
        f.iter().filter(|x| x.rule == rule && x.severity == sev).count()
    }

    #[test]
    fn no_alternates_is_just_info() {
        let m = meta_with(|_| {});
        let f = validate(&m);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, "hreflang.present");
        assert_eq!(f[0].severity, Severity::Info);
    }

    #[test]
    fn underscore_value_is_flagged() {
        let m = meta_with(|m| {
            m.links.push(alt("en_US", "https://example.com/en"));
        });
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "hreflang.value.valid"), Some(Severity::Warning));
    }

    #[test]
    fn valid_values_pass_and_xdefault_detected() {
        let m = meta_with(|m| {
            m.links.push(alt("de", "https://example.com/"));
            m.links.push(alt("en-US", "https://example.com/en"));
            m.links.push(alt("x-default", "https://example.com/"));
        });
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "hreflang.value.valid"), Some(Severity::Pass));
        assert_eq!(rule_sev(&f, "hreflang.x_default"), Some(Severity::Pass));
        // final_url ist https://example.com/ → Selbstverweis vorhanden.
        assert_eq!(rule_sev(&f, "hreflang.self_reference"), Some(Severity::Pass));
    }

    #[test]
    fn relative_href_warns() {
        let m = meta_with(|m| {
            m.links.push(alt("de", "/de"));
        });
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "hreflang.absolute"), Some(Severity::Warning));
    }

    #[test]
    fn conflicting_same_value_warns() {
        let m = meta_with(|m| {
            m.links.push(alt("de", "https://example.com/de1"));
            m.links.push(alt("de", "https://example.com/de2"));
        });
        let f = validate(&m);
        assert_eq!(rule_count(&f, "hreflang.conflict", Severity::Warning), 1);
    }

    #[test]
    fn missing_xdefault_is_info() {
        let m = meta_with(|m| {
            m.links.push(alt("de", "https://example.com/"));
            m.links.push(alt("en", "https://example.com/en"));
        });
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "hreflang.x_default"), Some(Severity::Info));
    }

    #[test]
    fn canonical_to_other_variant_breaks_hreflang() {
        let m = meta_with(|m| {
            // Seite selbst ist example.com/, kanonisiert aber auf die EN-Variante.
            m.links.push(alt("de", "https://example.com/"));
            m.links.push(alt("en", "https://example.com/en"));
            m.links.push(LinkTag {
                rel: "canonical".to_string(),
                href: "https://example.com/en".to_string(),
                hreflang: None,
            });
        });
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "hreflang.canonical_consistency"), Some(Severity::Warning));
    }
}
