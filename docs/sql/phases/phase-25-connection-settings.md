# Phase 25 — Connection settings

> Finished phase of Parley SQL. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

**Status:** Done (2026-11-16), checked by the owner on Windows, macOS, Linux and WSL: naming, libraries, commitment control, formats and the CSV delimiter (R-SQL-97..100), chosen by the owner from a review of fixed values.

## Requirements

| ID | Requirement |
|---|---|
| R-SQL-97 | **Naming per connection** (2026-11-16, the owner chose it from a review of fixed values). The connection dialog's "Naming": SQL (`SCHEMA.TABLE`, default, as today) or System (`LIBRARY/FILE` and the library list), passed to JTOpen as `naming` (`sql` or `system`). |
| R-SQL-98 | **Libraries at connect** (2026-11-16, from the same review). The connection dialog's "Libraries": a list of libraries, separated by commas or blanks, passed to JTOpen as `libraries`; with SQL naming the first is the default schema, with System naming they make the library list. Empty: the user profile's defaults, as today. |
| R-SQL-99 | **Commitment control per connection** (2026-11-16, from the same review). The connection dialog's "Auto-commit" (on by default, as today) sets how the SQL window's Autocommit switch starts, and "Isolation": the driver's default, *NONE, *CHG, *CS, *ALL or *RR, passed to JTOpen as `transaction isolation` (none, read uncommitted, read committed, repeatable read, serializable). |
| R-SQL-100 | **Formats and the CSV delimiter** (2026-11-16, from the same review). The connection dialog's "Date format" (ISO by default, as today; also USA, EUR, JIS, MDY, DMY, YMD, Julian), "Time format" (ISO; also USA, EUR, JIS, HMS) and "Decimal separator" (period or comma), passed to JTOpen as `date format`, `time format` and `decimal separator`; and Settings ▸ SQL ▸ "CSV delimiter": comma (default) or semicolon, for downloads and Copy as CSV, as Excel in Danish or Norwegian expects. |

## Verification

*From the section "Checked 2026-11-16 — R-SQL-87, R-SQL-88, R-SQL-93..96, Phase 25 (R-SQL-97..100), R-SQL-101"; its heading and notes are in [Between phases](between-phases.md).*

- **Phase 25: connection settings** (2026-11-16), built; `make verify` on Linux under WSL. **Automated** (`make test`):
  the defaults are the JTOpen properties Parley SQL always sent (naming sql, ISO dates and times, a period); chosen
  options become `naming`, `libraries` (upper case, commas or blanks, `æøå`), `transaction isolation` (each level by
  its JDBC name), `date format`, `time format` and `decimal separator`, sent in the connect line with how
  autocommit starts; old saved connections read with the defaults; the connection dialog saves naming, libraries and
  auto-commit, and keeps its place; a window starts Autocommit as its connection says; the helper puts the
  properties over its defaults (`make helper`). The CSV delimiter: a semicolon separates and quotes by the
  semicolon, in downloads and Export, chosen on the SQL page. **Checked in the running app** by the owner on Windows, macOS, Linux and Linux under WSL (2026-11-16).
