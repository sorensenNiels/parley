# Parley 5250

A TN5250 terminal emulator for IBM i, written in Rust. Shown as "Parley 5250" since 2026-11-16 (D40); crates, the
`parley` command, folders, the app id and the keychain service keep the name Parley, and so do the releases.
Targets: Windows, Linux and macOS desktop only (roadmap D5).

**Scope rule:** `docs/ROADMAP.md` is the plan for Parley 5250 (the terminal; "Parley" in the docs), `docs/sql/ROADMAP.md` for Parley SQL (the SQL
window: Parley's SQL add-on and `parley-sql`). Build what the user asks for; when choosing what comes next,
follow the roadmap's order. If a request is unclear, or would change a decision recorded in the roadmap, ask before building.

## Planning documents

| File | Holds | Rule |
|---|---|---|
| `docs/ROADMAP.md` | Vision, principles, phase status, unfinished phases, open requirements (`R-NET-1`), next free IDs. | The plan and the spec for what is not done. Implement requirements as written. |
| `docs/phases/` | One file per finished phase (`phase-NN-<slug>.md`, plus `between-phases.md`): its story, the full text of its requirements, how each was verified. | A verified requirement's row and its verification move here; a finished phase's §4 story too. |
| `docs/REQUIREMENTS.md` | Index of finished requirements: ID, title, link to the phase file. | One row per ID, added when the row moves to its phase file. |
| `docs/FEATURES.md` | Checks of requirements still in progress; links to the phase files. | Verification goes into the phase file only when the requirement is fully met, tested, and checked in the running app; record which OS. |
| `docs/DECISIONS.md` | Decisions (`D1`). | Record new decisions here. |
| `docs/IDEAS.md`, `docs/ideas/` | Ideas (`IDEA-n`) that may become requirements, one file per theme; next free ID on `IDEAS.md`'s last line. | Not a plan: new ideas go here. When one is picked, write requirements in the roadmap and mark the idea. |
| `docs/sql/` | The same layout for SQL: every `R-SQL`, the SQL probes (R-NET-8/9), the assistant switch (R-ADD-3), decisions D19–D35 and D39 in `sql/DECISIONS.md`. | Same rules. One sequence of numbers for both: `R-SQL` is counted on the SQL roadmap's last line; every other area, `D`, phases and `IDEA` on `docs/ROADMAP.md`'s and `docs/IDEAS.md`'s last lines, whichever product uses it. |
| `docs/PRD.md`, `docs/ARCHITECTURE.md` | The product summary and the design with diagrams (`docs/images/`), for readers new to Parley. | Summaries that link IDs, not specs: update them when a phase ends or the design changes; never copy requirement text into them. |

## Working efficiently

Context is the budget; spend it on code that changes.
- Don't read the planning docs or `README.md` whole: look an ID up with `grep -rn "R-UI-14 " docs/` (or `"| D3 "`,
  a heading), then read that range. Next free IDs are the roadmap's last line (`R-SQL`: `docs/sql/ROADMAP.md`'s; ideas: the last line of `docs/IDEAS.md`).
- Source files over ~500 lines: `grep -n` the symbol, then read with offset/limit. Don't re-read a file after editing it.
- While working, run the narrowest check (`cargo test -p <crate> <filter>`); run `/verify` once at the end. Send long
  command output to a file under `target/` and grep it instead of printing it.
- `/audit-context` re-checks these files and settings for token cost and stale references.

IDs are never reused or renumbered. Reference requirement IDs in commit messages and tests.
When building something that has no requirement yet, add one to the roadmap (next free ID in its area).

## Layout

