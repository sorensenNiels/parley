# Parley roadmap

> The plan, and the spec for everything that is not done yet. Completed requirements move to
> [REQUIREMENTS.md](REQUIREMENTS.md), and how each was verified is in [FEATURES.md](FEATURES.md).
> Look things up with `grep -n` (an ID such as `"R-NET-1 "`, `"| D3 "` or a heading) rather than reading the whole file.

**Status of this document:** a first draft written while setting up the development environment (2026-11-12),
updated when Phase 1 was built the same day. The vision, principles and requirements are proposals to review.

## 1. Vision

Parley is a fast, modern TN5250 terminal emulator for IBM i, written in Rust. It is a desktop app for Windows, Linux and
macOS from one code base, starts instantly, and feels at home for people who use green screens all day, with a
modern take on what surrounds the screen: sessions, keyboard mapping, copy and paste, and scripting.

## 2. Product principles

| ID | Principle |
|---|---|
| R-P-1 | **Keyboard first.** Every 5250 function key and action is reachable from the keyboard, with mappings people know from existing emulators. |
| R-P-2 | **Correct before clever.** The 5250 data stream is implemented to IBM's specification and tested against recorded host traffic. |
| R-P-3 | **Secure by default.** TLS (port 992) is the default for new sessions; passwords are never stored in plain text. |
| R-P-4 | **One engine.** All protocol logic lives in `parley-core`, with no UI or platform code, so it can be tested on its own. |

## 3. Status

| Phase | State |
|---|---|
| Phase 0: development environment | Done (2026-11-12): workspace, Makefile, setup scripts, CI, lints, Claude Code setup. |
| Phase 1: connect and show a screen | Built (2026-11-12); in progress until signing on with a real account is checked (FEATURES.md "In progress"). |
| Phase 2: saved sessions and auto sign-on | Built (2026-11-12); in progress until auto sign-on is checked against a real IBM i. |

## 4. Phases

### Phase 0 — Development environment (done)

Workspace with `parley-core`, `parley-ui-egui`, `parley-app`; `make verify` as the local quality gate (= CI);
setup scripts for Linux, macOS and Windows; the app opens a window with a blank 24×80 screen.

### Phase 1 — Connect and show a screen (built, being verified)

Open a session to an IBM i, sign on and use simple screens: R-NET-1..3, R-DS-1..4, R-EBC-1, R-KEY-1, R-UI-1..4.
Known gaps are IDEA-1..10.

### Phase 2 — Saved sessions and auto sign-on (built 2026-11-12, being verified)

Several named sessions with their own settings, a session dialog, passwords in the keychain, auto sign-on (typed or
bypassed per RFC 4777) and keep-alive: R-UI-5, R-NET-4, R-NET-5. A roomier look and a settings dialog with the terminal
font: R-UI-6. Close buttons on popups: R-UI-7. Settings in pages: R-UI-8. Tortuga's look: R-UI-9. App name in the menu bar, Help on the right: R-UI-10.

### Later (unordered, to be planned)

Printer sessions, macros/scripting (IDEA-11), a keyboard map per session.

## 5. Requirements

Open requirements, in the order they are planned. Areas: `NET` connection and telnet, `DS` the 5250 data stream,
`EBC` character sets, `KEY` keyboard, `UI` user interface, `NF` non-functional.

