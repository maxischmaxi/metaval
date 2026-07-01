//! Ausgabe: pretty (farbig, owo-colors) + json (serde). JSON-Schema stabil für CI.

use std::io::{self, Write};

use serde::Serialize;

use crate::cli::{Args, Format};
use crate::fetch::FetchedPage;
use crate::model::{Category, Finding, Severity};

#[derive(Serialize)]
pub struct Summary {
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
    pub pass: usize,
}

pub fn summarize(findings: &[Finding]) -> Summary {
    let mut s = Summary {
        errors: 0,
        warnings: 0,
        info: 0,
        pass: 0,
    };
    for f in findings {
        match f.severity {
            Severity::Error => s.errors += 1,
            Severity::Warning => s.warnings += 1,
            Severity::Info => s.info += 1,
            Severity::Pass => s.pass += 1,
        }
    }
    s
}

#[derive(Serialize)]
struct JsonReport<'a> {
    url: &'a str,
    final_url: &'a str,
    status: u16,
    summary: Summary,
    findings: &'a [Finding],
}

/// Rendert den Report im gewählten Format.
pub fn render(
    args: &Args,
    page: &FetchedPage,
    findings: &[Finding],
    out: &mut impl Write,
) -> io::Result<()> {
    match args.format {
        Format::Json => render_json(page, findings, out),
        Format::Pretty => render_pretty(args.color_enabled(), page, findings, out),
    }
}

fn render_json(page: &FetchedPage, findings: &[Finding], out: &mut impl Write) -> io::Result<()> {
    let report = JsonReport {
        url: page.requested_url.as_str(),
        final_url: page.final_url.as_str(),
        status: page.status,
        summary: summarize(findings),
        findings,
    };
    serde_json::to_writer_pretty(&mut *out, &report)?;
    writeln!(out)
}

fn render_pretty(
    color: bool,
    page: &FetchedPage,
    findings: &[Finding],
    out: &mut impl Write,
) -> io::Result<()> {
    writeln!(out, "{} — {}", bold("metaval", color), page.requested_url)?;

    // Kategorien in stabiler Reihenfolge gruppieren.
    let mut categories: Vec<Category> = findings.iter().map(|f| f.category).collect();
    categories.sort_by_key(|c| c.order());
    categories.dedup();

    for cat in categories {
        writeln!(out)?;
        writeln!(out, "{}", bold(cat.title(), color))?;
        for f in findings.iter().filter(|f| f.category == cat) {
            // Farbiges Symbol, gedimmte Regel-ID, Meldung normal, Detail gedimmt —
            // so bleibt die Meldung der optische Anker jeder Zeile.
            let symbol = colorize(f.severity.symbol(), f.severity, color);
            write!(out, "  {symbol} {} — {}", dim(&f.rule, color), f.message)?;
            if let Some(detail) = &f.detail {
                write!(out, " {}", dim(&format!("({detail})"), color))?;
            }
            writeln!(out)?;
        }
    }

    let s = summarize(findings);
    writeln!(out)?;
    writeln!(
        out,
        "{} {}, {}, {}, {}",
        bold("Summary:", color),
        severity_count(s.errors, "error", "errors", Severity::Error, color),
        severity_count(s.warnings, "warning", "warnings", Severity::Warning, color),
        severity_count(s.info, "info", "info", Severity::Info, color),
        severity_count(s.pass, "OK", "OK", Severity::Pass, color),
    )?;
    writeln!(
        out,
        "{} {} {}{}{}",
        dim("Final URL:", color),
        page.final_url,
        dim("(status ", color),
        status_colored(page.status, color),
        dim(")", color),
    )?;
    Ok(())
}

/// Zähler wie "2 errors": in Severity-Farbe, wenn > 0, sonst gedimmt;
/// Singular/Plural korrekt.
fn severity_count(n: usize, singular: &str, plural: &str, sev: Severity, color: bool) -> String {
    let text = format!("{n} {}", if n == 1 { singular } else { plural });
    if n == 0 { dim(&text, color) } else { colorize(&text, sev, color) }
}

/// HTTP-Status nach Klasse eingefärbt: 2xx grün, 3xx gelb, Rest rot.
fn status_colored(status: u16, enabled: bool) -> String {
    if !enabled {
        return status.to_string();
    }
    use owo_colors::OwoColorize;
    match status {
        200..=299 => status.green().to_string(),
        300..=399 => status.yellow().to_string(),
        _ => status.red().to_string(),
    }
}

fn colorize(text: &str, sev: Severity, enabled: bool) -> String {
    if !enabled {
        return text.to_string();
    }
    use owo_colors::OwoColorize;
    match sev {
        Severity::Error => text.red().to_string(),
        Severity::Warning => text.yellow().to_string(),
        Severity::Info => text.blue().to_string(),
        Severity::Pass => text.green().to_string(),
    }
}

fn bold(text: &str, enabled: bool) -> String {
    if !enabled {
        return text.to_string();
    }
    use owo_colors::OwoColorize;
    text.bold().to_string()
}

fn dim(text: &str, enabled: bool) -> String {
    if !enabled {
        return text.to_string();
    }
    use owo_colors::OwoColorize;
    text.dimmed().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Category;

    fn finding(sev: Severity) -> Finding {
        Finding::new(Category::Baseline, sev, "x.y", "msg")
    }

    #[test]
    fn summary_counts_by_severity() {
        let f = vec![
            finding(Severity::Error),
            finding(Severity::Warning),
            finding(Severity::Warning),
            finding(Severity::Pass),
        ];
        let s = summarize(&f);
        assert_eq!(s.errors, 1);
        assert_eq!(s.warnings, 2);
        assert_eq!(s.pass, 1);
        assert_eq!(s.info, 0);
    }

    #[test]
    fn severity_count_pluralizes() {
        assert_eq!(severity_count(0, "error", "errors", Severity::Error, false), "0 errors");
        assert_eq!(severity_count(1, "warning", "warnings", Severity::Warning, false), "1 warning");
        assert_eq!(severity_count(2, "error", "errors", Severity::Error, false), "2 errors");
    }

    #[test]
    fn json_serializes_stable_keys() {
        let f = vec![Finding::new(
            Category::OpenGraph,
            Severity::Error,
            "og.image.present",
            "fehlt",
        )];
        let report = JsonReport {
            url: "https://example.com/",
            final_url: "https://example.com/",
            status: 200,
            summary: summarize(&f),
            findings: &f,
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["findings"][0]["category"], "open_graph");
        assert_eq!(json["findings"][0]["severity"], "error");
        assert_eq!(json["summary"]["errors"], 1);
    }
}
