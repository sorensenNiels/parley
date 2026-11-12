# Parley

A TN5250 terminal emulator for IBM i, written in Rust.
Targets: Windows, Linux and macOS desktop only (roadmap D5).

**Scope rule:** `docs/ROADMAP.md` is the plan. Build what the user asks for; when choosing what comes next,
follow the roadmap's order. If a request is unclear, or would change a decision recorded in the roadmap, ask before building.

## Planning documents

| File | Holds | Rule |
|---|---|---|
| `docs/ROADMAP.md` | Vision, principles, phases, open requirements (`R-NET-1`), decisions (`D1`), next free IDs. | The plan and the spec for what is not done. Implement requirements as written; record new decisions there. |
| `docs/IDEAS.md` | Ideas (`IDEA-n`) that may become requirements later, by theme; own next free ID on its last line. | Not a plan: new ideas go here. When one is picked, write requirements in the roadmap and mark the idea. |
| `docs/FEATURES.md` | Implemented **and verified** features by requirement ID, and how each was verified. | Add an entry only when the requirement is fully met, tested, and checked in the running app; record which OS. Otherwise it goes under "In progress". |
| `docs/REQUIREMENTS.md` | The text of every completed requirement. | When a requirement is verified, move its row here from the roadmap. |

## Working efficiently

Context is the budget; spend it on code that changes.
- Once they grow, don't read the planning docs or `README.md` whole: `grep -n` the ID (`"R-NET-1 "`, `"| D3 "`) or
  heading, then read that range. Next free IDs are the roadmap's last line (ideas: the last line of `IDEAS.md`).
- Source files over ~500 lines: `grep -n` the symbol, then read with offset/limit. Don't re-read a file after editing it.
- While working, run the narrowest check (`cargo test -p <crate> <filter>`); run `/verify` once at the end. Send long
  command output to a file under `target/` and grep it instead of printing it.
- `/audit-context` re-checks these files and settings for token cost and stale references.

IDs are never reused or renumbered. Reference requirement IDs in commit messages and tests.
When building something that has no requirement yet, add one to the roadmap (next free ID in its area).

## Layout

| Path | Purpose |
|---|---|
| `crates/parley-core` | UI-agnostic TN5250 engine. No UI deps, ever. Sans-I/O protocol (D4): `telnet` (negotiation, records), `record` (5250 header), `session` (`Session`: `datastream` commands and orders, `input` keys and field rules, `reply` what goes to the host), `screen` (buffer, attributes, format table), `signon` (RFC 4777 password substitutes, `Password` with hidden `Debug`), `macros` (macro steps and their text format, D14; recording and playing are `session/playback`), `ebcdic` (code pages; `tables.rs` is generated). `net` runs a session over TCP/TLS on a thread. `update` asks GitHub for the latest release (D13). `examples/probe.rs` drives a real host without the UI. |
| `crates/parley-ui-egui` | The UI, built with egui 0.37: `ParleyUi` (frame and actions), `theme` (the design system shared with Tortuga), `dialog` (dialog frame, close button, actions), `menu` (menu bar, toolbar), `status` (status bar), `terminal` (the screen widget), `keymap` (PC keys to 5250 keys), `connect` (start screen), `profile` (saved sessions, `Keychain` trait), `sessions` (session dialog), `settings` (settings and their dialog), `fonts` (the terminal font: bundled or installed), `updates` (the background check for a new version), `macros` (`MacroStore` trait, macro dialogs). Fonts in `assets/fonts/`. No platform code. |
| `crates/parley-app` | Thin eframe launcher (`parley` binary): window, command line, saved sessions, `keychain` (OS keychain via `keyring`), `macros` (macro files), `--screenshot` for development. Also the app icon (`icons/`, `make icons`) and the installer settings (`[package.metadata.packager]`, docs/RELEASING.md). |
| `scripts/` | One-time dev setup per OS; `gen-ebcdic.py` regenerates the code page tables; `release.sh` (`make release-patch/minor/major`); `render-icons.py`. |
| `deny.toml` | Dependency licence/advisory policy (`cargo deny check`). |

## Commands

Use the Makefile (`make help` lists everything). The main ones:

```sh
make verify      # fmt-check + lint + test + deny: run before calling work done (= CI)
make test        # all tests; make test-core for the engine only, make test-ui for the UI
make lint        # clippy -D warnings
make run         # desktop app (cargo run); sets GALLIUM_DRIVER=d3d12 under WSL
make probe HOST=pub400.com KEYS=help,reset   # a real host without the UI: prints each screen (ARGS="--macro FILE" plays a macro)
make package     # installers for this OS (docs/RELEASING.md); make release-patch|minor|major releases
```

Checking against a real IBM i: PUB400.COM is a free public one (sign-on screen without an account). Never sign on
or send credentials to it from Claude's side. `PARLEY_TRACE=file` records what a host sends (inbound only).
To see the app, `cargo run -p parley-app -- HOST --screenshot target/app.ppm` saves the window after 6 s.

## Conventions

