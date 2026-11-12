# Parley features

> Features that are implemented **and verified**, by requirement ID. The requirements they were built to are in
> [REQUIREMENTS.md](REQUIREMENTS.md); open work and new requirements are in [ROADMAP.md](ROADMAP.md).
> Look an ID up with `grep -n` rather than reading the whole file.
>
> Add an entry only when the requirement is fully met, has automated tests where practical, and has
> been checked in the running app on at least one OS. Note which OSes were checked. Then move the
> requirement's row from the roadmap to [REQUIREMENTS.md](REQUIREMENTS.md).

**Verified on:** Windows, macOS, Linux (desktop) and Linux under WSL2/WSLg, by the owner in the running app (2026-11-12);
Claude's checks ran on Linux/WSL2 against PUB400.COM (IBM i 7.5) and the owner's IBM i (sign-on screen only).

## In progress

- **R-NET-4 on Linux** (2026-11-12): auto sign-on is verified on Windows and macOS, but not yet on a Linux desktop.
  Under WSL2 it stops with "no keychain is available": WSL runs no Secret Service, so there is no saved password (D11).
  **Left:** the owner checks auto sign-on, by screen and bypassed, on a Linux desktop with GNOME Keyring or KDE Wallet.
  Until then the Phase 2 "verified on" list does not cover R-NET-4 on Linux.

## Implemented

### Phase 1: connect and show a screen (2026-11-12) — R-NET-1..3, R-DS-1..4, R-EBC-1, R-KEY-1, R-UI-1..4, R-NF-1

Verified by the owner in the running app on Windows, macOS, Linux and WSL2 (2026-11-12).

How it was checked:
- **Automated** (`make test`): telnet negotiation and framing; record header; every command and order in R-DS-1 and
  R-DS-4 with hand-built records; field rules and keys; the exact bytes of Enter, Read Immediate, the query reply and
  Save Screen; CCSID 37/277/1140/1142 tables (generated from iconv by `scripts/gen-ebcdic.py`); 6000 random records
  without a panic; a whole session over TCP against a fake host (`tests/connection.rs`); the terminal widget and form
  in egui_kittest (typing, Enter, a click moving the cursor, the screen exposed to assistive technology).
- **Against PUB400.COM** (`cargo run -p parley-core --example probe`): TN5250 negotiation on port 23 and TLS on 992
  (certificate checked against the OS store); the sign-on screen shown with both input fields and the cursor where the
  host put it; CCSID 277 (`KBDTYPE` DMB) and the 3477-FC terminal type accepted; Help answered by a Write Error Code
  ("Function key not allowed."), Reset putting the line back; Enter with empty fields answered by
  "CPF1296 - Sign-on information required." (the host accepted Parley's input record).
- **Running app** (`parley pub400.com --screenshot FILE`): the sign-on screen drawn with the host's colours, reverse
  image and underlines, the block cursor in the user field, "Ready" in the status line.


### Phase 2: saved sessions, auto sign-on and the look (2026-11-12) — R-NET-4, R-NET-5, R-UI-5..12

Verified by the owner in the running app on Windows, macOS, Linux and WSL2 (2026-11-12). Auto sign-on (R-NET-4) on Linux is still
to be checked (see "In progress").

How it was checked:
- **Automated**: the DES and SHA-1 password substitutes against RFC 4777's test vectors (sections 5.1 and 5.2) and
  the RFC's example negotiation byte for byte; no credentials without a host seed; a truncated or escaped seed;
  typed sign-on sends the same record as an operator typing, only once; keep-alive bytes; in egui_kittest, the start
  screen, saving a session with its password in a (memory) keychain only, required name and host, delete and rename
  moving the password, one session marked for program start; the bundled terminal fonts measured as monospace (and
  Inter not), a missing font falling back, choosing a font in the settings, OK keeping it and Cancel restoring; the
  close button (×) of the settings, the session dialog and the shortcuts panel (R-UI-7); settings pages, Restore
  defaults per page, and the dialog keeping its place from frame to frame (R-UI-8); R-UI-9: Inter / Inter SemiBold /
  JetBrains Mono families, thin floating scroll bars, every symbol the UI draws present in the bundled fonts, the
  session dialog keeping its place. Screenshots (Linux/WSLg): start screen, PUB400 connected, session dialog and
  settings dialog in dark, settings dialog in light. R-UI-10: the name (and icon) in the menu bar and Help at its right
  edge, in kittest and a screenshot. R-UI-11/12: version comparison, GitHub's answer read from a fixture (newer, same,
  404, error, garbage), unsafe download URLs left out, the `curl` command, the request naming only the app (against a
  local fake server); in kittest with a fake checker: the menu bar link, no check when turned off, Check for updates
  saying what it found, a failure silent at start-up but shown on Check now, the download command menu on Linux, and
  no checker meaning no check. Against GitHub (once, by hand): the About page said "up to date" for 0.2.1, and a check
  as 0.1.0 found 0.2.0 with the `.deb` and AppImage `curl` commands.
- **Running app** (Linux/WSLg screenshots): the start screen lists the session migrated from the 0.1 settings; the
  session dialog with every option. WSL has no Secret Service, so the dialog says the keychain is unavailable.
  R-UI-6: the start screen, session dialog (two columns, fits a 1000×640 window), settings dialog with the font
  preview, and PUB400's sign-on screen in IBM Plex Mono with the roomier bars; `make package` puts the OFL files in
  the .deb (66 ms to list this machine's installed monospace fonts).

### Phase 3: keyboard maps and macros (in progress) — R-DS-5

**R-DS-5** (2026-11-12): verified by the owner on their IBM i in the running app on Windows, macOS, Linux and WSL2
(signed on; a spooled file shown at 27×132 in a wide session).

How it was checked:
- **Against the manual**: IBM's 5494 Remote Control Unit Functions Reference (SC30-3533-02, chapter 12), cited next to
  the code. Start of Header matched (clears the format table; error row in byte 4) except that length 0 is now an
  error. Nulls as blanks and the negative zone matched; Read MDT Alternate now strips trailing nulls and sends only the
  address for an all-null field. The query reply sent a 2-byte model where the manual has 3, so every later byte was
  one early and the screen sizes landed in capability byte 0; fixed, with 256 input fields and the colour bit.
- **Automated** (`make test`): a test per fix in `tests/session.rs` (`*_r_ds_5`), each failing without its fix.
- **Against PUB400.COM**: the same sign-on screen at 24×80 and 27×132 before and after (it sends no Query before
  sign-on).
