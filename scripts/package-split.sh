#!/usr/bin/env bash
# Parley 5250, Parley SQL and Parley SSH as packages of their own (R-NF-4, R-NF-5, D56):
#   deb  three .deb files, each taking over the files of an earlier all-in-one package (Linux)
#   dmg  three apps in one .dmg, beside a link to Applications (macOS)
# cargo-packager reads one config per crate from Cargo.toml, so this gives it one per program on the command line.
# Windows' .msi and the AppImage still come from `[package.metadata.packager]` in crates/parley-app/Cargo.toml.
#
# Usage: scripts/package-split.sh deb|dmg [TARGET_TRIPLE]
# Needs the release binaries (`cargo build -p parley-app --release [--target …]`), target/helpers (`make helper`)
# and, for dmg, target/rsync (`scripts/build-rsync.sh target/rsync TARGET_TRIPLE`).
set -euo pipefail

format=${1:?"usage: $0 deb|dmg [TARGET_TRIPLE]"}
target=${2:-}
root=$(cd "$(dirname "$0")/.." && pwd)
app="$root/crates/parley-app"
fonts="$root/crates/parley-ui-egui/assets/fonts"
bins="$root/target/${target:+$target/}release"
out="$root/target/packages"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)
authors=$(sed -n 's/^authors = \(\[.*\]\)$/\1/p' "$root/Cargo.toml" | head -1)

case $format in
  deb) packager_format=deb; staging=$out ;;
  dmg) packager_format=app; staging="$root/target/packages-split/apps" ;;
  *) echo "usage: $0 deb|dmg [TARGET_TRIPLE]" >&2; exit 2 ;;
esac

# The bundled fonts' licence (SIL OFL 1.1) travels with every program; the SQL helper, JTOpen and their notices
# (R-SQL-13, D20) with Parley SQL only.
licences='"'"$fonts"'/Inter-LICENSE.txt", "'"$fonts"'/JetBrainsMono-LICENSE.txt", "'"$fonts"'/IBMPlexMono-LICENSE.txt", "'"$fonts"'/Phosphor-LICENSE.txt"'
helper='{"src": "'"$root"'/target/helpers/parley-sql.jar", "target": "helpers/parley-sql.jar"},
  {"src": "'"$root"'/target/helpers/jt400.jar", "target": "helpers/jt400.jar"},
  {"src": "'"$root"'/helpers/sql/NOTICE.txt", "target": "helpers/NOTICE.txt"},
  {"src": "'"$root"'/helpers/sql/JTOpen-LICENSE.html", "target": "helpers/JTOpen-LICENSE.html"}'
# rsync for Parley SSH (R-SSH-66, D58): on macOS the one scripts/build-rsync.sh built into target/rsync, with its
# licence and where its source is; on Linux the distribution's, as a dependency of the package.
rsync=""
ssh_depends=""
case $format in
  dmg) rsync='{"src": "'"$root"'/target/rsync/rsync", "target": "rsync"},
  {"src": "'"$root"'/target/rsync/rsync-COPYING.txt", "target": "rsync-COPYING.txt"},
  {"src": "'"$root"'/target/rsync/rsync-SOURCE.txt", "target": "rsync-SOURCE.txt"}' ;;
  deb) ssh_depends='"rsync"' ;;
esac
icons=$(printf '"%s/icons/%s", ' "$app" 32.png 64.png 128.png 256.png 512.png)
triple=""
[ -z "$target" ] || triple="\"targetTriple\": \"$target\","

# The .deb's dependencies: X11 keyboard and OpenGL/EGL are loaded at run time by the window library, fontconfig to
# list the installed fonts, GTK 3 by the file windows (R-KEY-7).
# One program's config: its binary, package name, product name, identifier, description, extra resources and extra
# dependencies of its .deb.
config() {
  local binary=$1 package=$2 product=$3 identifier=$4 description=$5 long=$6 extra=${7:-} depends=${8:-}
  cat <<EOF
{
  "name": "parley-app",
  "productName": "$product",
  "version": "$version",
  "identifier": "$identifier",
  "description": "$description",
  "longDescription": "$long",
  "authors": $authors,
  "homepage": "https://github.com/sorensenNiels/parley",
  "category": "Utility",
  "licenseFile": "$root/LICENSE-MIT",
  "icons": [${icons%, }],
  "outDir": "$staging",
  "binariesDir": "$bins",
  $triple
  "binaries": [{"path": "$binary", "main": true}],
  "resources": [$licences${extra:+, $extra}],
  "deb": {
    "packageName": "$package",
    "section": "net",
    "desktopTemplate": "$app/linux/$binary.desktop",
    "depends": ["libgtk-3-0", "libasound2", "libxkbcommon-x11-0", "libegl1", "libgl1", "libfontconfig1"${depends:+, $depends}]
  },
  "macos": {"minimumSystemVersion": "11.0"}
}
EOF
}

configs="[
$(config parley parley-5250 "Parley 5250" dk.softdesign.parley "A TN5250 terminal for IBM i" \
  "Parley 5250 is a fast, modern TN5250 terminal emulator for IBM i: green screens over telnet or TLS, in colour, with familiar keys."),
$(config parley-sql parley-sql "Parley SQL" dk.softdesign.parley-sql "SQL for IBM i (needs Java 11 or later)" \
  "Parley SQL runs SQL on IBM i: an editor with completion, results, the schema browser and data transfer, through IBM's JTOpen." "$helper"),
$(config parley-ssh parley-ssh "Parley SSH" dk.softdesign.parley-ssh "An SSH client" \
  "Parley SSH is an SSH client with tabs and split panes, saved sessions, snippets and the hosts of ~/.ssh/config." \
  "$rsync" "$ssh_depends")
]"

[ "$format" = deb ] || rm -rf "$staging"
mkdir -p "$staging"
cargo packager --config "$configs" -f "$packager_format"

case $format in
  deb)
    # The package `parley-5250` (and `parley` before it, D40) held all three programs until R-NF-4: each package now
    # takes over its files from those, and makes apt replace them, which cargo-packager cannot write.
    arch=$(dpkg --print-architecture)
    for binary in parley parley-sql parley-ssh; do
      deb="$out/${binary}_${version}_${arch}.deb"
      tree=$(mktemp -d)
      dpkg-deb -R "$deb" "$tree"
      chmod 755 "$tree"
      old=""
      [ "$binary" = parley ] || old="parley-5250 (<< $version), "
      printf 'Replaces: %sparley (<< %s)\nBreaks: %sparley (<< %s)\n' "$old" "$version" "$old" "$version" \
        >>"$tree/DEBIAN/control"
      dpkg-deb --root-owner-group -b "$tree" "$deb" >/dev/null
      rm -rf "$tree"
      echo "$deb"
    done
    ;;
  dmg)
    # The three apps and a link to Applications, as cargo-packager's own .dmg holds one app.
    # Not a `case` inside `$( )`: macOS's bash 3.2 cannot parse its `)`.
    arch=aarch64
    if [[ "${target:-$(uname -m)}" == x86_64* ]]; then arch=x64; fi
    ln -sfn /Applications "$staging/Applications"
    # target/packages holds the .msi and AppImage elsewhere, but nothing has made it on a Mac.
    mkdir -p "$out"
    dmg="$out/Parley_${version}_${arch}.dmg"
    rm -f "$dmg"
    hdiutil create -volname "Parley $version" -srcfolder "$staging" -fs HFS+ -format UDZO -ov "$dmg"
    echo "$dmg"
    ;;
esac
