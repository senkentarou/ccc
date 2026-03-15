#!/usr/bin/env bash
set -euo pipefail

REPO="masahirosenda/ccc"
INSTALL_DIR="/usr/local/bin"
BINARY_NAME="ccc"

info() { printf '\033[1;34m%s\033[0m\n' "$*"; }
error() { printf '\033[1;31merror: %s\033[0m\n' "$*" >&2; exit 1; }

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Darwin) os="apple-darwin" ;;
    Linux)  os="unknown-linux-gnu" ;;
    *)      error "Unsupported OS: $os" ;;
  esac

  case "$arch" in
    x86_64)  arch="x86_64" ;;
    aarch64|arm64) arch="aarch64" ;;
    *)       error "Unsupported architecture: $arch" ;;
  esac

  echo "${arch}-${os}"
}

install_from_release() {
  local target="$1"
  local tmpdir
  tmpdir="$(mktemp -d)"
  trap 'rm -rf "$tmpdir"' EXIT

  local archive="ccc-${target}.tar.gz"
  local url="https://github.com/${REPO}/releases/latest/download/${archive}"

  info "Downloading ${archive} ..."
  if command -v curl &>/dev/null; then
    curl -fsSL "$url" -o "${tmpdir}/${archive}"
  elif command -v wget &>/dev/null; then
    wget -q "$url" -O "${tmpdir}/${archive}"
  else
    error "curl or wget is required"
  fi

  tar xzf "${tmpdir}/${archive}" -C "$tmpdir"

  info "Installing to ${INSTALL_DIR}/${BINARY_NAME} ..."
  install -d "$INSTALL_DIR"
  install -m 755 "${tmpdir}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"

  info "Done! Run 'ccc' to get started."
}

install_from_source() {
  info "Building from source with cargo ..."
  cargo build --release
  local bin="target/release/${BINARY_NAME}"
  [ -f "$bin" ] || error "Build succeeded but binary not found at ${bin}"

  info "Installing to ${INSTALL_DIR}/${BINARY_NAME} ..."
  install -d "$INSTALL_DIR"
  install -m 755 "$bin" "${INSTALL_DIR}/${BINARY_NAME}"

  info "Done! Run 'ccc' to get started."
}

main() {
  local target
  target="$(detect_target)"
  info "Detected target: ${target}"

  # If run from repo root with Cargo.toml, try local build first
  if [ -f "Cargo.toml" ]; then
    if command -v cargo &>/dev/null; then
      install_from_source
      return
    fi
    info "Rust toolchain not found. Falling back to download ..."
  fi

  install_from_release "$target"
}

main
