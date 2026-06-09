#!/usr/bin/env bash

# Builds, tests, and bundles the app in optimized (release) mode, producing a
# native installer for the host operating system.
#
# IMPORTANT: Tauri bundles cannot be cross-compiled. A single run produces
# installers only for the OS it runs on:
#   - macOS         -> .dmg, .app
#   - Debian/Ubuntu -> .deb
#   - Fedora/RHEL   -> .rpm
# To target all three, run this script once on each operating system.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

log() {
  printf '\n==> %s\n' "$1"
}

warn() {
  printf 'warning: %s\n' "$1" >&2
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'error: required command not found: %s\n' "$1" >&2
    exit 1
  fi
}

usage() {
  cat <<'EOF'
Usage: scripts/release.sh [options]

Options:
  --bundles <list>  Comma-separated bundle types to build (e.g. deb,rpm,dmg,app,appimage).
                    Defaults to the right type(s) for the host OS.
  --no-clean        Skip `cargo clean` (faster, but less reproducible).
  --skip-tests      Skip the test suite.
  -h, --help        Show this help.
EOF
}

# --- Parse arguments --------------------------------------------------------
CLEAN=1
RUN_TESTS=1
BUNDLES=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bundles)
      BUNDLES="${2:-}"
      if [[ -z "$BUNDLES" ]]; then
        printf 'error: --bundles requires a value\n' >&2
        exit 1
      fi
      shift 2
      ;;
    --no-clean)
      CLEAN=0
      shift
      ;;
    --skip-tests)
      RUN_TESTS=0
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      printf 'error: unknown argument: %s\n' "$1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

# --- Detect the default bundle types for this host --------------------------
detect_default_bundles() {
  case "$(uname -s)" in
    Darwin)
      echo "dmg,app"
      ;;
    Linux)
      local id="" id_like=""
      if [[ -r /etc/os-release ]]; then
        # shellcheck disable=SC1091
        . /etc/os-release
        id="${ID:-}"
        id_like="${ID_LIKE:-}"
      fi
      case "$id $id_like" in
        *debian* | *ubuntu*)
          echo "deb"
          ;;
        *fedora* | *rhel* | *centos*)
          echo "rpm"
          ;;
        *)
          warn "unrecognized Linux distribution; defaulting to deb,rpm. Override with --bundles."
          echo "deb,rpm"
          ;;
      esac
      ;;
    *)
      printf 'error: unsupported host OS: %s\n' "$(uname -s)" >&2
      exit 1
      ;;
  esac
}

if [[ -z "$BUNDLES" ]]; then
  BUNDLES="$(detect_default_bundles)"
fi

# --- Verify toolchain -------------------------------------------------------
require_command cargo
require_command dx

if ! cargo tauri --version >/dev/null 2>&1; then
  printf 'error: required cargo subcommand not found: cargo tauri\n' >&2
  printf 'hint: install it with: cargo install tauri-cli\n' >&2
  exit 1
fi

# Soft checks for distro-specific bundler dependencies.
case ",$BUNDLES," in
  *,rpm,*) command -v rpmbuild >/dev/null 2>&1 || warn "rpmbuild not found; .rpm bundling may fail (install the rpm-build package)." ;;
esac
case ",$BUNDLES," in
  *,deb,*) command -v dpkg-deb >/dev/null 2>&1 || warn "dpkg-deb not found; .deb bundling may fail (install the dpkg package)." ;;
esac

log "Host: $(uname -s) $(uname -m) | bundles: $BUNDLES"

# --- Clean ------------------------------------------------------------------
if [[ "$CLEAN" -eq 1 ]]; then
  log "Clean build artifacts"
  cargo clean
fi

# --- Test --------------------------------------------------------------------
if [[ "$RUN_TESTS" -eq 1 ]]; then
  log "Run workspace tests"
  cargo test --workspace
fi

# --- Build & bundle ----------------------------------------------------------
# Run from the crate that owns tauri.conf.json so the CLI resolves the config
# and its relative beforeBuildCommand / frontendDist paths correctly.
log "Build and bundle (release)"
cd "$ROOT_DIR/presentation_backend"
IFS=',' read -r -a BUNDLE_ARGS <<<"$BUNDLES"
cargo tauri build --bundles "${BUNDLE_ARGS[@]}"

# --- Report ------------------------------------------------------------------
BUNDLE_DIR="$ROOT_DIR/target/release/bundle"
log "Done. Installers written to:"
if [[ -d "$BUNDLE_DIR" ]]; then
  find "$BUNDLE_DIR" -type f \( -name '*.dmg' -o -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) -print
else
  warn "bundle directory not found: $BUNDLE_DIR"
fi
