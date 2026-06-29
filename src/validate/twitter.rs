//! Twitter-Card-Validierung (`PLAN.md §6.3`). Regel-IDs `tw.*`.
//! Konsistent zu OG: ohne jegliches `twitter:`-Tag nur ein Info, sonst volle Prüfung.

use crate::model::{Category, Finding, PageMetadata, Severity};

const VALID_CARDS: &[&str] = &["summary", "summary_large_image", "app", "player"];
/// Card-Typen, die ein Bild verlangen.
const IMAGE_CARDS: &[&str] = &["summary", "summary_large_image", "player"];

pub fn validate(meta: &PageMetadata) -> Vec<Finding> {
    let tw = Category::Twitter;
    let mut f = Vec::new();

    if !meta.has_twitter() {
        f.push(Finding::new(tw, Severity::Info, "tw.present", "Keine Twitter-Card-Metadaten vorhanden"));
        return f;
    }

    // twitter:card
    let card = meta.named("twitter:card");
    match card {
        Some(c) => {
            f.push(Finding::new(tw, Severity::Pass, "tw.card.present", "twitter:card vorhanden").with_detail(c.to_string()));
            if VALID_CARDS.contains(&c.to_ascii_lowercase().as_str()) {
                f.push(Finding::new(tw, Severity::Pass, "tw.card.valid", "twitter:card-Wert gültig"));
            } else {
                f.push(
                    Finding::new(tw, Severity::Error, "tw.card.valid", "twitter:card mit ungültigem Wert")
                        .with_detail(c.to_string()),
                );
            }
        }
        None => f.push(Finding::new(tw, Severity::Warning, "tw.card.present", "twitter:card fehlt")),
    }

    // twitter:title (Fallback og:title zulässig)
    if meta.named("twitter:title").is_some() {
        f.push(Finding::new(tw, Severity::Pass, "tw.title.present", "twitter:title vorhanden"));
    } else if meta.og("og:title").is_some() {
        f.push(Finding::new(tw, Severity::Info, "tw.title.present", "twitter:title fehlt (Fallback og:title vorhanden)"));
    } else {
        f.push(Finding::new(tw, Severity::Info, "tw.title.present", "twitter:title fehlt"));
    }

    // twitter:description
    if meta.named("twitter:description").is_some() {
        f.push(Finding::new(tw, Severity::Pass, "tw.description.present", "twitter:description vorhanden"));
    } else {
        f.push(Finding::new(tw, Severity::Info, "tw.description.present", "twitter:description fehlt"));
    }

    // twitter:image (nur relevant, wenn Card-Typ ein Bild verlangt)
    let needs_image = card
        .map(|c| IMAGE_CARDS.contains(&c.to_ascii_lowercase().as_str()))
        .unwrap_or(false);
    let has_image = meta.named("twitter:image").is_some() || meta.og("og:image").is_some();
    if needs_image {
        if has_image {
            f.push(Finding::new(tw, Severity::Pass, "tw.image.present", "twitter:image (oder og:image-Fallback) vorhanden"));
        } else {
            f.push(Finding::new(
                tw,
                Severity::Warning,
                "tw.image.present",
                "twitter:image fehlt, obwohl Card-Typ ein Bild verlangt",
            ));
        }
    }

    f
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::test_support::meta_with;

    fn rule_sev(f: &[Finding], rule: &str) -> Option<Severity> {
        f.iter().find(|x| x.rule == rule).map(|x| x.severity)
    }

    #[test]
    fn no_twitter_is_just_info() {
        let m = meta_with(|_| {});
        let f = validate(&m);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, "tw.present");
    }

    #[test]
    fn invalid_card_value_is_error() {
        let m = meta_with(|m| {
            m.meta_named
                .insert("twitter:card".to_string(), vec!["bogus".to_string()]);
        });
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "tw.card.valid"), Some(Severity::Error));
    }

    #[test]
    fn image_card_without_image_warns() {
        let m = meta_with(|m| {
            m.meta_named
                .insert("twitter:card".to_string(), vec!["summary_large_image".to_string()]);
        });
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "tw.image.present"), Some(Severity::Warning));
    }
}
