#!/usr/bin/env bash
# One-time developer setup for Linux (Debian/Ubuntu, Fedora, Arch). Also works in WSL2 (needs WSLg for the GUI).
set -euo pipefail

# At run time the window needs OpenGL/EGL (Mesa) and libxkbcommon(-x11), which are loaded
# dynamically: without libxkbcommon-x11 the app fails on X11. GTK 3 draws the macro file
# windows (R-KEY-7) and is needed to build.
echo "==> System libraries for the desktop app (OpenGL, xkbcommon, GTK 3) and builds"
if command -v apt-get >/dev/null; then
  sudo apt-get update
  sudo apt-get install -y build-essential pkg-config curl file \
    libgtk-3-dev libxkbcommon-x11-0 libgl1 libegl1
elif command -v dnf >/dev/null; then
  sudo dnf install -y gcc gcc-c++ make pkgconf-pkg-config curl file \
    gtk3-devel libxkbcommon-x11 mesa-libGL mesa-libEGL
elif command -v pacman >/dev/null; then
  sudo pacman -S --needed --noconfirm base-devel pkgconf curl file \
    gtk3 libxkbcommon-x11 mesa
else
  echo "Unsupported package manager - install a C toolchain, Mesa (GL/EGL), libxkbcommon-x11 and GTK 3 (development files) manually." >&2
  exit 1
fi

"$(dirname "$0")/setup-rust.sh"
