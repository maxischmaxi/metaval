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
    writeln!(out, "metaval — {}", page.requested_url)?;

    // Kategorien in stabiler Reihenfolge gruppieren.
    let mut categories: Vec<Category> = findings.iter().map(|f| f.category).collect();
    categories.sort_by_key(|c| c.order());
    categories.dedup();

    for cat in categories {
        writeln!(out)?;
        writeln!(out, "{}", bold(cat.title(), color))?;
        for f in findings.iter().filter(|f| f.category == cat) {
            let symbol = colorize(f.severity.symbol(), f.severity, color);
            write!(out, "  {symbol} {} — {}", f.rule, f.message)?;
            if let Some(detail) = &f.detail {
                write!(out, " ({detail})")?;
            }
            writeln!(out)?;
        }
    }

    let s = summarize(findings);
    writeln!(out)?;
    writeln!(
        out,
        "Summary: {} errors, {} warnings, {} info, {} OK",
        s.errors, s.warnings, s.info, s.pass
    )?;
    writeln!(out, "Final URL: {} (status {})", page.final_url, page.status)?;
    Ok(())
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
