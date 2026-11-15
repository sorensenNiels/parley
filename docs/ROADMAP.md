# Parley roadmap

> The plan, and the spec for everything that is not done yet. Completed requirements move to
> [REQUIREMENTS.md](REQUIREMENTS.md), and how each was verified is in [FEATURES.md](FEATURES.md).
> Look things up with `grep -n` (an ID such as `"R-NET-1 "`, `"| D3 "` or a heading) rather than reading the whole file.

**Status of this document:** first written while setting up the development environment (2026-11-12). Phases 1–4
are built and verified (the last on 2026-11-15); Phase 20, everyday screen work, is done too.

## 1. Vision

Parley is a fast, modern TN5250 terminal emulator for IBM i, written in Rust. It is a desktop app for Windows, Linux and
macOS from one code base, starts instantly, and feels at home for people who use green screens all day, with a
modern take on what surrounds the screen: sessions, keyboard mapping, copy and paste, and scripting.

**Parley SQL**, the SQL window for IBM i (Parley's SQL add-on and the program `parley-sql`), has a plan of its own:
[sql/ROADMAP.md](sql/ROADMAP.md). The two share this repository, the principles below and one sequence of IDs
(see §8).

## 2. Product principles

| ID | Principle |
|---|---|
| R-P-1 | **Keyboard first.** Every 5250 function key and action is reachable from the keyboard, with mappings people know from existing emulators. |
| R-P-2 | **Correct before clever.** The 5250 data stream is implemented to IBM's specification and tested against recorded host traffic. |
| R-P-3 | **Secure by default.** TLS (port 992) is the default for new sessions; Parley never stores a password and never signs on by itself (R-NET-7, D16). |
| R-P-4 | **One engine.** All protocol logic lives in `parley-core`, with no UI or platform code, so it can be tested on its own. *Exception (D20, 2026-11-13): the SQL add-on runs its database work in a Java helper with IBM's JTOpen; Parley's side of it, the helper's protocol, is in `parley-core`.* *Changed by D36 (2026-11-15): Parley SQL's engine is `parley-sql-core`, beside `parley-core`, held to the same rule.* |
| R-P-5 | **Two programs, apart in the code** (2026-11-15, asked by the owner; D36). Parley (the terminal) and Parley SQL are separate crates: `parley-core` and `parley-ui-egui` are Parley's and never depend on `parley-sql-core` or `parley-sql-ui`; Parley SQL builds on Parley's engine only to connect and for code pages, and on its UI only for the shared look (theme, dialogs, icons, the settings frame). Each program keeps its own settings type in its own storage. `cargo deny` (`make verify`) fails a build that breaks this. |

## 3. Status

| Phase | State |
|---|---|
| Phase 0: development environment | Done (2026-11-12): workspace, Makefile, setup scripts, CI, lints, Claude Code setup. |
| Phase 1: connect and show a screen | Done (2026-11-12): verified on Windows, macOS, Linux and WSL2 (FEATURES.md). |
| Phase 2: saved sessions and auto sign-on | Done (2026-11-12): verified on Windows, macOS, Linux and WSL2 (FEATURES.md). Auto sign-on and saved passwords were removed again in Phase 3 (R-NET-7, D16). |
| Phase 3: keyboard maps and macros | Done (2026-11-12): verified on Windows, macOS and Linux (FEATURES.md): R-DS-5, keyboard maps (R-KEY-2), macros (R-KEY-3, R-KEY-4), add-ons (R-ADD-1, R-ADD-2), auto sign-on removed (R-NET-7, D16). The macro editor (R-KEY-5), Stop in the status bar (R-KEY-6) and import and export of macros (R-KEY-7) verified 2026-11-15. |
| Phase 4: a modern look | Done (2026-11-15): verified on Windows, macOS, Linux and WSL (FEATURES.md): icons (R-UI-13, D17), an interactive status bar (R-UI-14), one header with the session (R-UI-15), the 5250 keys in a Keys menu in the status bar (R-UI-16), a shortcut sheet (R-UI-17), no error for a key in a protected area (R-KEY-8); both commands after installing (R-NF-2). |
| Phases 5–19: SQL | Parley SQL's phases: [sql/ROADMAP.md](sql/ROADMAP.md). |
| Phase 20: everyday screen work | Done (2026-11-15): checked by the owner in the running app (FEATURES.md): R-UI-20..23, R-NET-10. |
| Phase 21: protocol gaps | Started (2026-11-15): R-DS-6, R-DS-7 (IDEA-7), R-DS-8 (IDEA-4) done; R-UI-24, R-UI-25 (IDEA-6) done; R-KEY-9 (IDEA-2) done; R-KEY-10 (IDEA-3) done; R-KEY-11 (IDEA-14) done; IDEA-5 to come. |

## 4. Phases

### Phase 0 — Development environment (done)

Workspace with `parley-core`, `parley-ui-egui`, `parley-app`; `make verify` as the local quality gate (= CI);
setup scripts for Linux, macOS and Windows; the app opens a window with a blank 24×80 screen.

### Phase 1 — Connect and show a screen (done)

Open a session to an IBM i, sign on and use simple screens: R-NET-1..3, R-DS-1..4, R-EBC-1, R-KEY-1, R-UI-1..4.
Known gaps are IDEA-1..10 ([IDEAS.md](IDEAS.md); IDEA-1 became R-DS-5).

### Phase 2 — Saved sessions and auto sign-on (done)

Several named sessions with their own settings, a session dialog, passwords in the keychain, auto sign-on (typed or
bypassed per RFC 4777) and keep-alive: R-UI-5, R-NET-4, R-NET-5. *(Auto sign-on and the saved passwords were removed in
Phase 3 as a security risk: R-NET-7, D16.)* A roomier look and a settings dialog with the terminal
font: R-UI-6. Close buttons on popups: R-UI-7. Settings in pages: R-UI-8. Tortuga's look: R-UI-9. App name in the menu bar, Help on the right: R-UI-10. New version available, with the download command on Linux:
R-UI-11, R-UI-12.

### Phase 3 — Keyboard maps and macros (done)

First a correctness step (R-P-2): the points of the data stream built from memory checked against IBM's manual
(R-DS-5, from IDEA-1). Then keyboard maps chosen per session (R-KEY-2, from IDEA-9) and macros, recorded (R-KEY-3) and run (R-KEY-4), from IDEA-11. Settings ▸ Add-ons
(R-ADD-1, D15), with macros as the first add-on (R-ADD-2). Auto sign-on and saved passwords removed (R-NET-7, D16);
R-NET-6 (why a bypass sign-on failed) is dropped with it.

### Phase 4 — A modern look (done)

The owner chose these from mockups of three options per area (2026-11-12): an icon font (R-UI-13, D17); a status
bar of pills that shows only what is true and can be clicked (R-UI-14, option S3); one header row with the session
instead of the menu bar plus toolbar (R-UI-15, option T2); the seldom-used 5250 keys in a Keys menu in the status bar (R-UI-16; first a key strip under the screen, changed 2026-11-13); and
a keyboard shortcut sheet over the screen instead of the side panel (R-UI-17, option K1). Built in that order.
Labels from the screen ("F3=Exit") on the function keys stayed IDEA-19, now R-UI-20 in Phase 20.

### Phase 20 — Everyday screen work (done)

Chosen by the owner (2026-11-15) as the first terminal phase after Phase 4: what makes a day at the green screen
easier, all in the UI, without new protocol. In this order: hotspots (R-UI-20, from IDEA-19), find on the screen
(R-UI-21, IDEA-22), selecting and copying part of the screen, also as a table (R-UI-22, IDEA-20), 5250 colour schemes
(R-UI-23, IDEA-23) and reconnecting by itself when the network drops (R-NET-10, IDEA-16). Then, as candidates for
later phases: the protocol gaps (IDEA-7 first, then IDEA-4, 6, 2, 3, 5, 14), several sessions as tabs (IDEA-15),
importing sessions from ACS and PCOMM (IDEA-17).

### Phase 21 — Protocol gaps (started)

The commands and fields Parley still turns away (R-P-2), from IDEA-7, then IDEA-4, 6, 2, 3, 5 and 14 in that order,
each checked against IBM's 5494 Remote Control Unit Functions Reference (SC30-3533-04, chapter 15) and RFC 1205.
First Save/Restore Partial Screen (R-DS-6) and Read MDT Immediate Alternate (R-DS-7), which a host now gets an
error for.

### Later (unordered, to be planned)

Printer sessions (IDEA-18); see [IDEAS.md](IDEAS.md). SQL work is planned in [sql/ROADMAP.md](sql/ROADMAP.md).

## 5. Requirements

Open requirements, in the order they are planned. The done ones are in [REQUIREMENTS.md](REQUIREMENTS.md); new
ones come from [IDEAS.md](IDEAS.md). Areas: `NET` connection and telnet, `DS` the 5250 data stream,
`EBC` character sets, `KEY` keyboard, `UI` user interface, `ADD` add-ons, `NF` non-functional. The `SQL` area, and
the SQL window's add-on switches, probes and assistant, are in [sql/ROADMAP.md](sql/ROADMAP.md).

| ID | Requirement | Phase |
|---|---|---|

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
| D10 | Are installers signed? | **Not yet**: macOS and Windows warn on first start (docs/RELEASING.md says how to go on). Signing needs an Apple Developer ID and a Windows certificate. | Sign from the first release. |
| D11 | Where are passwords kept? *Replaced by D16 (2026-11-12): Parley keeps none.* | **The operating system's keychain** through the `keyring` crate (Keychain Services on macOS, Credential Manager on Windows, Secret Service on Linux), filed under service "Parley" and the session's name; the UI reaches it through a `Keychain` trait so tests use memory (2026-11-12). Without a keychain (e.g. WSL with no Secret Service) the password is not saved, and the dialog says why. | Encrypt the settings file with a master password; never save passwords. |
| D12 | How does Parley look? | **Like Tortuga** (2026-11-12): the same design system, ported as `parley-ui-egui/src/theme.rs` (fonts, palette, sizes, visuals, building blocks) and `dialog.rs` (dialog frame, close button, actions, `fitting_scroll_area`), the same fonts (`assets/fonts`), and the same UI rules (CLAUDE.md "UI look"). A change to the shared look is made in both repositories. Parley's own additions: the terminal font family and the 5250 screen colours. | A look of Parley's own. |
| D13 | How does Parley tell that a new version is available (R-UI-11)? | **As Tortuga (its D63): a check at start-up, on by default, that can be turned off, plus "Check now" in Settings → About**; the source is GitHub's latest published release, asked with `ureq` (blocking, on a thread; rustls with the OS trust store as D6) from `parley_core::update`; the result is a quiet link in the menu bar, never a pop-up (2026-11-12). It is the one connection Parley makes besides the IBM i hosts; it sends nothing but the request. Tests use a fake checker or a local server, never GitHub. | Only a button the user clicks; no check at all. |
| D14 | How are macros saved (R-KEY-3)? | **One UTF-8 text file per macro**, `<name>.macro` in a `macros` folder beside the settings (eframe's folder for the app id `parley`: `~/.local/share/parley/macros` on Linux, `~/Library/Application Support/parley/macros` on macOS, `%APPDATA%\parley\data\macros` on Windows), behind a `MacroStore` trait so tests use memory (2026-11-12). One step per line: `text "…"`, a key name (`enter`, `f3`, `field-exit`, …), `wait`, `password`, `move ROW COLUMN`; an optional `session "NAME"` line first keeps a macro to one saved session; `#` starts a comment (`parley_core::macros`). Passwords are never in the file. The name is the file name, so names may not hold `/ \ : * ? " < > \|` or start with a dot. | JSON or TOML through serde; all macros inside the settings file. |
| D15 | How are features not everyone needs added (add-ons)? Changes R-KEY-3, R-KEY-4. | **As Tortuga (its D78): add-ons are built in, each with its own default, turned on or off in Settings ▸ Add-ons, and not loaded while off** (2026-11-12). One `AddOn` enum in `parley-ui-egui/src/addons.rs` is the list the rest of the UI asks ("is this add-on on?"); the settings keep only the ids that differ from each default. Each add-on keeps its code in its own modules (macros: `parley-ui-egui/src/macros.rs`, `parley_core::macros`, `session/playback`). Not a plugin system for third parties. Macros are the first add-on, on by default since they are a main feature of Phase 3 (the owner, 2026-11-12). | Hide features in a Settings page without unloading them; a Cargo feature per add-on (separate builds); plugins loaded at run time. |
| D16 | Should Parley sign on by itself (R-NET-4)? Changes D11, R-P-3, R-KEY-3, R-KEY-4. | **No: auto sign-on and saved passwords are removed** (the owner, 2026-11-12: a security risk). A password kept on the computer lets anyone with the unlocked desktop open a session as that user, and the bypass sign-on also needs the host's QRMTSIGN opened up. The operator signs on, or a macro asks for the password each time; passwords already saved are deleted from the keychain (R-NET-7). The RFC 4777 password-substitute code is removed from `parley-core`; `keyring` stays in `parley-app` only to delete the old passwords. | Keep it with a warning; keep it but ask for the password at each connect (bypass still keeps it out of clear text); make it an add-on, off by default. |
| D17 | How does Parley show icons (R-UI-13)? | **The Phosphor icon font (Regular, MIT licence) as a font file in `assets/fonts`, with the codepoints Parley uses named in `parley-ui-egui/src/icons.rs`** (2026-11-12). Not the `egui-phosphor` crate: its release is built for egui 0.36 and Parley uses 0.37, and the font is all that is needed. The font has a font family of its own and icons are drawn beside text, never inside it: Inter has glyphs of its own at some of Phosphor's private-use codepoints (U+E136 is a circled ">"), and Phosphor maps a–z to blanks (found 2026-11-12 in the first screenshot). Under D12 the same font goes into Tortuga when it adopts icons. | The `egui-phosphor` crate; icons drawn as shapes in code; text only. |
| D18 | What does a key typed in a protected area do (R-KEY-8)? Changes R-KEY-1. | **Nothing, quietly** (the owner, 2026-11-13): a real 5250 inhibits the keyboard with operator error 0005 until Reset, which PC users find hostile, as D8 found for Backspace. The key is dropped and the operator moves on with Tab or a click. | The 5250 behaviour; jump to the next input field and type the character there. |
| D36 | How are Parley and Parley SQL kept apart in the code (R-P-5)? Changes R-P-4, D20's "Parley's side … is in `parley-core`" and D26's "same window". | **Four crates and the launcher** (the owner asked, 2026-11-15): `parley-core` (the TN5250 engine, connecting, code pages), `parley-sql-core` (host servers, the helper's protocol, SQL text, transfers, the assistant: once `parley_core::sql` and `::hostserver`), `parley-ui-egui` (Parley's UI and the look both share: theme, dialogs, icons, the header, the settings frame, Appearance and About), `parley-sql-ui` (the SQL window and Parley SQL around it). Parley starts Parley SQL with a session's name and `SessionConfig` (R-SQL-70); the launcher turns them into Parley SQL's connection. Each program's settings are a flat struct of their own with the field names they always had, because eframe's RON storage cannot read old settings into a `#[serde(flatten)]` part (tried: it fails and would reset everyone's settings); a test reads 0.3.1's saved settings into both. The SQL assistant's switch is Parley SQL's (`Assistant`), kept under its old id. `cargo deny` bans the SQL crates from Parley's (`deny.toml`). | Modules in the same crates (nothing stops a new dependency); one shared settings type (each program carries the other's fields); flattened settings (breaks stored settings); a separate repository (the shared look would drift, D12). |
| D37 | How does Parley sound the 5250 alarm (R-UI-25)? | **A short sine beep through the `rodio` crate, played by the launcher (`parley-app`)**, which hands `ParleyUi` a function to call (as it does for starting Parley SQL): `parley-ui-egui` keeps no platform code. On Linux this needs ALSA (`libasound2-dev` to build: CI, the release workflow and `scripts/setup-linux.sh`; `libasound2` in the .deb); no sound device is not an error (2026-11-15, the owner chose sound and flash). | The terminal bell character (no terminal behind a desktop app); each OS's own beep call (needs `unsafe` or a crate per OS); flash only. |

Decisions D19–D27 are about SQL and are in [sql/ROADMAP.md](sql/ROADMAP.md) §6.

## 8. How to change this roadmap

- New work gets a requirement in §5 with the next free ID of its area; IDs are never reused or renumbered.
- A requirement that is implemented, tested and checked in the running app moves to [REQUIREMENTS.md](REQUIREMENTS.md),
  with how it was verified in [FEATURES.md](FEATURES.md).
- A choice that future work must respect becomes a decision in §7.
- An idea is not a plan: it goes in [IDEAS.md](IDEAS.md) until it is picked, then becomes requirements here.
- **One sequence of IDs for both roadmaps.** Parley SQL keeps its own `R-SQL` counter; every other area, decisions
  (`D`), phases and ideas (`IDEA`, in [IDEAS.md](IDEAS.md)) take their next number here, whichever roadmap uses it,
  so a number never means two things.

Next free IDs (removed rows keep their numbers): `R-P-6` · `R-NET-11` · `R-DS-9` · `R-EBC-2` · `R-KEY-12` · `R-UI-26` · `R-ADD-4` · `R-NF-3` · `D38` · `Phase 22` (`R-SQL`: [sql/ROADMAP.md](sql/ROADMAP.md); ideas: last line of IDEAS.md).
