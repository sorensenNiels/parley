#!/usr/bin/env bash
# One-time developer setup for macOS. The app draws with OpenGL and uses native dialogs, so only Xcode CLT is needed.
set -euo pipefail

if ! xcode-select -p >/dev/null 2>&1; then
  echo "==> Installing Xcode Command Line Tools (finish the dialog, then re-run this script)"
  xcode-select --install
  exit 0
fi

if ! /usr/libexec/java_home -v 11+ >/dev/null 2>&1; then
  echo "==> Installing a JDK for the SQL helper (D20)"
  if command -v brew >/dev/null; then
    brew install --cask temurin@17
  else
    echo "Install a JDK 11 or later (https://adoptium.net), then re-run this script." >&2
    exit 1
  fi
fi

"$(dirname "$0")/setup-rust.sh"
