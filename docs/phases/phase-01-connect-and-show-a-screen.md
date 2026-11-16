# Phase 1 — Connect and show a screen

> Finished phase of Parley 5250. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

Open a session to an IBM i, sign on and use simple screens: R-NET-1..3, R-DS-1..4, R-EBC-1, R-KEY-1, R-UI-1..4.
Known gaps are IDEA-1..10 ([IDEAS.md](../IDEAS.md); IDEA-1 became R-DS-5).

**Status:** Done (2026-11-12): verified on Windows, macOS, Linux and WSL2 (FEATURES.md).

## Requirements

| ID | Requirement |
|---|---|
| R-NET-1 | Open a TCP connection to a host and port (default 23) and negotiate TN5250 per RFC 1205 / RFC 2877: terminal type (`IBM-3179-2`), binary, end-of-record. |
| R-NET-2 | TLS connections (default port 992), checking the server certificate; a session can trust a given CA certificate. |
| R-NET-3 | Request a device name through the NEW-ENVIRON option (RFC 2877), and show the host's error when it is refused. |
| R-DS-1 | Parse the 5250 data stream commands needed for sign-on and menus: Clear Unit, Write To Display (with its orders: Set Buffer Address, Start of Header, Start of Field, Insert Cursor, Repeat to Address), Read MDT Fields, Read Input Fields. |
| R-DS-2 | A screen model: character buffer, field table (attributes, input/output, mandatory, bypass), cursor and the operator-information line. |
| R-DS-3 | Send AID keys (Enter, F1–F24, Page Up/Down, Clear, Help, Sys Req, Attention) with the modified fields. |
| R-DS-4 | The other commands an IBM i sends in normal use: Write Error Code (message on the error line, keyboard inhibited until Reset), Save Screen / Restore Screen, Roll, Clear Unit Alternate (27×132), Clear Format Table, Read Immediate, Read Screen Immediate, and the reply to the 5250 Query structured field. Erase to Address, Transparent Data and Move Cursor orders. |
| R-EBC-1 | Convert EBCDIC to and from Unicode for CCSID 37 (US) and 277 (Danish/Norwegian), and their euro variants 1140 and 1142; the session chooses the CCSID and announces it to the host (`KBDTYPE`, `CODEPAGE`, `CHARSET`, RFC 4777). |
| R-KEY-1 | A default keyboard mapping: Enter; Shift+Enter or Ctrl+Enter for Field Exit; Ctrl++ / Ctrl+- (and + / - in signed numeric fields) for Field+ / Field-; a Ctrl key pressed alone or Ctrl+R for Reset; Tab / Shift+Tab between fields; F1–F24 (Shift+F1–F12 for F13–F24); Page Up / Down to roll; Esc / Shift+Esc for Attention / System Request; Insert, Delete, Backspace, Home, End, Ctrl+End (Erase EOF), arrows. The field rules of the field format word: bypass, alpha/numeric/digits/signed numeric only, monocase, auto enter, field exit required, right adjust. (Right Ctrl for Enter, as in IBM's emulators, is not possible: egui does not tell left and right Ctrl apart.) |
| R-UI-1 | The screen in a monospace grid that scales with the window, with the 5250 colours, reverse image, underline, column separators and non-display; a block cursor (a bar in insert mode); a click moves the cursor; Ctrl+C copies the screen as text; Ctrl+V pastes into fields (a line break moves to the next field). |
| R-UI-2 | The operator information area under the screen: Ready / X SYSTEM / the operator error and "press Reset", insert mode, message waiting, cursor row/column, and the last problem in the host's data. A list of the keyboard shortcuts. |
| R-UI-3 | A connection form (host, port, TLS, CA certificate file, code page, screen size, device name) that remembers the last settings; `parley HOST[:PORT] [--tls]` connects straight away. *Since 2026-11-12 the form is the session dialog of R-UI-5; `HOST` uses the saved session for that host if there is one.* |
| R-UI-4 | A menu bar (File, Edit, View, Tools, Help) and a toolbar (Disconnect, Copy, Paste, Clear, Attn, Sysreq, Reset, Help, Enter) above the screen, and the status bar of R-UI-2 as boxes along the bottom (message, Online/Offline, TLS, Insert, MW, row,column), as in IBM's emulators; all three also show without a connection. The menus hold today's actions; their full contents come later. |
| R-NF-1 | Installers for every platform, built by the Release workflow from a `vX.Y.Z` tag and attached to a GitHub release: `.msi` (Windows), `.dmg` (macOS, Apple silicon and Intel), `.deb` and `.AppImage` (Linux, glibc 2.35+). `make release-patch/minor/major` bumps the version, verifies, tags and pushes (docs/RELEASING.md). |

## Verification

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