| ID | Requirement | Phase |
|---|---|---|
| R-NET-1 | Open a TCP connection to a host and port (default 23) and negotiate TN5250 per RFC 1205 / RFC 2877: terminal type (`IBM-3179-2`), binary, end-of-record. | 1 |
| R-NET-2 | TLS connections (default port 992), checking the server certificate; a session can trust a given CA certificate. | 1 |
| R-NET-3 | Request a device name through the NEW-ENVIRON option (RFC 2877), and show the host's error when it is refused. | 1 |
| R-DS-1 | Parse the 5250 data stream commands needed for sign-on and menus: Clear Unit, Write To Display (with its orders: Set Buffer Address, Start of Header, Start of Field, Insert Cursor, Repeat to Address), Read MDT Fields, Read Input Fields. | 1 |
| R-DS-2 | A screen model: character buffer, field table (attributes, input/output, mandatory, bypass), cursor and the operator-information line. | 1 |
| R-DS-3 | Send AID keys (Enter, F1–F24, Page Up/Down, Clear, Help, Sys Req, Attention) with the modified fields. | 1 |
| R-DS-4 | The other commands an IBM i sends in normal use: Write Error Code (message on the error line, keyboard inhibited until Reset), Save Screen / Restore Screen, Roll, Clear Unit Alternate (27×132), Clear Format Table, Read Immediate, Read Screen Immediate, and the reply to the 5250 Query structured field. Erase to Address, Transparent Data and Move Cursor orders. | 1 |
| R-EBC-1 | Convert EBCDIC to and from Unicode for CCSID 37 (US) and 277 (Danish/Norwegian), and their euro variants 1140 and 1142; the session chooses the CCSID and announces it to the host (`KBDTYPE`, `CODEPAGE`, `CHARSET`, RFC 4777). | 1 |
| R-KEY-1 | A default keyboard mapping: Enter; Shift+Enter or Ctrl+Enter for Field Exit; Ctrl++ / Ctrl+- (and + / - in signed numeric fields) for Field+ / Field-; a Ctrl key pressed alone or Ctrl+R for Reset; Tab / Shift+Tab between fields; F1–F24 (Shift+F1–F12 for F13–F24); Page Up / Down to roll; Esc / Shift+Esc for Attention / System Request; Insert, Delete, Backspace, Home, End, Ctrl+End (Erase EOF), arrows. The field rules of the field format word: bypass, alpha/numeric/digits/signed numeric only, monocase, auto enter, field exit required, right adjust. (Right Ctrl for Enter, as in IBM's emulators, is not possible: egui does not tell left and right Ctrl apart.) | 1 |
| R-UI-1 | The screen in a monospace grid that scales with the window, with the 5250 colours, reverse image, underline, column separators and non-display; a block cursor (a bar in insert mode); a click moves the cursor; Ctrl+C copies the screen as text; Ctrl+V pastes into fields (a line break moves to the next field). | 1 |
| R-UI-2 | The operator information area under the screen: Ready / X SYSTEM / the operator error and "press Reset", insert mode, message waiting, cursor row/column, and the last problem in the host's data. A list of the keyboard shortcuts. | 1 |
| R-UI-4 | A menu bar (File, Edit, View, Tools, Help) and a toolbar (Disconnect, Copy, Paste, Clear, Attn, Sysreq, Reset, Help, Enter) above the screen, and the status bar of R-UI-2 as boxes along the bottom (message, Online/Offline, TLS, Insert, MW, row,column), as in IBM's emulators; all three also show without a connection. The menus hold today's actions; their full contents come later. | 1 |
| R-NF-1 | Installers for every platform, built by the Release workflow from a `vX.Y.Z` tag and attached to a GitHub release: `.msi` (Windows), `.dmg` (macOS, Apple silicon and Intel), `.deb` and `.AppImage` (Linux, glibc 2.35+). `make release-patch/minor/major` bumps the version, verifies, tags and pushes (docs/RELEASING.md). | 1 |
| R-UI-3 | A connection form (host, port, TLS, CA certificate file, code page, screen size, device name) that remembers the last settings; `parley HOST[:PORT] [--tls]` connects straight away. *Since 2026-11-12 the form is the session dialog of R-UI-5; `HOST` uses the saved session for that host if there is one.* | 1 |
| R-UI-5 | Saved sessions (2026-11-12): the start screen lists them (Connect, Edit…, New session…) and File has New session…, Edit sessions… and Connect to ▸. A session dialog, as in IBM-style emulators: name (and a menu to switch session), host, port and TLS, CA certificate, code page, terminal size (27×132 / 24×80), device name; auto sign-on (user, password, enable, bypass the sign-on screen, password encryption); run macro (shown disabled until IDEA-11); exit when the host ends the session; confirm exit while connected; send keep-alive; connect on program start (one session at most); Help; Cancel, Delete, Apply, Connect (disabled while connected: one session per window). New sessions default to TLS on 992 (R-P-3). Passwords go to the operating system's keychain (D11), never to the settings file; renaming or deleting a session moves or removes its password. The single connection Parley 0.1 remembered becomes the first saved session. | 2 |
| R-UI-6 | A roomier look and settings (2026-11-12): Inter as the UI font, larger text and more padding and spacing (sizes since R-UI-9: Tortuga's) in menus, toolbar, status bar, dialogs and the start screen (sessions as cards). Tools ▸ Settings…: theme (System / Light / Dark), size (90–175 %), and the terminal font: one of the bundled JetBrains Mono (default since R-UI-9), IBM Plex Mono and Hack, or any installed monospace font (found on a background thread; a font counts as monospace when i, M, W and . are equally wide), with a preview. Changes show at once; Cancel restores them; the settings are remembered. A saved font that is no longer installed falls back to the default with a note. Installers carry the fonts' OFL licence files. | 2 |
| R-UI-7 | Every popup and side panel has a close button (×) at the top right of its title row (2026-11-12): the session dialog (closes without saving, as Cancel), Settings (as Cancel: the old settings come back), the exit question (as Cancel) and the keyboard shortcuts panel (hides it). Screen readers name it "Close". | 2 |
| R-UI-8 | The settings dialog in pages (2026-11-12): a list of groups on the left (Appearance, Terminal; new settings add a group or go into one), the chosen page on the right with its title and a one-line description. The dialog keeps one size whichever page is shown. "Restore defaults" resets only the page shown. | 2 |
| R-UI-9 | Tortuga's look (2026-11-12, D12): `theme.rs` ported from Tortuga: Inter (Regular, SemiBold for headings) and JetBrains Mono, the same light and dark palette (teal accent), text sizes, spacing, corner radii and shadows, thin floating scroll bars; the menu bar, toolbar and status bar in the chrome colour; filled primary (Connect, Save, Enter, Exit) and danger (Delete) buttons; switches for on/off options and segmented choices; dialogs with Tortuga's frame and Cancel/confirm row; the settings dialog laid out as Tortuga's (pages under section headings, groups of rows on cards); a Density setting (Comfortable / Compact). The 5250 screen keeps its own colours. | 2 |
| R-UI-10 | The menu bar starts with the app icon (the launcher's PNG, 18 px) and "Parley" in SemiBold, then a separator and File, Edit, View, Tools; Help sits at the right edge (2026-11-12). As Tortuga shows its own name at the start of its top bar. | 2 |
| R-NET-4 | Auto sign-on (2026-11-12). *Screen*: on the first screen whose first two input fields are a user field and a non-display field, type the user and password and press Enter, once per connection. *Bypass*: answer the host's NEW-ENVIRON SEND with `VAR USER`, `USERVAR IBMRSEED` (a random client seed) and `USERVAR IBMSUBSPW` (the password substitute) per RFC 4777 section 5, DES (5.1) or SHA-1 (5.2) as the session's password-encryption setting says (QPWDLVL 0–1 or 2–3); the host needs QRMTSIGN `*VERIFY` or `*SAMEPRF`. Only when the host sends its seed: the password never goes in clear text. Checked against the RFC's test vectors and its example negotiation, byte for byte. | 2 |
| R-NET-5 | Keep-alive (2026-11-12): a session with it on sends telnet NOP (IAC X'F1') after 60 s without sending anything, so firewalls keep an idle connection. | 2 |

## 6. Ideas

| ID | Idea |
|---|---|
| IDEA-1 | Check against IBM's 5250 Functions Reference the points built from memory of the spec, not yet seen on a real host: Start of Header clears the format table; Read MDT Fields sends nulls as blanks; a signed numeric field is sent without its sign position, a minus making the last digit's zone X'D'; the query reply's capability bytes. |
| IDEA-2 | Operator error codes (0005 and so on) next to the messages, as IBM's emulators show them, once the codes are checked against the manual. |
| IDEA-3 | Mandatory enter, mandatory fill and self-check (modulo 10/11) fields: not enforced yet; the host checks them. |
| IDEA-4 | The Start of Header command-key bits that say whether F1–F24 return field data: all keys return data now. |
| IDEA-5 | Extended attributes (Write Extended Attribute) and the GUI structured fields (windows, menus, scroll bars, WDSF): ignored now; Parley's query reply does not ask for them. |
| IDEA-6 | Sound the alarm (CC2 bit) and blink (blink attributes): both are ignored now. |
| IDEA-7 | Save Partial Screen / Restore Partial Screen and Read Immediate Alternate: not supported yet (the host gets a notice-level error). |
| IDEA-8 | Insert mode accepts trailing blanks as room, not only nulls as on a real 5250: kinder, but check it does not surprise people. |
| IDEA-9 | Session profiles: several saved connections, each with its own settings and keyboard map. *Saved sessions are R-UI-5 (2026-11-12); the keyboard map per session is still open.* |
| IDEA-11 | Macros: record and play keystrokes, and run one after sign-on ("Run macro" in the session dialog, disabled until then). |
| IDEA-12 | Startup Response Record (RFC 4777 section 10, `IBMSENDCONFREC`): show why a bypass sign-on failed (user unknown, password expired, and so on) instead of only the sign-on screen. |
| IDEA-10 | A trace window (the host's records, decoded) for support, built on `PARLEY_TRACE`. |

## 7. Decisions

| ID | Question | Decision | Alternatives considered |
|---|---|---|---|
| D1 | Language and UI toolkit? | **Rust with egui/eframe** (OpenGL through glow), the same stack and conventions as the Tortuga project. | A web/Electron front end; Tauri. |
| D2 | How is the code split? | **`parley-core`** (engine, no UI deps), **`parley-ui-egui`** (UI state and components, no platform code), **`parley-app`** (thin launcher). No CLI (2026-11-12): Parley is a desktop app only. | One crate; a CLI crate as in Tortuga. |
| D3 | Must the engine run in a browser? | **No** (follows from D5): `parley-core` targets the desktop only and may use threads, `std::fs` and a runtime where needed. *Changed 2026-11-12; first drafted as "compiles for wasm32".* | Keep the engine wasm32-portable. |
| D4 | Where does networking live? | **The protocol is sans-I/O** (`parley_core::Session`: bytes in, bytes out, keys in), so it is tested byte for byte. **`parley_core::net`** runs it over a socket on a plain thread that polls every 15 ms; the UI locks the session to draw and to pass keys. No async runtime (2026-11-12). | The launcher owns the socket; tokio. |
| D5 | Which platforms? | **Desktop apps for Windows, macOS and Linux only; no web build** (2026-11-12). | A web build through a WebSocket-to-TCP relay. |
| D6 | Which TLS stack and trust store? | **rustls** (ring backend, TLS 1.2 and 1.3) with **the operating system's trust store** (`rustls-platform-verifier`), plus an optional CA certificate file per connection for company CAs. No "ignore certificate errors" switch (R-P-3). | OpenSSL / native-tls; Mozilla's roots only. |
| D7 | What does Parley send as Save Screen restore data? | **An id** (ESC X'12', "PY", 2-byte id) **for a copy of the screen kept in the session** (the last 16), not a rebuilt data stream as tn5250 sends. Restore data from elsewhere is reported and ignored. | Re-encode the screen as Write To Display orders. |
| D8 | Destructive Backspace? | **Yes**: Backspace deletes the character left of the cursor inside a field, as PC users expect; a real 5250 only moves the cursor. | The 5250 behaviour. |
| D9 | How are installers built? | **cargo-packager** from `[package.metadata.packager]`, on each OS in the Release workflow, as in Tortuga (2026-11-12). | cargo-dist; per-OS tools (WiX, create-dmg) by hand. |
| D11 | Where are passwords kept? | **The operating system's keychain** through the `keyring` crate (Keychain Services on macOS, Credential Manager on Windows, Secret Service on Linux), filed under service "Parley" and the session's name; the UI reaches it through a `Keychain` trait so tests use memory (2026-11-12). Without a keychain (e.g. WSL with no Secret Service) the password is not saved, and the dialog says why. | Encrypt the settings file with a master password; never save passwords. |
| D12 | How does Parley look? | **Like Tortuga** (2026-11-12): the same design system, ported as `parley-ui-egui/src/theme.rs` (fonts, palette, sizes, visuals, building blocks) and `dialog.rs` (dialog frame, close button, actions, `fitting_scroll_area`), the same fonts (`assets/fonts`), and the same UI rules (CLAUDE.md "UI look"). A change to the shared look is made in both repositories. Parley's own additions: the terminal font family and the 5250 screen colours. | A look of Parley's own. |
| D10 | Are installers signed? | **Not yet**: macOS and Windows warn on first start (docs/RELEASING.md says how to go on). Signing needs an Apple Developer ID and a Windows certificate. | Sign from the first release. |

## 8. How to change this roadmap

- New work gets a requirement in §5 with the next free ID of its area; IDs are never reused or renumbered.
- A requirement that is implemented, tested and checked in the running app moves to [REQUIREMENTS.md](REQUIREMENTS.md),
  with how it was verified in [FEATURES.md](FEATURES.md).
- A choice that future work must respect becomes a decision in §7.

Next free IDs (removed rows keep their numbers): `R-P-5` · `R-NET-6` · `R-DS-5` · `R-EBC-2` · `R-KEY-2` · `R-UI-11` · `R-NF-2` · `IDEA-13` · `D13`.
