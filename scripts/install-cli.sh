#!/usr/bin/env sh
set -eu

REPO="${LETTURA_REPO:-congqiu/lettura}"
VERSION="${LETTURA_CLI_VERSION:-latest}"
INSTALL_DIR="${LETTURA_INSTALL_DIR:-$HOME/.local/bin}"

info() { printf '==> %s\n' "$*" >&2; }
err()  { printf 'error: %s\n' "$*" >&2; exit 1; }

UNAME_S="$(uname -s | tr '[:upper:]' '[:lower:]')"
UNAME_M="$(uname -m)"
case "$UNAME_S-$UNAME_M" in
  linux-x86_64)   TARGET=x86_64-unknown-linux-gnu ;;
  linux-aarch64)  TARGET=aarch64-unknown-linux-gnu ;;
  darwin-x86_64)  TARGET=x86_64-apple-darwin ;;
  darwin-arm64)   TARGET=aarch64-apple-darwin ;;
  *) err "unsupported platform: $UNAME_S-$UNAME_M (supported: linux-x86_64, linux-aarch64, darwin-x86_64, darwin-arm64)" ;;
esac

if [ "$VERSION" = "latest" ]; then
  info "resolving latest release tag..."
  VERSION="$(curl -sSL "https://api.github.com/repos/$REPO/releases/latest" \
    | grep '"tag_name":' | head -1 | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')"
  if [ -z "$VERSION" ]; then
    err "could not resolve latest release tag from GitHub API; set LETTURA_CLI_VERSION to a specific version (e.g. v0.1.0)"
  fi
fi

ASSET="lettura-cli-${VERSION}-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${VERSION}/${ASSET}"

info "downloading ${ASSET}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

if ! curl -fsSL "$URL" -o "$TMP/$ASSET"; then
  err "download failed: $URL (check that $VERSION is a published release for $TARGET)"
fi

info "extracting..."
tar -xzf "$TMP/$ASSET" -C "$TMP"

if [ ! -f "$TMP/lettura-cli" ]; then
  err "extracted archive does not contain 'lettura-cli' binary"
fi

mkdir -p "$INSTALL_DIR"
mv "$TMP/lettura-cli" "$INSTALL_DIR/lettura-cli"
chmod +x "$INSTALL_DIR/lettura-cli"

info "installed to ${INSTALL_DIR}/lettura-cli"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) : ;;
  *) info "note: $INSTALL_DIR is not in your PATH; add it to your shell profile" ;;
esac

info "next: run 'lettura-cli login' to configure"
