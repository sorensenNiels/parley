# Phase 19 — A fuller History

> Finished phase of Parley SQL. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

From IDEA-35 (the owner, 2026-11-15): the History tab of R-SQL-85 becomes a log of every run, with when it ran, how long
it took and how it ended (R-SQL-89), shown as two-line rows grouped by day with a search and an All / Succeeded / Failed
filter (R-SQL-90); a click selects an entry, and it can be put in the editor, opened in a tab, run again, copied, saved
as a snippet or removed (R-SQL-91). D35.

**Status:** Done, verified (2026-11-15, Linux under WSL): every run kept with when, how long and how it ended (R-SQL-89), listed by day with search and a filter (R-SQL-90), and what can be done with an entry (R-SQL-91); D35.

## Requirements

| ID | Requirement |
|---|---|
| R-SQL-89 | **Every run in the history** (2026-11-15; IDEA-35, D35). Each statement run (from Run, Run all, a snippet, the current library menu or the Explorer) adds an entry, also when it ran before: the statement as run, when it ran, how long it took, and how it ended: the rows read (and whether more were left), the rows changed, done, or the error's first line with its message ID. Never results (R-SQL-7). The last 500 runs per saved connection are kept in Parley SQL's settings, oldest dropped first; the history kept before (statements only) stays, as entries without a time or outcome. *Changes R-SQL-7 (100 statements, a statement run again moved to the end).* |
| R-SQL-90 | **The history list** (2026-11-15; IDEA-35, D35). The History tab lists the entries newest first, under day headings (Today, Yesterday, then the date; entries without a time under "Earlier"). Each entry is two lines: a mark (succeeded or failed) and the statement on one line in the editor's colours, cut at the edge with the whole statement on hover; under it, muted, the time, how long it took and the outcome ("10:42 · 0.31 s · 128 rows, more to fetch"), an error in the error colour. Above the list: a search field (the statement and the error, ignoring case), All / Succeeded / Failed, and Clear history (asks first; for this connection only). Empty, it says so; nothing found, it says that. |
| R-SQL-91 | **Using an entry** (2026-11-15; IDEA-35, D35). A click selects an entry; the arrow keys move the selection. Double-click or Enter adds the statement to the end of the script, as a click did before. The selected entry shows Insert (at the cursor), Run again and Copy; a right-click offers those and Open in a new script tab, Save as snippet… and Remove from history. *Changes R-SQL-85 (a click put the statement in the editor).* |

## Verification

- **R-SQL-89..91** (2026-11-15): a fuller History (Phase 19). **Automated** (`make test`): every run kept with
  its time, how long and how it ended, also a statement run again, the last 500, an error's first line only, the
  history kept before read as entries without a time (core); runs listed newest first under Today and Earlier, the
  search and Failed narrowing the list and "Nothing found.", a click selecting and Enter adding to the script, the
  arrows moving the selection, Insert at the cursor, Clear history asking first, Remove and Clear for one connection
  only (UI). **Checked in the running app** by the owner, Linux under WSL
  (2026-11-15).
