#!/usr/bin/env bash
# One-time developer setup for Linux (Debian/Ubuntu, Fedora, Arch). Also works in WSL2 (needs WSLg for the GUI).
set -euo pipefail

# At run time the window needs OpenGL/EGL (Mesa) and libxkbcommon(-x11), which are loaded
# dynamically: without libxkbcommon-x11 the app fails on X11. GTK 3 draws the macro file
# windows (R-KEY-7) and is needed to build.
echo "==> System libraries for the desktop app (OpenGL, xkbcommon, GTK 3) and builds, and a JDK for the SQL helper (D20)"
if command -v apt-get >/dev/null; then
  sudo apt-get update
  sudo apt-get install -y build-essential pkg-config curl file \
    libgtk-3-dev libasound2-dev libxkbcommon-x11-0 libgl1 libegl1 openjdk-17-jdk-headless
elif command -v dnf >/dev/null; then
  sudo dnf install -y gcc gcc-c++ make pkgconf-pkg-config curl file \
    gtk3-devel alsa-lib-devel libxkbcommon-x11 mesa-libGL mesa-libEGL java-17-openjdk-devel
elif command -v pacman >/dev/null; then
  sudo pacman -S --needed --noconfirm base-devel pkgconf curl file \
    gtk3 alsa-lib libxkbcommon-x11 mesa jdk17-openjdk
else
  echo "Unsupported package manager - install a C toolchain, Mesa (GL/EGL), libxkbcommon-x11, GTK 3 (development files) and a JDK 11+ manually." >&2
  exit 1
fi

"$(dirname "$0")/setup-rust.sh"
