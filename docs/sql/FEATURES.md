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
