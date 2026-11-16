# Phase 9 — Parameters and more from results

> Finished phase of Parley SQL. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

From IDEA-31 (2026-11-14), in this order, the small ones first: copy a result as Markdown, `INSERT` statements or
JSON (R-SQL-38); binary values as hex and EBCDIC (R-SQL-39); a form for parameter markers (R-SQL-40, D23); compare
two results (R-SQL-41); quick charts (R-SQL-42). The owner's choices (D23): parameters as a prepared statement's
values, charts as bars and lines with `egui_plot`, rows compared by key columns.

**Status:** Done, verified (2026-11-14, Linux under WSL): copy as Markdown, `INSERT` or JSON (R-SQL-38), hex and EBCDIC (R-SQL-39), a parameter form (R-SQL-40), compare results (R-SQL-41), charts (R-SQL-42); D23.

## Requirements

| ID | Requirement |
|---|---|
| R-SQL-38 | **Copy as Markdown, INSERT or JSON** (2026-11-14). Beside Copy, a menu copies the rows shown (or the selected rows) as a Markdown table, as `INSERT INTO lib.table (…) VALUES (…)` statements (the table named from the query when it reads one table, else asked), or as a JSON array of objects keyed by column name. Nulls stay null (`NULL`, `null`, empty in Markdown); numbers are not quoted; text is escaped for each format (`\|` in Markdown, `''` in SQL, JSON escapes). |
| R-SQL-39 | **Binary values as hex and EBCDIC** (2026-11-14). A `FOR BIT DATA` or `BINARY` value (sent as hex) shows in the row detail as hex bytes in groups beside the same bytes read as EBCDIC in a chosen code page (the session's by default), unprintable bytes as `.`. |
| R-SQL-40 | **A form for parameter markers** (2026-11-14). Running a statement with `?` or `:name` markers (outside strings and comments) opens a form with a field per marker (`:name` once per name); Run sends the statement prepared with the values as its parameters (D23), an empty field as null when ticked. The values are remembered per statement text for the session's window. |
| R-SQL-41 | **Compare two results** (2026-11-14). From a result tab, "Compare with…" picks another tab with the same column names; rows are matched by key columns (the first column unless others are chosen); a result tab then lists rows only in the first, only in the second, and rows whose other values differ, with the differing cells marked. |
| R-SQL-42 | **Quick charts** (2026-11-14). A Chart view of a result: the first text or date column along the bottom, the numeric columns chosen as bars or lines, from the rows fetched; a legend; light and dark. Drawn with `egui_plot` (D23). |

## Verification

- **R-SQL-38, R-SQL-39** (2026-11-14): Copy as and binary values, built. Beside Copy, "Copy as" copies the selected
  rows (or all shown) as a Markdown table, `INSERT` statements (into the table the columns come from, else
  `TABLE_NAME`) or JSON; numbers unquoted, nulls kept, text escaped per format. In the row detail a `FOR BIT DATA`
  or `BINARY` value shows as hex in groups of four beside its EBCDIC text, in the session's code page or another
  chosen. **Automated:** each format's escaping, numbers and nulls (`æøå`, `O'Brien`, `|`); the dump's lines and
  unprintable bytes; Copy as takes the selection. **Checked:** JTOpen's type names for both on the owner's 7.3
  (`CHAR () FOR BIT DATA`, `BINARY`), `'Æble'` arriving as `7B829385`. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-40** (2026-11-14): a form for parameter markers, built. A statement with `?` or `:name` markers (not in
  strings, names or comments, never in `CL:`) opens a form first, a field per `?` and per name, each with a NULL
  switch; Run sends it prepared with the values (helper protocol 2, D23), the history keeps the markers, and the
  values come back next time for the same statement in that window; Cancel stops what was queued. **Automated:**
  markers and their slots (`:name` twice, `æøå`, quotes, comments, `12:30`), the request's `params`, the form's
  buttons in place and its values, the window waiting for it. **Checked:** `sqlprobe --tools` on the owner's 7.3
  as NODEJT400: text, a number, a name used twice and null came back right. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-41, R-SQL-42** (2026-11-14): compare and charts, built. "Compare with…" over the result tabs ticks the key
  columns (the first by default) and lists the other results with the same column names; the comparison opens as
  a result of its own: rows only in the first, only in the second, and pairs that differ with the differing values
  marked; how many were the same is said. "Table | Chart" shows the rows shown as bars or lines (`egui_plot` 0.38,
  D23): the first text or date column along the bottom, up to four numeric columns at first, others under
  Columns. **Automated:** matching by key in any row and column order, several keys, duplicates, mismatched
  columns or keys (`æøå`); the comparison tab and its marks; chart data (labels, gaps, row numbers) and a kittest
  switching Table, Chart, Lines. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).
