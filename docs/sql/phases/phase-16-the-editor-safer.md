# Phase 16 — The editor, safer

> Finished phase of Parley SQL. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

From IDEA-34's leftovers that fit egui's text field (D28; the owner, 2026-11-15): scripts kept within seconds of a
change (R-SQL-74), the place of an error marked (R-SQL-75), completion of a `WITH`'s names (R-SQL-76), and formatting
choices (R-SQL-77). Folding and several cursors, which need an editor of Parley's own, stay in IDEA-34.

**Status:** Done, verified (2026-11-15, Linux under WSL): scripts kept within seconds (R-SQL-74), an error's place marked (R-SQL-75), completion of a `WITH`'s names (R-SQL-76), formatting choices (R-SQL-77).

## Requirements

| ID | Requirement |
|---|---|
| R-SQL-74 | **Scripts kept within seconds** (2026-11-15; IDEA-34). The scripts, saved to a file or not, are kept with the settings (R-SQL-20) at most 5 seconds after a change, not only when Parley SQL closes and every 30 seconds (eframe's own), so a crash or power cut loses at most the last few seconds of typing. Nothing is written while nothing changed. | 16 |
| R-SQL-75 | **The place of an error marked** (2026-11-15; IDEA-34). When the host's message names what it stopped at, the editor marks it in the statement that ran: the token of SQL0104 ("Token X was not valid"), the name of SQL0206 (column not found) and of SQL0204 (object not found), and the like; found as a word outside strings and comments, from the statement's start (the first place it occurs). The mark is a red underline, the error's text on hover, and a click on the error in Messages (or the error line) puts the cursor there; it goes when the statement is edited or runs again. Otherwise the statement's first line keeps its mark (R-SQL-62). A pure function in `parley_core::sql`, tested on the host's messages as they read, with `æøå` and quoted names. | 16 |
| R-SQL-76 | **Completion of a `WITH`'s names** (2026-11-15; IDEA-34, adds to R-SQL-36). In a statement with common table expressions, completion offers their names with the tables, and after `name.` (or an alias of it) their columns: those listed (`x (a, b) AS (…)`), else the names of the inner select's list (a column, or what `AS` names; an expression without a name is left out). Read from the statement's text only; no catalog query. | 16 |
| R-SQL-77 | **Formatting choices** (2026-11-15; IDEA-34, changes R-SQL-22). Parley SQL's Settings ▸ SQL has a "Formatting" group: keyword case (Upper, the default, lower, or as written), indentation (2 or 4 spaces, the default 4), and where list commas go (at the end of a line, the default, or at the start of the next). Format SQL (R-SQL-22) follows them; Upper case and Lower case (R-SQL-58) stay the explicit choices they are. Restore defaults puts back today's format. | 16 |

## Verification

- **R-SQL-74..77** (2026-11-15): the editor, safer (Phase 16). **Automated** (`make test`): typing reaches the
  kept settings in the same frame and Parley SQL saves every 5 seconds (R-SQL-74); the name an error stopped at read
  from SQL0104, SQL0199, SQL0204..SQL0206 and none from `<END-OF-STATEMENT>` or other messages, its place as a whole
  word outside strings and comments, a delimited name with its quotes, a symbol, `æøå`; the underline in the error
  colour and none once stale, a failed run marking the token, the link selecting it and an edit dropping it (R-SQL-75);
  a `WITH`'s names and columns from a column list, `AS`, a trailing name or the column, numbers and expressions left
  out, an unclosed one, `RECURSIVE`, completion offering its names and columns and an alias found before an inner
  table of the same name (R-SQL-76); Format SQL in each style with the default unchanged, and the SQL page's
  Formatting group saved with Save in place (R-SQL-77). **Checked in the running app** by the owner, Linux under WSL (2026-11-15).
