# Parley roadmap

> The plan, and the spec for everything that is not done yet. Completed requirements move to
> [REQUIREMENTS.md](REQUIREMENTS.md), and how each was verified is in [FEATURES.md](FEATURES.md).
> Look things up with `grep -n` (an ID such as `"R-NET-1 "`, `"| D3 "` or a heading) rather than reading the whole file.

**Status of this document:** first written while setting up the development environment (2026-11-12); Phases 1 and 2
were built and verified the same day. The vision, principles and requirements are proposals to review.

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
| Phase 1: connect and show a screen | Done (2026-11-12): verified on Windows, macOS, Linux and WSL2 (FEATURES.md). |
| Phase 2: saved sessions and auto sign-on | Done (2026-11-12): verified on Windows, macOS, Linux and WSL2 (FEATURES.md), except auto sign-on on Linux, which waits for a check on a Linux desktop. |
| Phase 3: keyboard maps and macros | In progress: R-DS-5 done (2026-11-12); next R-NET-6, then R-KEY-2..4. |

## 4. Phases

### Phase 0 — Development environment (done)

Workspace with `parley-core`, `parley-ui-egui`, `parley-app`; `make verify` as the local quality gate (= CI);
setup scripts for Linux, macOS and Windows; the app opens a window with a blank 24×80 screen.

### Phase 1 — Connect and show a screen (done)

Open a session to an IBM i, sign on and use simple screens: R-NET-1..3, R-DS-1..4, R-EBC-1, R-KEY-1, R-UI-1..4.
Known gaps are IDEA-1..10 ([IDEAS.md](IDEAS.md); IDEA-1 became R-DS-5).

### Phase 2 — Saved sessions and auto sign-on (done)

Several named sessions with their own settings, a session dialog, passwords in the keychain, auto sign-on (typed or
bypassed per RFC 4777) and keep-alive: R-UI-5, R-NET-4, R-NET-5. A roomier look and a settings dialog with the terminal
font: R-UI-6. Close buttons on popups: R-UI-7. Settings in pages: R-UI-8. Tortuga's look: R-UI-9. App name in the menu bar, Help on the right: R-UI-10. New version available, with the download command on Linux:
R-UI-11, R-UI-12.

### Phase 3 — Keyboard maps and macros (planned)

First two small correctness steps (R-P-2): the points of the data stream built from memory checked against IBM's
manual (R-DS-5, from IDEA-1) and why a bypass sign-on failed (R-NET-6, from IDEA-12). Then keyboard maps chosen per
session (R-KEY-2, from IDEA-9) and macros, recorded (R-KEY-3) and run (R-KEY-4), from IDEA-11.

### Later (unordered, to be planned)

Printer sessions (IDEA-18), SQL against the session's host (IDEA-13); see [IDEAS.md](IDEAS.md).

## 5. Requirements

Open requirements, in the order they are planned. The done ones are in [REQUIREMENTS.md](REQUIREMENTS.md); new
ones come from [IDEAS.md](IDEAS.md). Areas: `NET` connection and telnet, `DS` the 5250 data stream,
`EBC` character sets, `KEY` keyboard, `UI` user interface, `NF` non-functional.

| ID | Requirement | Phase |
|---|---|---|
| R-NET-6 | **Why a bypass sign-on failed** (from IDEA-12). With bypass on (R-NET-4) Parley asks for the Startup Response Record (`USERVAR IBMSENDCONFREC` = `YES`, RFC 4777 section 10) and reads it before the first screen. On success nothing changes. On failure the status bar and a line on the start of the session say why, in words, with the host's response code (e.g. "Password expired", "User profile not found"; the codes and their meanings taken from RFC 4777 section 10, checked when this is built), and the sign-on screen the host sends next is shown as now. A truncated or malformed record is a reported error, not a panic. Tested with hand-built records. | 3 |
| R-KEY-2 | **Keyboard maps** (from IDEA-9). Settings ▸ Keyboard lists every 5250 key and Parley action (Enter, Field Exit, Field+/-, Reset, F1–F24, Attention, System Request, Roll, Erase EOF, Insert, Copy, Paste, …) with the PC keys bound to it; select one and press a key to add a binding, remove a binding, "Restore defaults". The map R-KEY-1 defines is the built-in **Default** map, which cannot be changed; the user can copy it under a new name, edit, rename and delete their own maps. A key bound twice in one map is flagged and the new binding wins only after the user confirms. A key egui cannot tell apart (e.g. left and right Ctrl, R-KEY-1) cannot be bound. The session dialog has "Keyboard map" (Default unless chosen); a session whose map was deleted falls back to Default. Maps are saved with the settings. | 3 |
| R-KEY-3 | **Record a macro** (from IDEA-11). Tools ▸ Record macro (and a key in the keyboard map) starts recording on the connected session; the status bar shows "REC" until Tools ▸ Stop recording (the same key), then Parley asks for a name and whether the macro is for all sessions or this one. A macro is the 5250 keys and text typed (not mouse clicks), with a "wait until the keyboard unlocks" step after each AID key. Text typed into a non-display field is never recorded: the step becomes "type the password", which plays the session's saved password (D11) or asks for it. Recording stops on its own when the connection ends. Macros are saved as readable text files next to the settings (the format is a decision, D14, taken when this is built), one per macro, so users can edit and share them. | 3 |
| R-KEY-4 | **Run a macro** (from IDEA-11). Tools ▸ Run macro ▸ lists the macros for all sessions and for this one; a key in the keyboard map can run a given macro; the session dialog's "Run macro" (disabled since R-UI-5) runs one after sign-on. A running macro shows in the status bar, waits at each "wait" step until the host unlocks the keyboard (giving up after 30 s with a message), stops at an operator error (the error stays on screen), and stops when the user presses Reset or Esc, or the connection ends. Tools ▸ Macros… lists the macros to rename, delete or open their folder. A macro file that cannot be read is listed with the reason and cannot be run. | 3 |

## 6. Ideas

Ideas that might become requirements later are in [IDEAS.md](IDEAS.md) (`IDEA-n`, with their own next free ID).

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
| D13 | How does Parley tell that a new version is available (R-UI-11)? | **As Tortuga (its D63): a check at start-up, on by default, that can be turned off, plus "Check now" in Settings → About**; the source is GitHub's latest published release, asked with `ureq` (blocking, on a thread; rustls with the OS trust store as D6) from `parley_core::update`; the result is a quiet link in the menu bar, never a pop-up (2026-11-12). It is the one connection Parley makes besides the IBM i hosts; it sends nothing but the request. Tests use a fake checker or a local server, never GitHub. | Only a button the user clicks; no check at all. |
| D10 | Are installers signed? | **Not yet**: macOS and Windows warn on first start (docs/RELEASING.md says how to go on). Signing needs an Apple Developer ID and a Windows certificate. | Sign from the first release. |

## 8. How to change this roadmap

- New work gets a requirement in §5 with the next free ID of its area; IDs are never reused or renumbered.
- A requirement that is implemented, tested and checked in the running app moves to [REQUIREMENTS.md](REQUIREMENTS.md),
  with how it was verified in [FEATURES.md](FEATURES.md).
- A choice that future work must respect becomes a decision in §7.
- An idea is not a plan: it goes in [IDEAS.md](IDEAS.md) until it is picked, then becomes requirements here.

Next free IDs (removed rows keep their numbers): `R-P-5` · `R-NET-7` · `R-DS-6` · `R-EBC-2` · `R-KEY-5` · `R-UI-13` · `R-NF-2` · `D14` (ideas: last line of IDEAS.md).
