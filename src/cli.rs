//! CLI-Definitionen (clap derive). Flags exakt nach `PLAN.md §2`.

use clap::{ArgAction, Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "metaval", version, about = "Fetch and validate the metadata of a web page")]
pub struct Args {
    /// URL to check.
    #[arg(short, long)]
    pub url: String,

    /// Render the page via headless Chrome (execute JS) instead of a plain HTTP GET.
    #[arg(long, default_value_t = false)]
    pub render: bool,

    /// Path to the Chrome binary (autodetected otherwise).
    #[arg(long)]
    pub chrome_path: Option<String>,

    /// Timeout per request in seconds.
    #[arg(long, default_value_t = 20)]
    pub timeout: u64,

    /// User agent for HTTP requests (default: `metaval/<version>`).
    #[arg(long)]
    pub user_agent: Option<String>,

    /// Check reachability of linked images (default: on).
    #[arg(long = "check-images", action = ArgAction::SetTrue, overrides_with = "no_check_images")]
    check_images: bool,

    /// Disable the image reachability check.
    #[arg(long = "no-check-images", action = ArgAction::SetTrue, overrides_with = "check_images")]
    no_check_images: bool,

    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Pretty)]
    pub format: Format,

    /// Severity level at which the exit code becomes non-zero.
    #[arg(long, value_enum, default_value_t = FailOn::Error)]
    pub fail_on: FailOn,

    /// Check only the base/minimum set (skip OG/Twitter/schema.org).
    #[arg(long, default_value_t = false)]
    pub min_only: bool,

    /// Ignore TLS certificate errors.
    #[arg(long, default_value_t = false)]
    pub insecure: bool,

    /// Disable colored output (also via NO_COLOR).
    #[arg(long, default_value_t = false)]
    pub no_color: bool,

    /// Increase logging verbosity (repeatable: -vv).
    #[arg(short, long, action = ArgAction::Count)]
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

    /// Farbe aktiv? `--no-color` und die `NO_COLOR`-Env deaktivieren sie.
    pub fn color_enabled(&self) -> bool {
        !self.no_color && std::env::var_os("NO_COLOR").is_none()
    }

    /// Lade-Animation erlaubt? Aus bei `--no-color`/`NO_COLOR` und bei `-v`
    /// (dann würden Log-Zeilen mit dem Spinner kollidieren). Die zusätzliche
    /// TTY-Prüfung passiert im Spinner selbst.
    pub fn progress_enabled(&self) -> bool {
        self.color_enabled() && self.verbose == 0
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Pretty,
    Json,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum FailOn {
    Error,
    Warning,
}
