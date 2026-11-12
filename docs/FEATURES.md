# Parley features

> Features that are implemented **and verified**, by requirement ID. The requirements they were built to are in
> [REQUIREMENTS.md](REQUIREMENTS.md); open work and new requirements are in [ROADMAP.md](ROADMAP.md).
> Look an ID up with `grep -n` rather than reading the whole file.
>
> Add an entry only when the requirement is fully met, has automated tests where practical, and has
> been checked in the running app on at least one OS. Note which OSes were checked. Then move the
> requirement's row from the roadmap to [REQUIREMENTS.md](REQUIREMENTS.md).

**Verified on:** Linux = Ubuntu under WSL2/WSLg (desktop build), against PUB400.COM (IBM i 7.5). Windows and macOS have
not been run yet.

## In progress

### Phase 1: connect and show a screen (2026-11-12)

Built: R-NET-1..3, R-DS-1..4, R-EBC-1, R-KEY-1, R-UI-1..4, R-NF-1. The requirements stay in the roadmap until signing on
with a real account has been tried in the running app.

Checked so far:
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

Not checked yet: signing on and using menus, prompts (F4), Save/Restore Screen from a real host, 27×132 screens,
the device name of R-NET-3 (a taken name), typing in the running app, a CA certificate file, Windows and macOS.

### Phase 2: saved sessions and auto sign-on (2026-11-12)

Built: R-UI-5, R-NET-4, R-NET-5, R-UI-6, R-UI-7, R-UI-8, R-UI-9, R-UI-10.

Checked so far:
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
  edge, in kittest and a screenshot.
- **Running app** (Linux/WSLg screenshots): the start screen lists the session migrated from the 0.1 settings; the
  session dialog with every option. WSL has no Secret Service, so the dialog says the keychain is unavailable.
  R-UI-6: the start screen, session dialog (two columns, fits a 1000×640 window), settings dialog with the font
  preview, and PUB400's sign-on screen in IBM Plex Mono with the roomier bars; `make package` puts the OFL files in
  the .deb (66 ms to list this machine's installed monospace fonts).

Not checked yet: auto sign-on (typed or bypassed) against a real IBM i with a real account, the keychain on Windows,
macOS and a Linux desktop, installed fonts on Windows and macOS, the light theme, keep-alive over a long idle period, exit when the host ends the session, confirm exit.

## Implemented

(none yet)
