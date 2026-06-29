//! schema.org / JSON-LD-Validierung (`PLAN.md §6.4`). Regel-IDs `ld.*`.
//! Erweiterbares Typ-Register: ein neuer Typ = eine neue Zeile in `REGISTRY`.
//! Bild-Erreichbarkeit (`ld.image.reachable`) läuft separat in `images.rs`.

use serde_json::{Map, Value};

use crate::model::{Category, Finding, PageMetadata, Severity};

/// Pflicht-/Empfehlungs-Spezifikation für einen bekannten `@type`.
pub struct TypeSpec {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub required: &'static [&'static str],
    pub recommended: &'static [&'static str],
}

pub static REGISTRY: &[TypeSpec] = &[
    TypeSpec {
        name: "Article",
        aliases: &["NewsArticle", "BlogPosting"],
        required: &["headline"],
        recommended: &["author", "datePublished", "image"],
    },
    TypeSpec {
        name: "Product",
        aliases: &[],
        required: &["name"],
        recommended: &["image", "offers", "description"],
    },
    TypeSpec {
        name: "Organization",
        aliases: &[],
        required: &["name"],
        recommended: &["url", "logo"],
    },
    TypeSpec {
        name: "WebSite",
        aliases: &[],
        required: &["name", "url"],
        recommended: &["potentialAction"],
    },
    TypeSpec {
        name: "BreadcrumbList",
        aliases: &[],
        required: &["itemListElement"],
        recommended: &[],
    },
    TypeSpec {
        name: "Person",
        aliases: &[],
        required: &["name"],
        recommended: &["url"],
    },
    TypeSpec {
        name: "Event",
        aliases: &[],
        required: &["name", "startDate"],
        recommended: &["location", "endDate"],
    },
];

fn lookup(ty: &str) -> Option<&'static TypeSpec> {
    REGISTRY.iter().find(|spec| {
        spec.name.eq_ignore_ascii_case(ty)
            || spec.aliases.iter().any(|a| a.eq_ignore_ascii_case(ty))
    })
}

pub fn validate(meta: &PageMetadata) -> Vec<Finding> {
    let ld = Category::SchemaOrg;
    let mut f = Vec::new();

    // Kaputte Blöcke (§8: Rest läuft weiter).
    for err in &meta.json_ld_errors {
        let snippet: String = err.chars().take(80).collect();
        f.push(
            Finding::new(ld, Severity::Error, "ld.json.valid", "Ungültiges JSON-LD")
                .with_detail(snippet),
        );
    }

    if meta.json_ld.is_empty() {
        if meta.json_ld_errors.is_empty() {
            f.push(Finding::new(ld, Severity::Info, "ld.present", "Kein JSON-LD vorhanden"));
        }
        return f;
    }
    f.push(Finding::new(ld, Severity::Pass, "ld.json.valid", "JSON-LD-Blöcke sind gültiges JSON"));

    // Knoten flach ziehen (Top-Level-Arrays + @graph auflösen).
    let mut nodes: Vec<&Map<String, Value>> = Vec::new();
    let mut ctx_seen = false;
    for value in &meta.json_ld {
        collect_nodes(value, &mut nodes, &mut ctx_seen);
    }

    // @context
    if ctx_seen {
        f.push(Finding::new(ld, Severity::Pass, "ld.context.present", "@context referenziert schema.org"));
    } else {
        f.push(Finding::new(ld, Severity::Warning, "ld.context.present", "@context referenziert nicht schema.org"));
    }

    // @type vorhanden?
    let any_type = nodes.iter().any(|n| !types_of(n).is_empty());
    if any_type {
        f.push(Finding::new(ld, Severity::Pass, "ld.type.present", "@type vorhanden"));
    } else {
        f.push(Finding::new(ld, Severity::Warning, "ld.type.present", "Kein @type in den JSON-LD-Knoten"));
    }

    // Pflicht-/Empfehlungs-Props je bekanntem Typ.
    for node in &nodes {
        for ty in types_of(node) {
            let Some(spec) = lookup(&ty) else { continue };
            let mut all_required = true;
            for &prop in spec.required {
                if !has_prop(node, prop) {
                    all_required = false;
                    f.push(
                        Finding::new(
                            ld,
                            Severity::Error,
                            "ld.required_props",
                            format!("{}: Pflicht-Property '{prop}' fehlt", spec.name),
                        )
                        .with_detail(ty.clone()),
                    );
                }
            }
            for &prop in spec.recommended {
                if !has_prop(node, prop) {
                    f.push(Finding::new(
                        ld,
                        Severity::Warning,
                        "ld.required_props",
                        format!("{}: empfohlene Property '{prop}' fehlt", spec.name),
                    ));
                }
            }
            if all_required {
                f.push(
                    Finding::new(ld, Severity::Pass, "ld.required_props", format!("{}: Pflicht-Properties vollständig", spec.name))
                        .with_detail(ty.clone()),
                );
            }
        }
    }

    f
}

