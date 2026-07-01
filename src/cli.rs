//! CLI-Definitionen (clap derive).

use clap::{ArgAction, Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "metaval",
    version,
    about = "Fetch and validate a web page's SEO metadata (meta tags, Open Graph, Twitter Cards, schema.org, hreflang).",
    long_about = "metaval fetches a single web page and validates the metadata that search \
engines and social platforms rely on, reporting every check as pass, info, warning or error.\n\n\
It covers baseline SEO (title, description, charset, viewport, lang, canonical, \
robots/indexability), hreflang, Open Graph, Twitter Cards, schema.org/JSON-LD structured \
data, and the reachability of every linked image. JavaScript-rendered pages can be analyzed \
with --render.\n\n\
Results print as a colored report, or as stable JSON with --format json. The process exits 0 \
when clean, 1 when findings reach the --fail-on threshold, and 2 on a fetch/tool error — so it \
drops straight into CI.",
    after_help = "Run 'metaval --help' for full option descriptions, examples and exit codes.",
    after_long_help = r#"EXAMPLES:
  Validate a page:
      metaval --url https://example.com

  Render a JavaScript / SPA site before checking:
      metaval --url https://app.example.com --render

  Machine-readable output for CI (pipe into jq):
      metaval --url https://example.com --format json

  Fail the build on warnings, not just errors:
      metaval --url https://example.com --fail-on warning

  Quick, low-noise gate (baseline only, no image requests):
      metaval --url https://example.com --min-only --no-check-images

  Pretend to be a browser when a site blocks the default agent:
      metaval --url https://example.com --user-agent "Mozilla/5.0 (compatible; metaval)"

EXIT CODES:
  0   Clean — no findings at or above the --fail-on threshold.
  1   Findings — at least one finding at/above the threshold (see --fail-on).
  2   Error — invalid URL, unreachable host, timeout, TLS or Chrome failure.

ENVIRONMENT:
  NO_COLOR    Disable colored output (same effect as --no-color).
  RUST_LOG    Override log verbosity, e.g. RUST_LOG=metaval=debug (takes
              precedence over -v)."#
)]
pub struct Args {
    /// URL to fetch and validate (http or https).
    ///
    /// The single page to analyze. metaval requests exactly this URL and
    /// follows redirects, so the report's "Final URL" may differ from what
    /// you pass here. Only http:// and https:// are accepted; any other
    /// scheme exits with code 2.
    #[arg(short, long, value_name = "URL")]
    pub url: String,

    // ── Fetching ────────────────────────────────────────────────────────────
    /// Render with headless Chrome (run JavaScript) instead of a plain HTTP GET.
    ///
    /// Use this for single-page apps (React, Vue, Angular, …) that inject their
    /// <title>, meta tags or JSON-LD on the client. metaval launches headless
    /// Chrome, lets the page's scripts run, and validates the final rendered
    /// DOM. Requires a local Chrome/Chromium (autodetected, or set
    /// --chrome-path). Slower than the default fetch — only enable it when a
    /// plain GET comes back empty (look for the fetch.spa_hint finding).
    #[arg(long, default_value_t = false, help_heading = "Fetching")]
    pub render: bool,

    /// Path to the Chrome/Chromium binary used by --render (autodetected otherwise).
    ///
    /// Only relevant together with --render. Point this at a specific browser
    /// build if autodetection fails or you need a particular version, e.g.
    /// --chrome-path /usr/bin/chromium.
    #[arg(long, value_name = "PATH", help_heading = "Fetching")]
    pub chrome_path: Option<String>,

