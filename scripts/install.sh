#!/bin/sh
# metaval installer — downloads the latest prebuilt binary for your platform
# (Linux or macOS) and installs it.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/maxischmaxi/metaval/main/scripts/install.sh | sh
#
# Environment:
#   METAVAL_INSTALL_DIR   Where to install the binary. Defaults to
#                         /usr/local/bin if writable, otherwise $HOME/.local/bin.
set -eu

REPO="maxischmaxi/metaval"
BIN="metaval"

err() { echo "metaval-install: error: $*" >&2; exit 1; }
info() { echo "metaval-install: $*" >&2; }

# --- detect platform -> Rust target triple -----------------------------------
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Linux)
    case "$arch" in
      x86_64 | amd64) target="x86_64-unknown-linux-gnu" ;;
      *) err "unsupported Linux architecture '$arch' (only x86_64 is published)" ;;
    esac
    ;;
  Darwin)
    case "$arch" in
      arm64 | aarch64) target="aarch64-apple-darwin" ;;
      x86_64) target="x86_64-apple-darwin" ;;
      *) err "unsupported macOS architecture '$arch'" ;;
    esac
    ;;
  *) err "unsupported OS '$os' (this installer supports Linux and macOS)" ;;
esac

asset="${BIN}-${target}.tar.gz"
url="https://github.com/${REPO}/releases/latest/download/${asset}"

# --- pick a downloader --------------------------------------------------------
if command -v curl >/dev/null 2>&1; then
  dl() { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
  dl() { wget -qO "$2" "$1"; }
else
  err "need either 'curl' or 'wget' on PATH"
fi
command -v tar >/dev/null 2>&1 || err "need 'tar' on PATH"

# --- download into a temp dir -------------------------------------------------
tmp="$(mktemp -d "${TMPDIR:-/tmp}/metaval.XXXXXXXX")"
trap 'rm -rf "$tmp"' EXIT INT TERM

info "downloading ${asset} (latest)…"
dl "$url" "$tmp/$asset" || err "download failed: $url"

# --- verify checksum (best effort: only if the .sha256 asset exists) ----------
if dl "${url}.sha256" "$tmp/${asset}.sha256" 2>/dev/null; then
  (
    cd "$tmp"
    if command -v sha256sum >/dev/null 2>&1; then
      sha256sum -c "${asset}.sha256" >/dev/null 2>&1 || err "checksum verification failed"
    elif command -v shasum >/dev/null 2>&1; then
      shasum -a 256 -c "${asset}.sha256" >/dev/null 2>&1 || err "checksum verification failed"
    fi
  )
  info "checksum OK"
fi

# --- extract ------------------------------------------------------------------
tar -xzf "$tmp/$asset" -C "$tmp"
[ -f "$tmp/$BIN" ] || err "archive did not contain the expected '$BIN' binary"
chmod +x "$tmp/$BIN"

# --- choose install dir -------------------------------------------------------
if [ -n "${METAVAL_INSTALL_DIR:-}" ]; then
  dir="$METAVAL_INSTALL_DIR"
  mkdir -p "$dir"
elif [ -w /usr/local/bin ]; then
  dir="/usr/local/bin"
else
  dir="$HOME/.local/bin"
  mkdir -p "$dir"
fi

mv "$tmp/$BIN" "$dir/$BIN" || err "could not install to '$dir' (try METAVAL_INSTALL_DIR, or re-run with sudo)"
info "installed $BIN -> $dir/$BIN"

# --- PATH hint ----------------------------------------------------------------
case ":$PATH:" in
  *":$dir:"*) : ;;
  *)
    info ""
    info "note: $dir is not on your PATH. Add it, e.g.:"
    info "  export PATH=\"$dir:\$PATH\""
    ;;
esac

"$dir/$BIN" --version || true
