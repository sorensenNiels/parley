# Phase 23 — A library's SQL and backups

> Finished phase of Parley SQL. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

Asked by the owner (2026-11-15): on a library in the Explorer, its SQL as one script (R-SQL-93) and a backup as one SQL
file with every row as an `INSERT` statement (R-SQL-96); on a table, its SQL with its indexes and views (R-SQL-94);
Download… writes `INSERT` statements too (R-SQL-95). D39.

**Status:** Done (2026-11-16), checked by the owner on Windows, macOS, Linux and WSL: a library's SQL (R-SQL-93), a table with its indexes and views (R-SQL-94), rows as `INSERT` statements (R-SQL-95), a library backed up as SQL (R-SQL-96); D39.

## Requirements

| ID | Requirement |
|---|---|
| R-SQL-93 | **A library's SQL** (2026-11-15; the owner, D39). A library's right-click menu in the Explorer has "Generate SQL…": a dialog names the library and has a switch per kind of object, all on: tables (with their constraints and triggers), views, indexes, procedures and functions. Generate reads, on the catalog job (D21), which kinds the library has, then `QSYS2.GENERATE_SQL` with the name `'%'` for each kind chosen that it has, and opens one script in a new tab: a header comment with the library and when, then each part under a `-- heading`, in an order that can be run: tables (without their constraints and triggers), constraints, indexes, functions, views, procedures, triggers. A part the host could not generate is a one-line comment saying why, and the rest is kept. A library with none of the kinds chosen says so in Messages. |
| R-SQL-94 | **A table with its indexes and views** (2026-11-15; the owner, D39). A table's right-click menu has "Show SQL with indexes and views" besides "Show SQL" (R-SQL-35): one new script tab with the table's SQL, then that of each index over it (`QSYS2.SYSINDEXES`) and each view built directly on it (`QSYS2.SYSVIEWDEP`), by system name. One that cannot be generated is a comment saying why. |
| R-SQL-95 | **Rows as INSERT statements** (2026-11-15; the owner, D39). The Download dialog (R-SQL-71) has a third format, "SQL INSERT": a UTF-8 `.sql` file with one `INSERT INTO library.table (columns) VALUES (…);` a row, written as the rows come; values as Copy as INSERT writes them (R-SQL-38). From a result tab, into the table its columns come from (else `TABLE_NAME`); from a table or view in the Explorer, into that table, after a comment naming it and when, and with `OVERRIDING SYSTEM VALUE` when the table has a column `GENERATED ALWAYS` (identity, row change timestamp, `ROWID`), so the rows go back as they were. The column headings switch is not shown for it. |
| R-SQL-96 | **A library backed up as SQL** (2026-11-15; the owner, D39). A library's right-click menu has "Back up as SQL…": the dialog of R-SQL-93 without the tables' switch (a backup always has them), then the file window for `LIBRARY.sql`. The file holds a header saying how to restore, the tables' SQL, then every table's rows (`T` and `P`: not views, aliases or materialized query tables) as R-SQL-95 writes them, each after a comment naming it, then the other parts chosen in R-SQL-93's order, so constraints and triggers do not stop the rows. The rows are read on the SQL job one table after the other, as a download is, with its progress and Cancel; Cancel or an error removes the file. When done, Messages says how many tables and rows were written and where. |

## Verification

*From the section "Checked 2026-11-16 — R-SQL-87, R-SQL-88, R-SQL-93..96, Phase 25 (R-SQL-97..100), R-SQL-101"; its heading and notes are in [Between phases](between-phases.md).*

- **R-SQL-93..96** (2026-11-15): a library's SQL, a table with its indexes and views, rows as `INSERT` statements, a
  library backed up as SQL (Phase 23, D39). Built; **automated** (`make test`): the `GENERATE_SQL` calls and catalog
  queries, the order of a library's parts, failures kept as comments, a table's indexes and views, the backup's file
  (its SQL, each table's rows with `OVERRIDING SYSTEM VALUE` where needed, the rest), Cancel and a busy connection,
  and the dialogs keeping their buttons in place. Not yet checked on the owner's IBM i (in particular `'%'` for
  `CONSTRAINT` and `TRIGGER`, and `QSYS2.SYSVIEWDEP` on 7.3) or in the running app. First try by the owner (7.3): SQL0104 at
  `EXISTS` in the contents query, rewritten without `EXISTS`; a download now clears an earlier error line. **Checked in the running app** by the owner on Windows, macOS, Linux and Linux under WSL (2026-11-16).
