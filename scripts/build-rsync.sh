#!/usr/bin/env bash
# rsync for Parley SSH's macOS app (R-SSH-66, D58): macOS has only rsync 2.6.9 or openrsync, neither with
# --info=progress2. Built from the pinned source (checked against its SHA-256; the tarball's signature by Andrew
# Tridgell, key 9FEF112DCE19A0DC7E882CB81BB24997A8535F6F, was checked when the checksum was pinned) with its own
# zlib and popt and nothing optional, so the program needs only the system's libraries. Its licence (GPLv3) and
# where its source is go beside it. Linux uses the distribution's rsync (the .deb depends on it). Windows has the
# same version from MSYS2 (scripts/fetch-rsync-windows.py).
# Not 3.5.x yet: rsync 3.5.2's tarball (2026) is signed by a key (8EDF88F4…2C52EC2D, "Zen Dodd") that the key above
# does not certify; move on once that key is confirmed as rsync's (rsync.samba.org, the rsync mailing list).
#
# Usage: scripts/build-rsync.sh OUT_DIR [TARGET_TRIPLE]   (aarch64-apple-darwin or x86_64-apple-darwin on macOS)
set -euo pipefail

version=3.4.1
sha256=2924bcb3a1ed8b551fc101f740b9f0fe0a202b115027647cf69850d65fd88c52
url="https://download.samba.org/pub/rsync/src/rsync-$version.tar.gz"

out=${1:?"usage: $0 OUT_DIR [TARGET_TRIPLE]"}
target=${2:-}
mkdir -p "$out"
out=$(cd "$out" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

curl -fsSL -o "$work/rsync.tar.gz" "$url"
echo "$sha256  $work/rsync.tar.gz" | shasum -a 256 -c - >/dev/null
tar -xzf "$work/rsync.tar.gz" -C "$work"
cd "$work/rsync-$version"

arch=""
host=""
case $target in
  aarch64-apple-darwin) arch="-arch arm64"; host=aarch64-apple-darwin ;;
  x86_64-apple-darwin) arch="-arch x86_64"; host=x86_64-apple-darwin ;;
  "") ;;
  *) echo "$0: no rsync build for $target" >&2; exit 2 ;;
esac
# macOS 11, as the apps (scripts/package-split.sh).
if [ -n "$arch" ]; then export MACOSX_DEPLOYMENT_TARGET=11.0; fi

CFLAGS="-O2 $arch" LDFLAGS="$arch" ./configure ${host:+--host=$host} \
  --disable-openssl --disable-xxhash --disable-zstd --disable-lz4 --disable-md2man \
  --with-included-popt --with-included-zlib >"$work/configure.log" 2>&1 ||
  { tail -40 "$work/configure.log" >&2; exit 1; }
make -j4 rsync >"$work/make.log" 2>&1 || { tail -40 "$work/make.log" >&2; exit 1; }

cp rsync "$out/rsync"
cp COPYING "$out/rsync-COPYING.txt"
cat >"$out/rsync-SOURCE.txt" <<EOF
rsync $version comes with Parley SSH as a program of its own, under the GNU General Public License version 3
(rsync-COPYING.txt). Parley SSH runs it for transfers; it is not linked into Parley SSH.

Its source: $url
SHA-256: $sha256
Built with: ./configure --disable-openssl --disable-xxhash --disable-zstd --disable-lz4 --disable-md2man
            --with-included-popt --with-included-zlib
EOF
# A native build must run; a cross build is only checked to be there.
if [ -z "$host" ] || [ "$host" = "$(uname -m | sed 's/arm64/aarch64/')-apple-darwin" ]; then
  "$out/rsync" --version | head -1
fi
echo "$out/rsync"
