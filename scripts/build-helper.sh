#!/usr/bin/env bash
# Builds and checks the SQL helper (R-SQL-11, D20): fetches JTOpen (pinned), compiles the helper
# with warnings as errors for Java 11, packs parley-sql.jar and runs its self-test.
# Usage: scripts/build-helper.sh OUT_DIR    (needs a JDK 11+; Linux, macOS, Git Bash on Windows)
set -euo pipefail

out="${1:?usage: build-helper.sh OUT_DIR}"
here="$(cd "$(dirname "$0")/.." && pwd)"

if ! command -v javac >/dev/null; then
    echo "parley: the SQL helper needs a JDK 11 or later (javac); run the setup script for your OS" >&2
    exit 1
fi
"$here/scripts/fetch-jt400.sh" "$out"

case "$(uname -s)" in
    MINGW* | MSYS* | CYGWIN*) sep=';' ;;
    *) sep=':' ;;
esac

rm -rf "$out/classes"
mkdir -p "$out/classes"
javac --release 11 -Xlint:all -Werror -encoding UTF-8 -cp "$out/jt400.jar" \
    -d "$out/classes" "$here"/helpers/sql/src/parley/sql/*.java
jar --create --file "$out/parley-sql.jar" -C "$out/classes" .
java -cp "$out/parley-sql.jar${sep}$out/jt400.jar" parley.sql.Helper --self-test
