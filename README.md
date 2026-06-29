# metaval — SEO Metadata Validator & Meta Tag Checker (CLI)

> A fast, single-binary **command-line SEO checker** that fetches any web page and
> validates its **meta tags**, **Open Graph**, **Twitter Cards**, **schema.org / JSON-LD
> structured data**, **hreflang**, **canonical** and **robots/indexability** — then tells
> you exactly what's missing or broken.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Made with Rust](https://img.shields.io/badge/Made%20with-Rust-orange.svg)](https://www.rust-lang.org/)

`metaval` is a **metadata validator** and **meta tag linter** for developers, SEO
engineers and CI pipelines. Point it at a URL and it reports — with clear
pass/warning/error severities — whether the page is correctly set up for **Google
Search**, social sharing previews (Facebook, LinkedIn, X/Twitter, Slack, Discord, …)
and **rich results**. It can render JavaScript-heavy **single-page apps (SPA)** with
headless Chrome, checks that every referenced image is actually reachable, and emits
**machine-readable JSON** so you can fail a build on missing or invalid metadata.

---

## Table of contents

- [Why metaval?](#why-metaval)
- [Features](#features)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Usage](#usage)
- [Example output](#example-output)
- [What it checks](#what-it-checks)
- [JSON output & CI integration](#json-output--ci-integration)
- [Exit codes](#exit-codes)
- [Rendering JavaScript / SPA pages](#rendering-javascript--spa-pages)
- [FAQ](#faq)
- [License](#license)

---

## Why metaval?

Most "SEO problems" are really just **missing or malformed metadata**: a forgotten
`<meta name="description">`, an `og:image` that 404s, a `noindex` left over from
staging, an `hreflang` typo like `en_US` instead of `en-US`, or JSON-LD that's missing
a required property for a rich result. These are invisible in the browser but quietly
cost you **search rankings** and **broken social previews**.

`metaval` makes them visible in **one command**:

- ✅ **No setup, no account, no SaaS** — a single static binary you run locally or in CI.
- ✅ **Actionable, rule-based output** — every finding has a stable rule ID and a clear message.
- ✅ **Built for automation** — stable JSON output and meaningful exit codes.
- ✅ **Handles modern sites** — optional headless-Chrome rendering for client-side apps.
- ✅ **Fast & parallel** — image reachability checks run concurrently.

---

## Features

- **Baseline SEO checks** — `<title>`, meta description (with recommended length
  ranges), `<meta charset>`, `viewport`, `<html lang>`, canonical link
  (presence, absoluteness, self-match, duplicate/conflict detection).
- **Indexability** — detects `noindex` / `nofollow` from `<meta name="robots">`,
  `<meta name="googlebot">` **and** the `X-Robots-Tag` HTTP response header, so a
  stray `noindex` can't slip into production unnoticed.
- **Open Graph validation** — `og:title`, `og:type`, `og:url`, `og:image`
  (+ dimensions, `og:image:alt`, absolute URLs), `og:description`, `og:site_name`.
- **Twitter Card validation** — `twitter:card` value validity, title/description/image
  with correct Open Graph fallbacks.
- **schema.org / JSON-LD structured data** — valid JSON, `@context`, `@type`, and
  required/recommended properties for common types (`Article`, `NewsArticle`,
  `BlogPosting`, `Product`, `Organization`, `WebSite`, `BreadcrumbList`, `Person`,
  `Event`). The type registry is trivially extensible.
- **hreflang / internationalization** — BCP-47 value validation, absolute-URL
  requirement, `x-default`, self-reference, conflicting entries, and
  canonical-consistency (a canonical pointing at a different language variant
  silently breaks hreflang).
- **Image reachability** — every `og:image`, `twitter:image`, JSON-LD image and
  favicon/apple-touch-icon is fetched (HEAD with ranged-GET fallback, up to 8 in
  parallel) and checked for a successful status **and** an `image/*` content type.
- **Headless-Chrome rendering** — `--render` executes JavaScript for SPAs that inject
  their metadata client-side.
- **Two output formats** — a colored, human-friendly report and **stable JSON** for CI.
- **CI-friendly exit codes** — choose whether warnings or only errors fail the build.

---

## Installation

### Quick install — Linux & macOS (recommended)

The install script detects your platform, downloads the latest prebuilt binary
and puts it on your `PATH`:

```sh
curl -fsSL https://raw.githubusercontent.com/maxischmaxi/metaval/main/scripts/install.sh | sh
```

What it does: detects your OS/architecture (Linux x86_64, macOS Apple Silicon or
Intel), downloads the matching release tarball from GitHub, **verifies its
SHA-256 checksum**, and installs the `metaval` binary to `/usr/local/bin` — or to
`~/.local/bin` if that isn't writable. Override the location with
`METAVAL_INSTALL_DIR`:

```sh
curl -fsSL https://raw.githubusercontent.com/maxischmaxi/metaval/main/scripts/install.sh \
  | METAVAL_INSTALL_DIR="$HOME/bin" sh
```

Prefer to read before piping into a shell? The script lives at
[`scripts/install.sh`](scripts/install.sh) — download and run it yourself, or use
one of the methods below.

### Prebuilt binaries (manual download)

Grab a tarball from the latest
[GitHub Release](https://github.com/maxischmaxi/metaval/releases/latest):

| Platform | Asset |
| --- | --- |
| Linux x86_64 | `metaval-x86_64-unknown-linux-gnu.tar.gz` |
| macOS (Apple Silicon) | `metaval-aarch64-apple-darwin.tar.gz` |
| macOS (Intel) | `metaval-x86_64-apple-darwin.tar.gz` |

```sh
tar -xzf metaval-<target>.tar.gz
sudo mv metaval /usr/local/bin/
metaval --version
```

Each tarball ships with a matching `.sha256` file — verify it with
`sha256sum -c metaval-<target>.tar.gz.sha256` (Linux) or
`shasum -a 256 -c metaval-<target>.tar.gz.sha256` (macOS).

### With cargo (from crates.io)

If you have a [Rust toolchain](https://rustup.rs/), install straight from
crates.io into `~/.cargo/bin`:

```sh
cargo install metaval
```

### Build from source

```sh
git clone https://github.com/maxischmaxi/metaval
cd metaval
cargo build --release        # binary at ./target/release/metaval
# or install it directly:
cargo install --path .
```

> Rendering JavaScript pages with `--render` additionally requires a Chrome/Chromium
> installation (autodetected, or point at it with `--chrome-path`).

---

## Quick start

```sh
metaval --url https://example.com
```

That's it — fetch the page, validate its metadata, print a report, and exit non-zero
if there are any errors.

---

## Usage

```text
Fetch and validate the metadata of a web page

Usage: metaval [OPTIONS] --url <URL>

Options:
  -u, --url <URL>              URL to check
      --render                 Render the page via headless Chrome (execute JS) instead of a plain HTTP GET
      --chrome-path <PATH>     Path to the Chrome binary (autodetected otherwise)
      --timeout <TIMEOUT>      Timeout per request in seconds [default: 20]
      --user-agent <UA>        User agent for HTTP requests (default: metaval/<version>)
      --check-images           Check reachability of linked images (default: on)
      --no-check-images        Disable the image reachability check
      --format <FORMAT>        Output format [default: pretty] [possible values: pretty, json]
      --fail-on <FAIL_ON>      Severity level at which the exit code becomes non-zero [default: error] [possible values: error, warning]
      --min-only               Check only the base/minimum set (skip OG/Twitter/schema.org)
      --insecure               Ignore TLS certificate errors
      --no-color               Disable colored output (also via NO_COLOR)
  -v, --verbose...             Increase logging verbosity (repeatable: -vv)
  -h, --help                   Print help
  -V, --version                Print version
```

### Common examples

```sh
# Validate a page
metaval --url https://example.com

# Render a JavaScript / SPA site before checking
metaval --url https://app.example.com --render

# Machine-readable output for scripts and CI
metaval --url https://example.com --format json

# Fail the build on warnings too, not just errors
metaval --url https://example.com --fail-on warning

# Only the essential baseline checks (no OG/Twitter/schema.org)
metaval --url https://example.com --min-only

# Skip the (network-heavy) image reachability checks
metaval --url https://example.com --no-check-images

# Pretend to be a real browser if a site blocks the default agent
metaval --url https://example.com --user-agent "Mozilla/5.0 (compatible; metaval)"
```

---

## Example output

```text
metaval — https://example.com/

Baseline
  ✓ base.title.present — Title present
  ✓ base.title.length — Title length within the recommended range
  ✗ base.description.present — <meta name="description"> missing or empty
  ✗ base.charset.present — No character set declared (<meta charset>)
  ✓ base.viewport.present — Viewport set
  ✓ base.lang.present — html lang set (en)
  ⚠ base.canonical.present — <link rel="canonical"> missing
  ✓ base.robots.indexable — Page is indexable (no noindex)

Internationalization (hreflang)
  ℹ hreflang.present — No hreflang alternates present (only needed for multilingual pages)

Open Graph
  ℹ og.present — No Open Graph metadata present

Twitter Cards
  ℹ tw.present — No Twitter Card metadata present

schema.org / JSON-LD
  ℹ ld.present — No JSON-LD present

Fetch
  ✓ fetch.status — HTTP status OK (200)

Summary: 2 errors, 1 warnings, 4 info, 6 OK
Final URL: https://example.com/ (status 200)
```

Severities: `✓` pass · `ℹ` info · `⚠` warning · `✗` error.

---

## What it checks

Every check has a **stable rule ID** so you can grep, filter or suppress findings
programmatically (the IDs never change between releases).

### Baseline (`base.*`)

| Rule | Checks |
| --- | --- |
| `base.title.present` | `<title>` exists and is non-empty |
| `base.title.length` | Title length within ~10–60 characters |
| `base.description.present` | `<meta name="description">` exists |
| `base.description.length` | Description length within ~50–160 characters |
| `base.charset.present` | `<meta charset>` declared |
| `base.viewport.present` | `<meta name="viewport">` set |
| `base.lang.present` | `<html lang>` set |
| `base.canonical.present` | `<link rel="canonical">` present |
| `base.canonical.absolute` | Canonical is an absolute URL |
| `base.canonical.matches` | Canonical matches the final URL |
| `base.canonical.unique` | No conflicting canonical links |
| `base.robots.indexable` | Page is not `noindex` (meta robots / googlebot / `X-Robots-Tag`) |
| `base.robots.follow` | Warns on `nofollow` |
| `base.robots.parse` | Robots directives are recognized |

### Internationalization — hreflang (`hreflang.*`)

| Rule | Checks |
| --- | --- |
| `hreflang.present` | hreflang alternates present |
| `hreflang.value.valid` | Values are valid BCP-47 (catches `en_US`, typos, …) |
| `hreflang.absolute` | hreflang URLs are absolute (Google requires it) |
| `hreflang.x_default` | `x-default` present |
| `hreflang.conflict` | No two entries map the same language to different URLs |
| `hreflang.self_reference` | Each variant references itself |
| `hreflang.canonical_consistency` | Canonical doesn't point at another language variant |

### Open Graph (`og.*`)

| Rule | Checks |
| --- | --- |
| `og.title.present` | `og:title` |
| `og.type.present` | `og:type` present and a known value |
| `og.url.present` | `og:url` present and absolute |
| `og.image.present` | `og:image` present |
| `og.image.absolute` | `og:image` is an absolute URL |
| `og.image.dimensions` | `og:image:width` / `og:image:height` set |
| `og.image.alt` | `og:image:alt` set |
| `og.image.reachable` | `og:image` actually loads |
| `og.description.present` | `og:description` (recommended) |
| `og.site_name.present` | `og:site_name` (recommended) |

### Twitter Cards (`tw.*`)

| Rule | Checks |
| --- | --- |
| `tw.card.present` | `twitter:card` present |
| `tw.card.valid` | `twitter:card` is a valid type |
| `tw.title.present` | `twitter:title` (or `og:title` fallback) |
| `tw.description.present` | `twitter:description` |
| `tw.image.present` | `twitter:image` (or `og:image` fallback) when the card needs one |
| `tw.image.reachable` | `twitter:image` actually loads |

### schema.org / JSON-LD (`ld.*`)

| Rule | Checks |
| --- | --- |
| `ld.json.valid` | JSON-LD blocks are valid JSON |
| `ld.context.present` | `@context` references schema.org |
| `ld.type.present` | `@type` present |
| `ld.required_props` | Required/recommended properties per known `@type` |
| `ld.image.reachable` | JSON-LD `image`/`logo`/`thumbnailUrl` actually loads |

### Images & Fetch (`icon.*`, `fetch.*`)

| Rule | Checks |
| --- | --- |
| `icon.reachable` | Favicons / apple-touch-icons load and are images |
| `fetch.status` | HTTP status of the page itself |
| `fetch.bot_block` | Hints when a 401/403/429/503 looks like bot protection |
| `fetch.content_type` | Response is an HTML document |
| `fetch.spa_hint` | Page looks like an SPA with no server-side metadata (try `--render`) |

---

## JSON output & CI integration

Use `--format json` for a stable, machine-readable report you can pipe into `jq`,
store as a build artifact, or assert against in tests:

```sh
metaval --url https://example.com --format json
```

```json
{
  "url": "https://example.com/",
  "final_url": "https://example.com/",
  "status": 200,
  "summary": { "errors": 2, "warnings": 1, "info": 4, "pass": 6 },
  "findings": [
    {
      "category": "baseline",
      "severity": "error",
      "rule": "base.description.present",
      "message": "<meta name=\"description\"> missing or empty",
      "detail": null
    }
  ]
}
```

Example GitHub Actions step that fails the build when metadata is broken:

```yaml
- name: Validate page metadata
  run: metaval --url https://staging.example.com --fail-on warning
```

Extract just the errors with `jq`:

```sh
metaval --url https://example.com --format json \
  | jq '.findings[] | select(.severity == "error")'
```

---

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | No findings at or above the `--fail-on` threshold (clean) |
| `1` | At least one finding at/above the threshold (`error` by default; `warning` with `--fail-on warning`) |
| `2` | Tool/fetch error — invalid URL, unreachable host, timeout, TLS or Chrome failure |

---

## Rendering JavaScript / SPA pages

Single-page apps (React, Vue, Angular, Svelte, …) often inject their `<title>`,
meta tags and JSON-LD **after** the initial HTML loads. A plain HTTP GET sees an
almost-empty document — `metaval` will flag this with `fetch.spa_hint`.

Add `--render` to drive a **headless Chrome** instance, execute the page's JavaScript,
and validate the fully rendered DOM:

```sh
metaval --url https://app.example.com --render
```

Chrome is autodetected; override the path with `--chrome-path /usr/bin/chromium`.

---

## FAQ

**Is this a replacement for Google Search Console / Lighthouse?**
No — it's complementary. `metaval` is a focused, scriptable **metadata validator** you
run *before* deploying, in CI, or against any URL on demand. It doesn't crawl your whole
site or measure performance; it tells you whether a given page's metadata is correct.

**Does it crawl my entire website?**
No. It validates one URL per run, which keeps it fast and predictable for CI.

**Does it send my data anywhere?**
No. It only makes HTTP requests to the URL you give it (and, for image checks, to the
image URLs that page references). There is no telemetry and no third-party service.

**Why are some good things shown as warnings or info, not errors?**
Severities reflect SEO impact: a missing `<title>` is an **error**, a missing
`og:description` is a **warning**, and a differing canonical is **info**. Tune what
breaks your build with `--fail-on`.

---

## License

Licensed under the [MIT License](LICENSE). © 2026 maxischmaxi.

---

<sub>
Keywords: SEO CLI tool, metadata validator, meta tag checker, Open Graph validator,
Twitter Card validator, schema.org / JSON-LD structured data checker, hreflang
validator, canonical tag checker, robots / noindex checker, meta description and title
length checker, headless Chrome SPA SEO, command-line SEO audit, CI metadata linter,
Rust SEO tool.
</sub>
