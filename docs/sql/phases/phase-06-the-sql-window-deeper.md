# Phase 6 — The SQL window, deeper

> Finished phase of Parley SQL. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

The owner chose two areas of the SQL add-on (2026-11-13), built in this order:
1. **More from results:** a tab per query (R-SQL-14), a row's values one under another (R-SQL-15), filtering and
   resizing in the grid (R-SQL-16), the rows fetched at a time and a query timeout as settings (R-SQL-18).
   *R-SQL-17, export to Excel, was dropped (the owner, 2026-11-13): CSV only, no new dependency.*
2. **Editor and scripts:** SQL in colour (R-SQL-19), several scripts in tabs (R-SQL-20), open and save `.sql`
   files (R-SQL-21), formatting (R-SQL-22), find and replace (R-SQL-23).

Not here: the schema browser and completion, the IBM i tools (library list, CL, job log) and the stand-alone
client (IDEA-28), for later phases.

**Status:** Done, verified (2026-11-14, Linux under WSL): more from results (R-SQL-14..16, R-SQL-18; R-SQL-17 dropped) and editor and scripts (R-SQL-19..23).

## Requirements

| ID | Requirement |
|---|---|
| R-SQL-14 | **A result tab per query** (2026-11-13). Each query run gets its own tab above the grid, named by its first words and row count ("SELECT * FROM ORDERS · 1000"); the newest is shown. Run all opens a tab per query it runs. Up to 10 tabs are kept, the oldest closing first; a tab closes with its ×, and each keeps its sort, filter, selection and Fetch more. |
| R-SQL-15 | **A row's values one under another** (2026-11-13). Double-clicking a row, or Enter on a selected one, shows its columns in a panel beside the grid: name, type, and the whole value wrapped (NULL apart from empty), each with a Copy button; the arrows step to the row above or below. |
| R-SQL-16 | **Filter and resize the grid** (2026-11-13). A filter field above the grid keeps the fetched rows that hold its text in any column, ignoring case, and says how many of how many are shown; Copy and Export take the rows shown. Dragging a column heading's right edge changes its width; double-clicking the edge fits it to the values shown. |
| R-SQL-18 | **Rows at a time and a query timeout** (2026-11-13). Settings ▸ Add-ons ▸ SQL: the rows fetched at first and by each Fetch more (default 1000, 100 to 100 000), and a query timeout in seconds (default none), after which the helper cancels the statement and says so. |
| R-SQL-19 | **SQL in colour** (2026-11-13). The editor shows keywords, strings, delimited names, numbers and comments in colours from the theme, in light and dark, as the user types, using the same scanner as statement splitting (`sql::text`); a large script stays responsive. |
| R-SQL-20 | **Several scripts in tabs** (2026-11-13). The editor holds several scripts, each in a tab named by its file or "Script n"; New tab (Ctrl+T), close (Ctrl+W, asking when a file's changes are not saved), Ctrl+Tab between them. Run works in the tab shown. The tabs, saved or not, come back for the same saved session the next time (kept with the settings, as the history; the owner, 2026-11-13). |
| R-SQL-21 | **Open and save SQL files** (2026-11-13). Open… (Ctrl+O), Save (Ctrl+S) and Save as… through the system's file window, UTF-8 text with the `.sql` extension; the tab shows the file's name and a dot while changes are not saved; opening a file that is open already shows its tab. |
| R-SQL-22 | **Format SQL** (2026-11-13). Ctrl+Shift+F formats the selection, or the statement under the cursor: keywords in capitals, each main clause (SELECT, FROM, WHERE, GROUP BY, ORDER BY, …) on its own line, the lists under it indented, strings, names and comments left exactly as written. A pure function in `parley_core::sql::text`, tested on statements with `æøå`, comments and nested selects. |
| R-SQL-23 | **Find and replace** (2026-11-13). Ctrl+F opens a find bar over the editor (next, previous, match case), Ctrl+H adds replace and replace all; Esc closes it. |

## Verification

- **R-SQL-14..16, R-SQL-18** (2026-11-13): more from results, built (`sql/results`, `sql/grid`). A tab per query
  (up to 10, the oldest closing first, each with its sort, filter and Fetch more); a row's values one under another
  beside the grid (double-click, or Enter on one selected row; Next and Previous); a filter over the rows fetched
  ("n of m rows shown", Copy and Export take the rows shown); column widths dragged at a heading's edge, fitted with
  a double-click; rows at a time and a query timeout in Settings, the timeout as JTOpen's `cancel` mechanism.
  **Automated:** tabs (newest shown, ten kept, fetch into the right tab, closing), filter with `æøå` and a hidden
  selection, the detail stepping and stopping at the ends, widths fitted within limits, limits in each request; drawn:
  a row's number selects it and Enter shows "Row 1 of 2". Two bugs found by these tests: the row's double-click area
  covered its number (no selection), and a focused number took Enter as a click. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-19..23** (2026-11-13): the editor, built (`sql/editor`, `sql::text::highlight` and `format`). Keywords,
  strings, names, numbers and comments in Tortuga's code colours (light and dark); scripts in tabs kept per saved
  session, with a dot when changed and a question before closing one; Open…, Save, Save as… through the file window
  (UTF-8 `.sql`); Format (Ctrl+Shift+F) on the selection or the statement under the cursor; find (Ctrl+F) and
  replace (Ctrl+H), Esc closing the bar before it cancels a statement. **Automated:** highlighting (strings,
  comments and names are never keywords), formatting (clauses, AND/OR, joins, nested selects, kept strings and
  comments, formatting twice changes nothing), find ignoring case for `ÆØÅ`, replace all, scripts kept and restored
  with whether they changed, closing asks only when changed; drawn: Ctrl+F and Esc, Ctrl+T. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).