- Edition 2024, stable toolchain pinned by `rust-toolchain.toml`. Lints are workspace-wide
  (`clippy::pedantic` on, `unsafe_code` denied) — fix warnings; if a lint must be silenced use
  `#[expect(lint, reason = "...")]`. There is no `unsafe` code.
- Errors: a `thiserror` enum in an `error` module of `parley-core` (create it with the first fallible code); the UI converts
  to display strings at the edge. No `unwrap()` outside tests.
- Protocol code is tested with byte-exact fixtures: recorded host traffic or hand-built records in tests, never a
  live IBM i in `cargo test`. Cite the spec section (RFC 1205/2877, IBM 5250 Functions Reference) next to the code.
- Tests: unit tests next to the code; whole-session tests in a `tests` folder in `parley-core` (create it with the first one).

## Code quality

Clippy enforces most mechanical rules. These cover what it can't check.

**KISS**
- Write the simplest code that meets the requirement. Add a trait, generic, macro or builder only when a
  second real use exists today (test fakes count). Don't add one for a phase that is still on the roadmap.
- Keep non-test code in a module under ~1000 lines; split into submodules when it grows past that.
  If a block inside a function needs a heading comment, it is a function: give it a name.

**DRY: search before you write**
- Before writing a helper, grep for an existing one. A rule that has to agree everywhere (e.g. "is this field
  input-capable?") lives in one function in `parley-core`.
- The second time you copy a block of more than ~10 lines, extract it into a function.

**State and types**
- Use enums, not `bool` pairs or magic numbers, for states, commands, orders and AID codes. In `match` on this
  workspace's own enums, list the variants rather than `_ =>` when a new variant would need a decision.

**Errors and panics**
- `expect("…")` only for real invariants, with a message that states the invariant. Never on host data.
- Data from the host is untrusted: never index a buffer at an offset taken from the data stream without a bounds
  check (`.get(i)`, not `[i]`); a malformed record is an error, not a panic.
- Don't discard a `Result` (`let _ =`, `.ok();`) unless failure is harmless, and say why in a comment.
- `parley-core` and `parley-ui-egui` never print: return the error. Only the binaries print, with a `parley:` prefix.

**Secrets**
- A type that holds passwords gets a hand-written `Debug` that hides them. Passwords never go into errors or logs,
  and trace output of host traffic must blank sign-on fields.

**Tests**
- Every bug fix comes with a test that fails without the fix. Name tests as sentences about behaviour
  and put the requirement ID in a comment or the test name.
- Test edge cases: empty and truncated records, buffer addresses at and past the screen edge, non-ASCII text (`"æøå"`).

## UI look

Parley looks like Tortuga (`~/Projects/tortuga`, decision D12): `theme.rs` and `dialog.rs` are ports of Tortuga's
`theme.rs` and `components/modals.rs`, and the fonts in `assets/fonts/` are Tortuga's. Before restyling anything,
check how Tortuga does it; a change to the shared look goes into both repositories.
- **Use the `theme` building blocks** where one exists, not raw egui styling: `primary_button` for the one main
  action (Connect, Save, Enter), `danger_button` for Delete, `icon_button`/`ghost_button` for small secondary
  actions, `switch`/`switch_row` for on/off options (not checkboxes), `segmented` for a choice of two to four,
  `section_label` for small upper-case group headings, `hint`/`warning`/`error` for secondary text, `card` for a
  bordered block, `sized_edit` for text fields in a `Grid`, `button_row` for right-aligned buttons.
- **Text:** Inter for the UI, `theme::semibold(..)` for emphasis and headings (egui's `.strong()` only changes the
  colour), JetBrains Mono for monospace. Don't set font sizes inline except as Tortuga does (section labels, badges).
- **Colours** come from `theme::palette(ui)` (light and dark variants); never hard-code a `Color32` in a component.
  The one exception is the 5250 screen (`terminal::color`, `terminal::BACKGROUND`). Check new UI in light and dark.
- **Dialogs** go through `dialog::dialog` (frame, title, 🗙 close button) and end with Cancel plus a filled confirm
  button on the right (`dialog::actions` or `theme::button_row`). A dialog fixes its width; content that may not
  fit goes in `dialog::fitting_scroll_area`. Settings pages use the settings dialog's `group`/`row` layout.
- **Layout traps** (each cost a bug here): in a `Modal`, nothing may size itself from the dialog's own height or
  width (a vertical `separator()`, a `Frame` in a `Grid` cell, a plain `ScrollArea::max_height`): the dialog then
  grows every frame and clicks miss. Give such content a fixed size, as the settings dialog does. Every dialog has a
  kittest test that its buttons stay in place over a few frames.
- **Glyphs:** egui panics on a character its fonts lack. Use only symbols listed in
  `bundled_fonts_have_every_symbol_the_ui_uses` (`lib.rs`) and add a new one there first.

## Project skills

- `/verify` — full local quality gate; run before saying work is done.
- `/audit-context` — recurring audit of these files, skills and settings for token cost and stale references.
