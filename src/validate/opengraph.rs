//! Open-Graph-Validierung (`PLAN.md §6.2`). Regel-IDs `og.*`.
//! Strategie: ≥1 `og:`-Tag ⇒ OG gilt als „gewollt", Pflichtfelder werden Error.
//! Reachability der Bilder läuft separat in `images.rs`.

use crate::model::{Category, Finding, PageMetadata, Severity};

use super::is_absolute_url;

/// Gängige gültige `og:type`-Präfixe/Werte.
const VALID_OG_TYPES: &[&str] = &[
    "website", "article", "book", "profile", "music", "video", "place", "product",
];

pub fn validate(meta: &PageMetadata) -> Vec<Finding> {
    let og = Category::OpenGraph;
    let mut f = Vec::new();

    if !meta.has_open_graph() {
        f.push(Finding::new(
            og,
            Severity::Info,
            "og.present",
            "Keine Open-Graph-Metadaten vorhanden",
        ));
        return f;
    }

    // og:title (Pflicht)
    require(&mut f, og, meta.og("og:title"), "og.title.present", "og:title");

    // og:type (vorhanden & gültig)
    match meta.og("og:type") {
        Some(t) => {
            let base = t.split('.').next().unwrap_or(t).to_ascii_lowercase();
            if VALID_OG_TYPES.contains(&base.as_str()) {
                f.push(Finding::new(og, Severity::Pass, "og.type.present", "og:type gültig").with_detail(t.to_string()));
            } else {
                f.push(
                    Finding::new(og, Severity::Warning, "og.type.present", "og:type mit unbekanntem Wert")
                        .with_detail(t.to_string()),
                );
            }
        }
        None => f.push(Finding::new(og, Severity::Error, "og.type.present", "og:type fehlt")),
    }

    // og:url (Pflicht, absolut)
    match meta.og("og:url") {
        Some(u) if is_absolute_url(u) => {
            f.push(Finding::new(og, Severity::Pass, "og.url.present", "og:url vorhanden (absolut)").with_detail(u.to_string()));
        }
        Some(u) => f.push(
            Finding::new(og, Severity::Warning, "og.url.present", "og:url ist nicht absolut")
                .with_detail(u.to_string()),
        ),
        None => f.push(Finding::new(og, Severity::Error, "og.url.present", "og:url fehlt")),
    }

    // og:image (Pflicht) + absolut
    let images = PageMetadata::all(&meta.meta_property, "og:image");
    if images.is_empty() {
        f.push(Finding::new(og, Severity::Error, "og.image.present", "og:image fehlt"));
    } else {
        f.push(
            Finding::new(og, Severity::Pass, "og.image.present", "og:image vorhanden")
                .with_detail(format!("{} Bild(er)", images.len())),
        );
        for img in images {
            if !is_absolute_url(img) {
                f.push(
                    Finding::new(og, Severity::Warning, "og.image.absolute", "og:image ist nicht absolut")
                        .with_detail(img.clone()),
                );
            }
        }
        // Dimensions
        if meta.og("og:image:width").is_some() && meta.og("og:image:height").is_some() {
            f.push(Finding::new(og, Severity::Pass, "og.image.dimensions", "og:image:width/height gesetzt"));
        } else {
            f.push(Finding::new(
                og,
                Severity::Warning,
                "og.image.dimensions",
                "og:image:width/height nicht (vollständig) gesetzt",
            ));
        }
        // Alt
        if meta.og("og:image:alt").is_some() {
            f.push(Finding::new(og, Severity::Pass, "og.image.alt", "og:image:alt gesetzt"));
        } else {
            f.push(Finding::new(og, Severity::Info, "og.image.alt", "og:image:alt nicht gesetzt"));
        }
    }

    // Empfehlungen
    if meta.og("og:description").is_some() {
        f.push(Finding::new(og, Severity::Pass, "og.description.present", "og:description vorhanden"));
    } else {
        f.push(Finding::new(og, Severity::Warning, "og.description.present", "og:description empfohlen"));
    }
    if meta.og("og:site_name").is_some() {
        f.push(Finding::new(og, Severity::Pass, "og.site_name.present", "og:site_name vorhanden"));
    } else {
        f.push(Finding::new(og, Severity::Info, "og.site_name.present", "og:site_name empfohlen"));
    }

    f
}

/// Pflichtfeld: vorhanden ⇒ Pass, sonst Error.
fn require(f: &mut Vec<Finding>, cat: Category, value: Option<&str>, rule: &str, label: &str) {
    match value {
        Some(v) => f.push(Finding::new(cat, Severity::Pass, rule, format!("{label} vorhanden")).with_detail(v.to_string())),
        None => f.push(Finding::new(cat, Severity::Error, rule, format!("{label} fehlt"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::test_support::meta_with;

    fn rule_sev(f: &[Finding], rule: &str) -> Option<Severity> {
        f.iter().find(|x| x.rule == rule).map(|x| x.severity)
    }

    #[test]
    fn no_og_is_just_info() {
        let m = meta_with(|_| {});
        let f = validate(&m);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, "og.present");
        assert_eq!(f[0].severity, Severity::Info);
    }

    #[test]
    fn og_present_makes_required_fields_errors() {
        let m = meta_with(|m| {
            m.meta_property
                .insert("og:title".to_string(), vec!["T".to_string()]);
        });
        let f = validate(&m);
        // og vorhanden, aber type/url/image fehlen → Error
        assert_eq!(rule_sev(&f, "og.title.present"), Some(Severity::Pass));
        assert_eq!(rule_sev(&f, "og.type.present"), Some(Severity::Error));
        assert_eq!(rule_sev(&f, "og.url.present"), Some(Severity::Error));
        assert_eq!(rule_sev(&f, "og.image.present"), Some(Severity::Error));
    }

    #[test]
    fn relative_og_image_warns() {
        let m = meta_with(|m| {
            m.meta_property
                .insert("og:title".to_string(), vec!["T".to_string()]);
            m.meta_property
                .insert("og:image".to_string(), vec!["/rel.png".to_string()]);
        });
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "og.image.absolute"), Some(Severity::Warning));
    }
}