fn collect_nodes<'a>(v: &'a Value, nodes: &mut Vec<&'a Map<String, Value>>, ctx_seen: &mut bool) {
    match v {
        Value::Array(arr) => arr.iter().for_each(|it| collect_nodes(it, nodes, ctx_seen)),
        Value::Object(map) => {
            if let Some(ctx) = map.get("@context")
                && context_has_schema_org(ctx)
            {
                *ctx_seen = true;
            }
            if let Some(Value::Array(graph)) = map.get("@graph") {
                graph.iter().for_each(|it| collect_nodes(it, nodes, ctx_seen));
                if map.contains_key("@type") {
                    nodes.push(map);
                }
            } else {
                nodes.push(map);
            }
        }
        _ => {}
    }
}

fn context_has_schema_org(v: &Value) -> bool {
    match v {
        Value::String(s) => s.contains("schema.org"),
        Value::Array(a) => a.iter().any(context_has_schema_org),
        Value::Object(o) => o.values().any(context_has_schema_org),
        _ => false,
    }
}

/// Liefert die `@type`-Werte eines Knotens (String oder Array).
pub fn types_of(map: &Map<String, Value>) -> Vec<String> {
    match map.get("@type") {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        _ => vec![],
    }
}

fn has_prop(map: &Map<String, Value>, prop: &str) -> bool {
    match map.get(prop) {
        None | Some(Value::Null) => false,
        Some(Value::String(s)) => !s.trim().is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::test_support::meta_with;

    fn rule_count(f: &[Finding], rule: &str, sev: Severity) -> usize {
        f.iter().filter(|x| x.rule == rule && x.severity == sev).count()
    }

    #[test]
    fn missing_required_prop_is_error() {
        let m = meta_with(|m| {
            m.json_ld
                .push(serde_json::json!({"@context":"https://schema.org","@type":"Article"}));
        });
        let f = validate(&m);
        // headline fehlt → Error
        assert_eq!(rule_count(&f, "ld.required_props", Severity::Error), 1);
        assert_eq!(rule_count(&f, "ld.context.present", Severity::Pass), 1);
    }

    #[test]
    fn graph_is_flattened() {
        let m = meta_with(|m| {
            m.json_ld.push(serde_json::json!({
                "@context":"https://schema.org",
                "@graph":[
                    {"@type":"WebSite","name":"X","url":"https://x.de"},
                    {"@type":"Organization","name":"Org","url":"https://x.de","logo":"https://x.de/l.png"}
                ]
            }));
        });
        let f = validate(&m);
        // WebSite + Organization beide vollständig → 2 Pass
        assert_eq!(rule_count(&f, "ld.required_props", Severity::Pass), 2);
        assert_eq!(rule_count(&f, "ld.required_props", Severity::Error), 0);
    }

    #[test]
    fn alias_type_is_recognized() {
        let m = meta_with(|m| {
            m.json_ld.push(serde_json::json!({
                "@context":"https://schema.org","@type":"NewsArticle","headline":"H"
            }));
        });
        let f = validate(&m);
        assert_eq!(rule_count(&f, "ld.required_props", Severity::Pass), 1);
    }
}