| Path | Purpose |
|---|---|
| `crates/parley-core` | Parley's UI-agnostic TN5250 engine. No UI deps, ever. Sans-I/O protocol (D4): `telnet` (negotiation, records), `record` (5250 header), `session` (`Session`: `datastream` commands and orders, `input` keys and field rules, `checks` mandatory and self-check fields, `typeahead` keys kept while the host works, `gui` GUI structured fields, `reply` what goes to the host), `screen` (buffer, attributes, format table; `screen/hotspots` what a click on the screen does, `screen/region` find and copy, `screen/extended` extended attributes, `screen/windows` GUI windows), `password` (`Password` with hidden `Debug`; Parley keeps none and never signs on by itself, D16), `macros` (macro steps and their text format, D14; recording and playing are `session/playback`), `ebcdic` (code pages; `tables.rs` is generated). `net` runs a session over TCP/TLS on a thread (`net::open_stream` also connects Parley SQL's host servers). `update` asks GitHub for the latest release (D13). `examples/probe.rs` drives a real host without the UI. |
| `crates/parley-sql-core` | Parley SQL's UI-agnostic engine (D20, D36), on `parley-core` only to connect, for code pages and `Password`. `hostserver` (the IBM i host servers' records and port mapper, D19), `net` (looks a port up, signs on), `error` (its own `Error`), `protocol` to the Java helper, `java` finds Java, `helper` runs it, `runner` on a thread, `history` the SQL history's runs, `text` splits, colours and formats statements and guards bulk changes, `edit` the editor's text changes (comment, indent, move lines, pairs, keyword case), `snippet` snippet files, placeholders and the starters in `assets/snippets`, `message` message IDs and help, `catalog` the catalog queries, `generate` a library's SQL and backups (R-SQL-93..96), `services` the IBM i Services the System node reads, `complete` what is typed and the tables a statement names, `params` parameter markers, `export` Copy as and binary dumps, `transfer` downloads to CSV and Excel (R-SQL-71, R-SQL-72), `upload` CSV files into a table (R-SQL-73), `compare` two results, `assistant` the SQL assistant's providers, D25, D27: `anthropic` and `openai` formats. `examples/sqlprobe.rs` asks a host for its SQL routes; `examples/askprobe.rs` asks the assistant's provider with a made-up catalog. |
| `crates/parley-ui-egui` | Parley's UI, built with egui 0.37, and the look Parley SQL shares (never uses Parley SQL's crates, R-P-5, D36): `ParleyUi` (frame and actions), `theme` (the design system shared with Tortuga), `dialog` (dialog frame, close button, actions), `menu` (the header: menus and the session pill), `keys` (the Keys menu of 5250 keys in the status bar), `status` (status bar), `shortcuts` (the shortcut sheet), `icons` (the icon font's codepoints, D17), `terminal` (the screen widget, its selection and hotspots; `terminal/find` the find bar, `terminal/colors` the colour schemes, `terminal/windows` window frames), `keymap` (keyboard maps: PC keys to 5250 keys and actions), `connect` (start screen), `profile` (saved sessions), `sessions` (session dialog), `settings` (Parley's settings and dialog; shared: `settings/layout` the dialog's frame and rows, `settings/appearance` theme and text, `settings/about`, `settings/key_capture`; `settings/keyboard` the Keyboard page), `addons` (the `AddOn` list and which are on, D15; the `Toggle` trait), `fonts` (the terminal font: bundled or installed), `updates` (the background check for a new version), `macros` (`MacroStore` trait, macro dialogs; `macros/editor` edits a macro's text). Fonts in `assets/fonts/`. No platform code. |
| `crates/parley-sql-ui` | Parley SQL's UI (D26, D36), on `parley-ui-egui`'s look and `parley-sql-core`. The crate root is the SQL window (`SqlWindow`, R-SQL-6): `keys` its shortcuts (R-SQL-60), `editor` its scripts (`editor/complete` the completion list, `editor/editing` its editing keys, `editor/colour` colours and marks, `editor/gutter` line numbers and ▶, `editor/signature` a routine's parameters, `editor/find` the find bar, `editor/error_mark` an error's place), `results` the result tabs, `grid` a result, `side` the side panel on the left (Explorer above Snippets), `history` the History tab, `snippets` the Snippets panel and `SnippetStore`, `tools` Explain, job log and library list, `database` the second connection, `browser` the schema browser (`browser/groups` library groups, `browser/generate` its generated SQL), `system` the System node's views and viewer windows (Phase 17), `parameters` the parameter form, `chart` charts, `download` and `upload` data transfer (Phase 15), `assistant` the Ask row and the `ApiKeys` trait, `hosting` opens the window from the settings and keeps what it changed. `app` is Parley SQL (`app/bars` its header and status bar, `app/connections` saved connections); `settings` its own `Settings` and pages (`settings/assistant`, `settings/sql_keys` the Shortcuts page). |
| `crates/parley-app` | Thin eframe launchers: `parley` (`main.rs`, `desktop.rs`) and `parley-sql` (`sql_main.rs`, Parley SQL, D26), over a small library of what both use; the only crate that uses both programs' crates. `parley`: window, command line, saved sessions, `keychain` (deletes the passwords Parley 0.2 saved, R-NET-7), `macros` (macro files; imports and exports them through the file window, rfd with GTK 3 on Linux), `helpers` (where the SQL helper's jars are; the CSV saver), `api_keys` (the assistant's keys in the keychain, D27), `snippets` (snippet folders), `sql_start` (Parley SQL's command line; Parley's Tools ▸ SQL… starts Parley SQL with it, R-SQL-70), `--screenshot` for development. Also the app icon (`icons/`, `make icons`) and the installer settings (`[package.metadata.packager]`, docs/RELEASING.md). |
| `helpers/sql` | The SQL helper (D20): Java, run by the SQL add-on over JSON lines on stdin/stdout, with IBM's JTOpen (`jt400.jar`, fetched pinned by `scripts/fetch-jt400.sh`, never in git). `NOTICE.txt` and `JTOpen-LICENSE.html` ship with it. Built by `make helper` (`scripts/build-helper.sh`), which needs a JDK 11+. |
| `scripts/` | One-time dev setup per OS; `gen-ebcdic.py` regenerates the code page tables; `release.sh` (`make release-patch/minor/major`); `render-icons.py`. |
| `deny.toml` | Dependency licence/advisory policy (`cargo deny check`). |

## Commands

Use the Makefile (`make help` lists everything). The main ones:

```sh
make verify      # fmt-check + lint + test + deny + helper: run before calling work done (= CI; needs a JDK)
make helper      # build and check the SQL helper (Java) into target/helpers
make test        # all tests; make test-core for the engines, make test-ui for the UIs, make test-sql for Parley SQL's
make coverage    # line coverage of all tests (cargo-llvm-cov) into target/coverage
make lint        # clippy -D warnings
make run         # desktop app (cargo run); sets GALLIUM_DRIVER=d3d12 under WSL
make run-sql     # Parley SQL, the SQL window on its own (CONNECTION=name signs on at once)
make probe HOST=pub400.com KEYS=help,reset   # a real host without the UI: prints each screen (ARGS="--macro FILE" plays a macro)
make package     # installers for this OS (docs/RELEASING.md); make release-patch|minor|major releases
```

Checking against a real IBM i: PUB400.COM is a free public one (sign-on screen without an account). Never sign on
or send credentials to it from Claude's side. Claude may probe the owner's test system only as NODEJT400, password from
`PARLEY_PASSWORD_FILE`, read-only, library NODEJT400 (D22); the host is `$PARLEY_TEST_HOST` (both set in the git-ignored
`.claude/settings.local.json`). `make probe HOST=$PARLEY_TEST_HOST ARGS="--macro FILE"` signs on with a macro whose `password` step reads the file. `PARLEY_TRACE=file` records what a host sends (inbound only).
To see the app, `cargo run -p parley-app -- HOST --screenshot target/app.ppm` saves the window after 6 s.

## Conventions

- Edition 2024, stable toolchain pinned by `rust-toolchain.toml`. Lints are workspace-wide
  (`clippy::pedantic` on, `unsafe_code` denied) — fix warnings; if a lint must be silenced use
  `#[expect(lint, reason = "...")]`. There is no `unsafe` code.
- Errors: a `thiserror` enum in the `error` module of `parley-core` (Parley SQL's in `parley-sql-core`); the UI converts
  to display strings at the edge. No `unwrap()` outside tests.
- Protocol code is tested with byte-exact fixtures: recorded host traffic or hand-built records in tests, never a
  live IBM i in `cargo test`. Cite the spec section (RFC 1205/2877, IBM 5250 Functions Reference) next to the code.
- Tests: unit tests next to the code; whole-session tests in `crates/parley-core/tests`.

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
  The one exception is the 5250 screen's colour schemes (`terminal::Palette`, R-UI-23). Check new UI in light and dark.
- **Dialogs** go through `dialog::dialog` (frame, title, 🗙 close button) and end with Cancel plus a filled confirm
  button on the right (`dialog::actions` or `theme::button_row`). A dialog fixes its width; content that may not
  fit goes in `dialog::fitting_scroll_area`. Settings pages use the settings dialog's `group`/`row` layout.
- **Layout traps** (each cost a bug here): in a `Modal`, nothing may size itself from the dialog's own height or
  width (a vertical `separator()`, a `Frame` in a `Grid` cell, a plain `ScrollArea::max_height`): the dialog then
  grows every frame and clicks miss. Give such content a fixed size, as the settings dialog does. Every dialog has a
  kittest test that its buttons stay in place over a few frames.
- **Glyphs:** egui panics on a character its fonts lack. Use only symbols listed in
  `bundled_fonts_have_every_symbol_the_ui_uses` (`lib.rs`) and add a new one there first.
- **Icons** (D17) come from `icons` and are drawn in their own font family (`icons::icon`, `icons::button`,
  `icons::label`, `theme::icon_badge`), never pasted into text: Inter has other glyphs at the same codepoints. Name an
  icon button for screen readers with `icons::add_named`.

## Project skills

- `/verify` — full local quality gate; run before saying work is done.
- `/audit-context` — recurring audit of these files, skills and settings for token cost and stale references.
