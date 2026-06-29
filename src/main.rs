//! metaval — Metadaten einer Webseite abrufen und validieren.

mod cli;
mod error;
mod extract;
mod fetch;
mod model;
mod progress;
mod report;
mod validate;

use std::io::{self, Write};
use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use reqwest::redirect;
use tracing_subscriber::EnvFilter;
use url::Url;

use cli::{Args, FailOn};
use error::{AppError, FetchError};
use fetch::Fetcher;
use model::{Category, Finding, PageMetadata, Severity};
use progress::Spinner;

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    init_tracing(args.verbose);

    match run(&args).await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Fehler: {e}");
            ExitCode::from(2)
        }
    }
}

/// Orchestriert den Ablauf. `Err` ⇒ Tool-/Fetch-Problem ⇒ Exit-Code 2.
async fn run(args: &Args) -> Result<ExitCode, AppError> {
    // Ungültige URL → Exit 2.
    let url = Url::parse(&args.url)
        .map_err(|e| FetchError::InvalidUrl(format!("{}: {e}", args.url)))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(FetchError::InvalidUrl(format!("nicht unterstütztes Schema: {}", url.scheme())).into());
    }

    // Seite abrufen (HTTP oder Chrome). FetchError ⇒ Exit 2.
    let fetcher = Fetcher::from_args(args)?;
    let spinner = Spinner::start(format!("Lade {url} …"), args.progress_enabled());
    let fetched = fetcher.fetch(&url).await;
    spinner.finish().await;
    let page = fetched?;

    // Synchrone Extraktion (vor jeglicher weiterer Async-Arbeit).
    let meta = extract::extract(&page);

    // Separater Client für Bild-Checks (unabhängig vom Fetch-Modus).
    let image_client = reqwest::Client::builder()
        .user_agent(args.effective_user_agent())
        .timeout(Duration::from_secs(args.timeout))
        .redirect(redirect::Policy::limited(10))
        .danger_accept_invalid_certs(args.insecure)
        .build()
        .map_err(|e| AppError::Other(format!("HTTP-Client konnte nicht gebaut werden: {e}")))?;

    let spinner = Spinner::start(
        "Prüfe Metadaten & verlinkte Bilder …",
        args.progress_enabled() && args.images_enabled(),
    );
    let mut findings = validate::run_all(&meta, args, &image_client).await;
    spinner.finish().await;
    findings.extend(fetch_level_findings(&meta, args.render));

    // Ausgabe.
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    report::render(args, &page, &findings, &mut lock).map_err(|e| AppError::Other(e.to_string()))?;
    lock.flush().ok();

    Ok(ExitCode::from(exit_from_findings(&findings, args.fail_on)))
}

/// Findings, die sich aus dem Fetch selbst ergeben (Status, Content-Type, SPA-Hinweis).
fn fetch_level_findings(meta: &PageMetadata, rendered: bool) -> Vec<Finding> {
    let mut f = Vec::new();
    let cat = Category::Fetch;

    if meta.status >= 400 {
        f.push(
            Finding::new(cat, Severity::Error, "fetch.status", "HTTP-Fehlerstatus")
                .with_detail(meta.status.to_string()),
        );
        // Typische Bot-Schutz-/Rate-Limit-Antworten → konkreter Lösungshinweis.
        if matches!(meta.status, 401 | 403 | 429 | 503) {
            f.push(Finding::new(
                cat,
                Severity::Info,
                "fetch.bot_block",
                "Status deutet auf Bot-Schutz/Rate-Limit hin — mit --user-agent oder --render erneut versuchen",
            ));
        }
    } else {
        f.push(
            Finding::new(cat, Severity::Pass, "fetch.status", "HTTP-Status OK")
                .with_detail(meta.status.to_string()),
        );
    }

    if !meta.is_html() {
        f.push(
            Finding::new(cat, Severity::Error, "fetch.content_type", "Kein HTML-Dokument")
                .with_detail(meta.content_type.clone().unwrap_or_default()),
        );
    }

    // SPA-Hinweis nur für (vermeintliche) HTML-Dokumente — bei Nicht-HTML
    // (z. B. text/plain, JSON) erklärt bereits `fetch.content_type` die leere Extraktion.
    if meta.looks_like_spa && !rendered && meta.is_html() {
        f.push(Finding::new(
            cat,
            Severity::Warning,
            "fetch.spa_hint",
            "Seite wirkt wie eine SPA ohne serverseitige Metadaten — ggf. --render nutzen",
        ));
    }

    f
}

/// Exit-Code aus Findings + `--fail-on`-Schwelle.
fn exit_from_findings(findings: &[Finding], fail_on: FailOn) -> u8 {
    let threshold = match fail_on {
        FailOn::Error => Severity::Error,
        FailOn::Warning => Severity::Warning,
    };
    if findings.iter().any(|f| f.severity.at_least(threshold)) {
        1
    } else {
        0
    }
}

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("metaval={level}")));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(io::stderr)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::Category;

    fn f(sev: Severity) -> Finding {
        Finding::new(Category::Baseline, sev, "x", "m")
    }

    #[test]
    fn fail_on_error_ignores_warnings() {
        let findings = vec![f(Severity::Warning), f(Severity::Info), f(Severity::Pass)];
        assert_eq!(exit_from_findings(&findings, FailOn::Error), 0);
    }

    #[test]
    fn fail_on_error_catches_errors() {
        let findings = vec![f(Severity::Error), f(Severity::Pass)];
        assert_eq!(exit_from_findings(&findings, FailOn::Error), 1);
    }

    #[test]
    fn fail_on_warning_catches_warnings() {
        let findings = vec![f(Severity::Warning)];
        assert_eq!(exit_from_findings(&findings, FailOn::Warning), 1);
        let clean = vec![f(Severity::Info), f(Severity::Pass)];
        assert_eq!(exit_from_findings(&clean, FailOn::Warning), 0);
    }
}
