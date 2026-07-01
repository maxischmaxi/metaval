//! Twitter-Card-Validierung. Regel-IDs `tw.*`.
//! Konsistent zu OG: ohne jegliches `twitter:`-Tag nur ein Info, sonst volle Prüfung.

use crate::model::{Category, Finding, PageMetadata, Severity};

const VALID_CARDS: &[&str] = &["summary", "summary_large_image", "app", "player"];
/// Card-Typen, die ein Bild verlangen.
const IMAGE_CARDS: &[&str] = &["summary", "summary_large_image", "player"];

pub fn validate(meta: &PageMetadata) -> Vec<Finding> {
    let tw = Category::Twitter;
    let mut f = Vec::new();

    if !meta.has_twitter() {
        f.push(Finding::new(tw, Severity::Info, "tw.present", "No Twitter Card metadata present"));
        return f;
    }

    // twitter:card
    let card = meta.twitter("twitter:card");
    match card {
        Some(c) => {
            f.push(Finding::new(tw, Severity::Pass, "tw.card.present", "twitter:card present").with_detail(c.to_string()));
            if VALID_CARDS.contains(&c.to_ascii_lowercase().as_str()) {
                f.push(Finding::new(tw, Severity::Pass, "tw.card.valid", "twitter:card value valid"));
            } else {
                f.push(
                    Finding::new(tw, Severity::Error, "tw.card.valid", "twitter:card with invalid value")
                        .with_detail(c.to_string()),
                );
            }
        }
        None => f.push(Finding::new(tw, Severity::Warning, "tw.card.present", "twitter:card missing")),
    }

    // twitter:title (Fallback og:title zulässig)
    if meta.twitter("twitter:title").is_some() {
        f.push(Finding::new(tw, Severity::Pass, "tw.title.present", "twitter:title present"));
    } else if meta.og("og:title").is_some() {
        f.push(Finding::new(tw, Severity::Info, "tw.title.present", "twitter:title missing (og:title fallback present)"));
    } else {
        f.push(Finding::new(tw, Severity::Info, "tw.title.present", "twitter:title missing"));
    }

    // twitter:description
    if meta.twitter("twitter:description").is_some() {
        f.push(Finding::new(tw, Severity::Pass, "tw.description.present", "twitter:description present"));
    } else {
        f.push(Finding::new(tw, Severity::Info, "tw.description.present", "twitter:description missing"));
    }

    // twitter:image (nur relevant, wenn Card-Typ ein Bild verlangt)
    let needs_image = card
        .map(|c| IMAGE_CARDS.contains(&c.to_ascii_lowercase().as_str()))
        .unwrap_or(false);
    let has_image = meta.twitter("twitter:image").is_some() || meta.og("og:image").is_some();
    if needs_image {
        if has_image {
            f.push(Finding::new(tw, Severity::Pass, "tw.image.present", "twitter:image (or og:image fallback) present"));
        } else {
            f.push(Finding::new(
                tw,
                Severity::Warning,
                "tw.image.present",
                "twitter:image missing although the card type requires an image",
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
    fn twitter_via_property_attribute_is_recognized() {
        let m = meta_with(|m| {
            m.meta_property
                .insert("twitter:card".to_string(), vec!["summary".to_string()]);
        });
        let f = validate(&m);
        assert_eq!(rule_sev(&f, "tw.card.present"), Some(Severity::Pass));
        assert_eq!(rule_sev(&f, "tw.card.valid"), Some(Severity::Pass));
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