    /// Per-request timeout in seconds.
    ///
    /// Applies to the page fetch and to each individual image-reachability
    /// request, not to the run as a whole. Raise it for slow origins; lower it
    /// to fail fast in CI.
    #[arg(
        long,
        default_value_t = 20,
        value_name = "SECONDS",
        value_parser = clap::value_parser!(u64).range(1..),
        help_heading = "Fetching"
    )]
    pub timeout: u64,

    /// User-Agent header for all HTTP requests [default: metaval/<version>].
    ///
    /// Some sites block unknown agents with 401/403/429 (you'll see a
    /// fetch.bot_block hint). Set a browser-like value to get through, e.g.
    /// --user-agent "Mozilla/5.0 (compatible; metaval)". Also used for the
    /// image-reachability requests, and passed to Chrome with --render.
    #[arg(long, value_name = "STRING", help_heading = "Fetching")]
    pub user_agent: Option<String>,

    /// Accept invalid / self-signed TLS certificates (insecure).
    ///
    /// Disables certificate verification for both the page fetch and the image
    /// checks. Use only for local or staging hosts you trust — it removes
    /// protection against man-in-the-middle attacks.
    #[arg(long, default_value_t = false, help_heading = "Fetching")]
    pub insecure: bool,

    // ── Checks ──────────────────────────────────────────────────────────────
    /// Check that linked images are reachable (this is the default).
    ///
    /// metaval resolves every og:image, twitter:image, JSON-LD
    /// image/logo/thumbnailUrl and favicon/apple-touch-icon, then verifies each
    /// returns a successful status with an image/* content type (HEAD, with a
    /// ranged-GET fallback; up to 8 in parallel). Image checking is already on
    /// by default — this flag is only needed to re-enable it when
    /// --no-check-images is also present.
    #[arg(
        long = "check-images",
        action = ArgAction::SetTrue,
        overrides_with = "no_check_images",
        help_heading = "Checks"
    )]
    check_images: bool,

    /// Skip the image-reachability checks (faster, fewer network requests).
    ///
    /// Suppresses all og.image.reachable / tw.image.reachable /
    /// ld.image.reachable / icon.reachable findings and the network calls
    /// behind them. Useful offline, in rate-limited environments, or when you
    /// only care about the tags themselves.
    #[arg(
        long = "no-check-images",
        action = ArgAction::SetTrue,
        overrides_with = "check_images",
        help_heading = "Checks"
    )]
    no_check_images: bool,

    /// Run only the baseline checks; skip Open Graph, Twitter Cards and schema.org.
    ///
    /// Keeps the core SEO essentials (title, description, charset, viewport,
    /// lang, canonical, robots/indexability) plus hreflang and the fetch
    /// status, and drops the social/structured-data validators. Image checks
    /// are reduced to favicons. Handy for a quick, low-noise gate.
    #[arg(long, default_value_t = false, help_heading = "Checks")]
    pub min_only: bool,

    // ── Output ──────────────────────────────────────────────────────────────
    /// Output format.
    ///
    /// "pretty" is a colored, human-readable report grouped by category.
    /// "json" is a stable, machine-readable document (url, final_url, status,
    /// summary, findings[]) intended for CI and tooling — its keys and rule
    /// IDs do not change between releases.
    #[arg(long, value_enum, default_value_t = Format::Pretty, help_heading = "Output")]
    pub format: Format,

    /// Severity at which the exit code becomes non-zero.
    ///
    /// With "error" only errors fail the run (exit 1); with "warning",
    /// warnings fail too. Info and pass never affect the exit code. A
    /// fetch/tool failure always exits with code 2, regardless of this setting.
    #[arg(long, value_enum, default_value_t = FailOn::Error, value_name = "SEVERITY", help_heading = "Output")]
    pub fail_on: FailOn,

    /// Disable colored output (also honored via the NO_COLOR env var).
    ///
    /// Color is auto-disabled when stdout is not a TTY. Turning it off also
    /// disables the progress spinner. JSON output is never colored.
    #[arg(long, default_value_t = false, help_heading = "Output")]
    pub no_color: bool,

    /// Increase logging verbosity to stderr (repeatable: -v, -vv, -vvv).
    ///
    /// Default logs warnings only. -v = info, -vv = debug, -vvv = trace. Logs
    /// go to stderr and never pollute the report on stdout. RUST_LOG overrides
    /// this (e.g. RUST_LOG=metaval=debug). Any -v also disables the progress
    /// spinner so log lines don't collide with it.
    #[arg(short, long, action = ArgAction::Count, help_heading = "Output")]
    pub verbose: u8,
}

impl Args {
    /// Bild-Checks aktiv? Default an; `--no-check-images` schaltet ab.
    pub fn images_enabled(&self) -> bool {
        !self.no_check_images
    }

    /// Effektiver User-Agent (Override oder Default mit Crate-Version).
    pub fn effective_user_agent(&self) -> String {
        self.user_agent
            .clone()
            .unwrap_or_else(|| format!("metaval/{}", env!("CARGO_PKG_VERSION")))
    }

    /// Farbe per Flag/Env erlaubt? (`--no-color` und `NO_COLOR` deaktivieren.)
    fn color_allowed_by_flags(&self) -> bool {
        !self.no_color && std::env::var_os("NO_COLOR").is_none()
    }

    /// Farbe für den Report aktiv? Zusätzlich zu Flag/Env muss stdout ein
    /// Terminal sein — Pipes und Dateien bekommen nie ANSI-Codes.
    pub fn color_enabled(&self) -> bool {
        use std::io::IsTerminal;
        self.color_allowed_by_flags() && std::io::stdout().is_terminal()
    }

    /// Lade-Animation erlaubt? Aus bei `--no-color`/`NO_COLOR` und bei `-v`
    /// (dann würden Log-Zeilen mit dem Spinner kollidieren). Der Spinner läuft
    /// auf stderr und prüft dessen TTY selbst — stdout darf gepiped sein.
    pub fn progress_enabled(&self) -> bool {
        self.color_allowed_by_flags() && self.verbose == 0
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// Colored, human-readable report grouped by category.
    Pretty,
    /// Stable JSON for CI and tooling (url, final_url, status, summary, findings).
    Json,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum FailOn {
    /// Only error-level findings make the exit code non-zero.
    Error,
    /// Warning- and error-level findings make the exit code non-zero.
    Warning,
}
