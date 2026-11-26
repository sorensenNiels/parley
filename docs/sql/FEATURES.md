# Parley SQL features

> SQL features (the SQL window, Parley's SQL add-on and Parley SQL) that are implemented **and verified**, by
> requirement ID. The requirements they were built to are in [REQUIREMENTS.md](REQUIREMENTS.md); open work and new
> requirements are in [ROADMAP.md](ROADMAP.md). Look an ID up with `grep -n` rather than reading the whole file.
>
> Add an entry only when the requirement is fully met, has automated tests where practical, and has
> been checked in the running app on at least one OS. Note which OSes were checked. Then move the
> requirement's row from the roadmap, and this entry, to its phase's file in [phases/](phases/), with a row in
> [REQUIREMENTS.md](REQUIREMENTS.md).

**Verified on:** Linux under WSL (the owner, 2026-11-14), and the owner's IBM i 7.3 as NODEJT400 (D22). Windows and macOS not yet.

## In progress

- **R-SQL-105** (Phase 43 built 2026-11-22, Linux under WSL, tests only; D64): Parley SQL's start screen on
  Parley's shared one (`app/start.rs`); `SqlConnection::favourite`, `Settings::recent_connections` and
  `all_connections_open`. **Automated** (`make test`): recent connections newest first, no repeats, at most five, one
  not saved kept by its host; found again by name, or by host and TLS; kittest tests for the star, Favourites and All
  connections open with nothing above it, then folding and kept; Recent with "Not saved" and Save as connection…; quick connect refusing a port; no
  groups. **Open:** seen in the running app, and the owner's check.

- **R-SQL-106..109** (Phase 44, built 2026-11-25; Linux under WSL, tests only). The Snippets header's `+` opens
  "New snippet" with the selection or empty, and the panel says how to make one until one of one's own exists; the
  editor's right-click menu and Ctrl+Shift+S (a Shortcuts action, `save-snippet`) open "Save as snippet"; the dialog
  lists what the SQL asks for as it is typed; the values form shows the filled SQL and offers Insert and Run. Tests in
  `parley-sql-ui` (`*_r_sql_106`, `*_r_sql_107`, `*_r_sql_108`, `*_r_sql_109`). **Open:** seen in the running app.
- **R-SQL-110, R-SQL-111** (Phase 44, built 2026-11-25; Linux under WSL, tests only). `${name as text|number|date|name}`
  checks and writes values (`snippet::placeholder`), the form names a refused value and waits, the dialog names unknown
  types and text quoted twice; the dialog lists the SQL's literals with "Ask for it" (`snippet::literals`). Tests in
  `parley-sql-core` (`snippet::placeholder::tests::*_r_sql_110`, `snippet::literals::tests::*_r_sql_111`) and
  `parley-sql-ui` (`*_r_sql_110`, `*_r_sql_111`). **Open:** seen in the running app.
- **R-SQL-112** (Phase 44, built 2026-11-25; Linux under WSL, tests only). `snippet::fill` doubles the `'` of an untyped
  value inside a `'…'` string (`'${u}'`, `LIKE '%${t}%'`), found on the SQL with its placeholders blanked so a `'` in a
  default does not move the strings. Tests `*_r_sql_112` in `parley-sql-core`. **Open:** seen in the running app.
- **R-SQL-113..115** (Phase 44, built 2026-11-25; Linux under WSL, tests only). Recent values: `SnippetPanel::remember`
  (five, newest first, not empty or password-like names), kept in `sql_snippet_values` by connection through
  `hosting::keep`/`rename`, offered by a ▾ menu beside the field. `as list` writes `'a', 'b'`. `as library`/`as table`
  are names; the form's "Names" list comes from `Browser::library_names`/`table_names` through
  `SqlWindow::offer_placeholder_names`. Tests `*_r_sql_113`, `*_r_sql_114`, `*_r_sql_115` in both crates. **Open:**
  seen in the running app, with a table's names read from the owner's IBM i.
- **R-SQL-116..119** (Phase 44, built 2026-11-25; Linux under WSL, tests only). Run snippet (`snippets/finder.rs`,
  Ctrl+Shift+R, `run-snippet`; the window's keys wait while it is open); a snippet's result titled by it with "Run
  again…" (`snippets::Report`, `SqlWindow::snippet_run`, `Results::add_from`); a snippet row dragged onto the editor
  (`snippets::Dragged`, `FieldOut::dropped`); a title and tags suggested (`snippet::suggest`). Tests `*_r_sql_116` ..
  `*_r_sql_119` in both crates; the drag is a kittest drag from the row to the text. **Open:** seen in the running
  app, the drag with a real mouse.
- **R-SQL-120** (Phase 44, built 2026-11-25; Linux under WSL, tests only). `sql_placeholders` (on at first, Settings ▸
  SQL "Ask for ${…} values") reaches the window as `Around::placeholders`; Run and Run all ask through
  `SnippetPanel::ask_for_editor` (the values form with Run and Run as written), then run the filled SQL as Run all does.
  Tests `*_r_sql_120` in `parley-sql-ui`. **Open:** seen in the running app; a statement's gutter mark after a filled
  run (the marks are matched by text, which the values changed).
- **R-SQL-121..127** (Phase 44, built 2026-11-25; Linux under WSL, tests only; D66). Settings ▸ SQL's new groups
  "Running and history" and "Results and CSV". History kept and its size, and values remembered, in `hosting::keep`
  (`history::add` takes the limit); the rest reaches the window as `Choices` through `Around` (`SqlWindow::apply`):
  `confirm_risky` in `next_statement`, `show_outcome` in `answers.rs`, the NULL text in the grid and a row's values,
  `CsvOptions` (byte-order mark, header, NULL) in Export and Download, `max_objects` in `Database` and the Explorer's
  "The first N only.". Tests `*_r_sql_121` .. `*_r_sql_127` in both crates. **Open:** seen in the running app; a
  library of more than 5000 objects read whole on the owner's IBM i.

- **R-SQL-92** (built 2026-11-15 in the code audit, finished 2026-11-20; Linux under WSL, tests only). `text::segments`
  skips the `;`s inside `BEGIN … END`, tells `END IF`/`WHILE`/`LOOP`/`FOR`/`REPEAT`/`CASE` and `CASE … END`
  expressions apart, and ignores `BEGIN`/`END` in strings, quoted names and comments; an unclosed `BEGIN` runs to the
  end. Tests in `parley-sql-core` (`text::tests::*_r_sql_92`: a procedure with nested blocks, `END IF`, `END CASE`,
  `END WHILE` and comments; a trigger with `BEGIN ATOMIC`; a labelled function; CASE expressions; `æøå`; unclosed
  text; the statement under the cursor) and in `parley-sql-ui` (`run_all_and_run_send_a_procedure_whole_r_sql_92`:
  Run all queues the procedure whole, Run in its body picks the whole procedure). *Still to check in the running app:*
  Run, Run all and Run selection on a `CREATE PROCEDURE` with a compound body, and a dynamic compound statement
  (`BEGIN DECLARE X INT; SET X = 1; END`) on an IBM i; the ▶ marks in the gutter show one statement.

## Verified

How each finished requirement was verified is in its phase file:

- [Phase 5 — SQL against the host](phases/phase-05-sql-against-the-host.md)
- [Phase 6 — The SQL window, deeper](phases/phase-06-the-sql-window-deeper.md)
- [Phase 7 — Snippets and column headings](phases/phase-07-snippets-and-column-headings.md)
- [Phase 8 — Know the database, IBM i tools](phases/phase-08-know-the-database-ibm-i-tools.md)
- [Phase 9 — Parameters and more from results](phases/phase-09-parameters-and-more-from-results.md)
- [Phase 10 — Several systems and an SQL assistant](phases/phase-10-several-systems-and-an-sql-assistant.md)
- [Phase 11 — Parley SQL](phases/phase-11-parley-sql.md)
- [Phase 12 — The assistant's providers](phases/phase-12-the-assistants-providers.md)
- [Phase 13 — The editor, handier](phases/phase-13-the-editor-handier.md)
- [Phase 14 — The editor, deeper](phases/phase-14-the-editor-deeper.md)
- [Phase 15 — Data transfer](phases/phase-15-data-transfer.md)
- [Phase 16 — The editor, safer](phases/phase-16-the-editor-safer.md)
- [Phase 17 — IBM i Services](phases/phase-17-ibm-i-services.md)
- [Phase 18 — The work area, rearranged](phases/phase-18-the-work-area-rearranged.md)
- [Phase 19 — A fuller History](phases/phase-19-a-fuller-history.md)
- [Phase 23 — A library's SQL and backups](phases/phase-23-a-librarys-sql-and-backups.md)
- [Phase 25 — Connection settings](phases/phase-25-connection-settings.md)
- [Between phases](phases/between-phases.md)
