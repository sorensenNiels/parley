#!/usr/bin/env python3
"""rsync for Parley SSH's Windows installer (R-SSH-66, D58): MSYS2's rsync and the DLLs it loads.

Windows has no rsync. MSYS2's packages are fetched at pinned versions and checked against their SHA-256; when they
were pinned, each package's signature was checked against MSYS2's keyring (key 5F944B02…A0AA7F57, certified by three
of its master keys). rsync is 3.4.1, as on macOS (scripts/build-rsync.sh). Only rsync.exe and the DLLs it needs are
taken (its imports and theirs, checked when pinned), with every licence and where each source is. Parley SSH finds
rsync.exe beside parley-ssh.exe; the installer puts these files there, in the Parley SSH feature.

Usage: python3 scripts/fetch-rsync-windows.py OUT_DIR    (Python 3.14 or later: it reads .tar.zst itself)
"""

import hashlib
import shutil
import sys
import tarfile
import urllib.request
from pathlib import Path

REPO = "https://repo.msys2.org/msys/x86_64/"
SOURCES = "https://repo.msys2.org/msys/sources/"
LICENCES = Path(__file__).resolve().parent.parent / "crates" / "parley-app" / "windows" / "rsync"

# (package file, SHA-256, its source package, licence, files taken: archive path -> name in OUT_DIR)
PACKAGES = [
    (
        "rsync-3.4.1-1-x86_64.pkg.tar.zst",
        "ced7a6d0cd397b13f1a24e73928674d6f973ee50a7467526047b1f4721b81683",
        "rsync-3.4.1-1",
        "GPL-3.0 (rsync-COPYING.txt)",
        {"usr/bin/rsync.exe": "rsync.exe", "usr/share/licenses/rsync/COPYING": "rsync-COPYING.txt"},
    ),
    (
        "msys2-runtime-3.6.12-1-x86_64.pkg.tar.zst",
        "c0dbaa624e29be6d08cf5c65609dca6c749431109e4572bf446a3ca9b4c4fd7c",
        "msys2-runtime-3.6.12-1",
        "GPL-3.0 or later (rsync-COPYING.txt)",
        {"usr/bin/msys-2.0.dll": "msys-2.0.dll"},
    ),
    (
        "libopenssl-3.6.5-1-x86_64.pkg.tar.zst",
        "05f6cc946f0dde3aecc3632007fa1d874e40720226117b750327bc2a01846325",
        "openssl-3.6.5-1",
        "Apache-2.0 (openssl-LICENSE.txt)",
        {
            "usr/bin/msys-crypto-3.dll": "msys-crypto-3.dll",
            "usr/share/licenses/libopenssl/LICENSE.txt": "openssl-LICENSE.txt",
        },
    ),
    (
        "libiconv-1.19.1-1-x86_64.pkg.tar.zst",
        "74efd2897aab2c68e465c6554e615d40c3c83d2e0a51a87309851030ca794cb9",
        "libiconv-1.19.1-1",
        "LGPL-2.1 (LGPL-2.1.txt)",
        {"usr/bin/msys-iconv-2.dll": "msys-iconv-2.dll"},
    ),
    (
        "liblz4-1.11.1-1-x86_64.pkg.tar.zst",
        "b35db1392281429746698daaee05934d3eaa437a585f723b1723bca409189742",
        "lz4-1.11.1-1",
        "GPL-2.0 or later, as MSYS2 labels the package (GPL-2.0.txt)",
        {"usr/bin/msys-lz4-1.dll": "msys-lz4-1.dll"},
    ),
    (
        "libxxhash-0.8.4-1-x86_64.pkg.tar.zst",
        "ba784ad117626401cd5d3ec0cf70399f9de72a63755f96c28fa2032303f9c330",
        "xxhash-0.8.4-1",
        "BSD-2-Clause (xxhash-LICENSE.txt)",
        {
            "usr/bin/msys-xxhash-0.dll": "msys-xxhash-0.dll",
            "usr/share/licenses/libxxhash/LICENSE": "xxhash-LICENSE.txt",
        },
    ),
    (
        "libzstd-1.5.7-1-x86_64.pkg.tar.zst",
        "5e370b2e725a0a7e5ff58d84653c9a0cd5a10b9cfe930f2d949a28ad2a11c1f2",
        "zstd-1.5.7-1",
        "BSD-3-Clause (zstd-LICENSE.txt)",
        {"usr/bin/msys-zstd-1.dll": "msys-zstd-1.dll"},
    ),
]

# The licence texts the packages do not carry, kept in the repository.
KEPT = ["LGPL-2.1.txt", "GPL-2.0.txt", "zstd-LICENSE.txt"]


def fetch(name: str, sha256: str, work: Path) -> Path:
    path = work / name
    with urllib.request.urlopen(REPO + name, timeout=120) as answer:
        path.write_bytes(answer.read())
    got = hashlib.sha256(path.read_bytes()).hexdigest()
    if got != sha256:
        sys.exit(f"fetch-rsync-windows: {name} has SHA-256 {got}, not the pinned {sha256}")
    return path


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__.strip().splitlines()[-1])
    out = Path(sys.argv[1])
    work = out / ".packages"
    work.mkdir(parents=True, exist_ok=True)
    notice = [
        "rsync 3.4.1 comes with Parley SSH as a program of its own, with the DLLs it needs, all from MSYS2",
        "(https://www.msys2.org). Parley SSH runs it for transfers; none of it is linked into Parley SSH.",
        "",
        "Each part, its licence, and where its source is:",
        "",
    ]
    for name, sha256, source, licence, files in PACKAGES:
        package = fetch(name, sha256, work)
        with tarfile.open(package, "r:zst") as archive:
            for inside, outside in files.items():
                member = archive.extractfile(inside)
                if member is None:
                    sys.exit(f"fetch-rsync-windows: {name} has no {inside}")
                (out / outside).write_bytes(member.read())
        dlls = ", ".join(o for o in files.values() if o.endswith((".dll", ".exe")))
        notice += [f"{dlls}: {licence}", f"  {SOURCES}{source}.src.tar.zst", ""]
    for text in KEPT:
        shutil.copyfile(LICENCES / text, out / text)
    (out / "rsync-SOURCE.txt").write_text("\n".join(notice), encoding="utf-8")
    shutil.rmtree(work)
    print(out / "rsync.exe")


if __name__ == "__main__":
    main()
