# Phase 7 — Snippets and column headings

> Finished phase of Parley SQL. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

The owner's ideas for the SQL add-on (2026-11-13), in this order: a visible Run selection (R-SQL-28), IBM i column
headings in the grid (R-SQL-29), then the snippets library (R-SQL-24..27). The owner also chose, for the phases
after it: the schema browser with completion and the IBM i tools (Phase 8: IDEA-29, IDEA-30), parameters and more
from results (Phase 9: IDEA-31), several systems at once and an SQL assistant (Phase 10: IDEA-32, IDEA-33, the
assistant only after a decision on what may leave the machine). Their requirements are written when each starts.

**Status:** Done, verified (2026-11-14, Linux under WSL): snippets (R-SQL-24..27), a visible Run selection (R-SQL-28), IBM i column headings (R-SQL-29). Then Phase 8 (IDEA-29, IDEA-30), Phase 9 (IDEA-31), Phase 10 (IDEA-32, IDEA-33), chosen by the owner the same day.

## Requirements

| ID | Requirement |
|---|---|
| R-SQL-28 | **A visible Run selection** (2026-11-13). While text is selected in the editor, Run reads "Run selection" and runs it, as it does today (R-SQL-7); otherwise the statement Run would execute, the one under the cursor, is shown with a faint background, so the operator sees what runs before pressing. |
| R-SQL-29 | **IBM i column headings** (2026-11-13). The helper asks JTOpen for extended metadata, so each column of a result also carries its column heading (COLHDG, set with `LABEL ON`, up to three parts of 20 characters) and its text, when the field has them. The grid's header shows the field names, the headings, or both (a choice above the grid, kept in the settings); hovering a heading shows the name, the heading, the text and the type. Copy and Export use what the header shows. Checked first on the owner's IBM i: that JTOpen returns the heading this way. |
| R-SQL-24 | **Snippets** (2026-11-13). A snippet is a `.sql` file, one per snippet, whose first comment lines hold `-- title:`, `-- tags:` and `-- about:`, then the SQL; the text is readable and editable outside Parley. They are kept in a `snippets` folder beside the settings, as macros are (D14), and optionally in a shared folder named in Settings ▸ Add-ons ▸ SQL (a network drive or a git checkout), so a team keeps one library. A name already used, a file that does not read, or a folder that cannot be written is named, not hidden. |
| R-SQL-25 | **The Snippets panel** (2026-11-13). Beside History, a Snippets tab lists the snippets by title with their tags, mine and shared apart; a search field matches title, tags, about and text, ignoring case. Double-click inserts a snippet at the cursor; Run runs it at once; the hover shows its about and SQL. "Save as snippet…" (from the editor, on the selection or the statement under the cursor) asks title, tags, about and mine or shared; mine and shared snippets can be edited, renamed and deleted (asking first). *Changed 2026-11-15 by R-SQL-84 (Snippets is a section under Libraries).* |
| R-SQL-26 | **Placeholders** (2026-11-13). `${name}` or `${name:default}` in a snippet is asked for in a small form when the snippet is inserted or run, one field per name, the last value used in this run of Parley offered first; the values are put in as typed (the operator writes the quotes in the snippet where SQL needs them). |
| R-SQL-27 | **"Did you know" snippets from IBM i Services** (2026-11-13). Parley comes with a starter set of read-only snippets, tagged "did you know", that show what IBM i Services can answer: the busiest active jobs, a job's job log, spooled files of a user, locks on an object, the PTF level and group PTFs, users and their last sign-on, system status, network connections, the history log, the largest objects in a library, table statistics. Each names the IBM i release it needs; each was run on the owner's IBM i (7.3) or marked as needing a later release. Copy to mine to change one. |

## Verification

- **R-SQL-28** (2026-11-13): a visible Run selection, built: "Run selection" while text is selected, otherwise
  the statement Run would execute on a faint background. **Automated:** the background covers exactly that
  statement (`æøå` in it) and nothing when text is selected; drawn: the button's name follows the selection.

- **R-SQL-29** (2026-11-13): IBM i column headings, built. With JTOpen's extended metadata the helper reports each
  column's field name (or alias), file, heading and text; the heading comes from `QSYS2.SYSCOLUMNS` (blanks between
  its parts made one space) or, where the catalog has none, from JTOpen's label. Names | Headings | Both above the
  grid (Headings by default, kept in the settings); the hover gives name, heading, text, file and type; Copy and
  Export use what the header shows. **Real host:** on the owner's IBM i a DDS file with COLHDG (`MSGSUDTA/REGP`) gave
  every column its heading and text; an alias keeps its name and has no heading; catalog views have none.
  **Automated:** the helper's self-test with the owner's real headings (no word split), the modes in Copy and CSV,
  the hover; drawn: the heading by default, Names switching it.

- **R-SQL-24..27** (2026-11-13): snippets, built. `.sql` files with `-- title/tags/about/needs` lines, mine in
  `snippets` beside the settings and an optional shared folder (Settings, Add-ons, SQL); a Snippets tab beside
  History (search, double-click inserts at the cursor, Run, Edit, Delete, hover with the SQL); Save as snippet… from
  the selection or the statement under the cursor; `${name}`/`${name:default}` asked for in a form that offers the
  last values; 13 "did you know" IBM i Services starters, read-only. **Automated:** parsing, writing back, search,
  placeholders and filling (`æøå`) in parley-core; the panel with a store in memory, the save and values dialogs
  keep their buttons in place, a name cannot leave its folder (parley-app). **Checked in the running app** by the owner, Linux under WSL (2026-11-14).
