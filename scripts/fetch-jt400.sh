#!/usr/bin/env bash
# Downloads JTOpen's jt400.jar, pinned and checked, for the SQL helper (R-SQL-13, D20).
# Usage: scripts/fetch-jt400.sh OUT_DIR    (Linux, macOS, and Git Bash on Windows)
set -euo pipefail

VERSION=21.0.7
SHA256=d2c06bba78d20b4733a3fabc9fe6ccabac337f7298bea95c6766108fa0b420f8
URL="https://repo1.maven.org/maven2/net/sf/jt400/jt400/${VERSION}/jt400-${VERSION}.jar"

out="${1:?usage: fetch-jt400.sh OUT_DIR}"
jar="${out}/jt400.jar"

sha256() {
    if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d' ' -f1
    else shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

if [[ -f "$jar" && "$(sha256 "$jar")" == "$SHA256" ]]; then
    exit 0
fi
mkdir -p "$out"
curl -fsSL --retry 3 -o "${jar}.part" "$URL"
if [[ "$(sha256 "${jar}.part")" != "$SHA256" ]]; then
    rm -f "${jar}.part"
    echo "parley: jt400.jar ${VERSION} does not have the expected SHA-256; not used" >&2
    exit 1
fi
mv "${jar}.part" "$jar"
echo "parley: fetched JTOpen ${VERSION} into ${jar}"
