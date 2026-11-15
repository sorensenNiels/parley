# Parley SQL ideas

> Ideas for SQL on IBM i (the SQL window, Parley's SQL add-on and Parley SQL) that might become requirements later.
> Nothing here is planned or promised: the plan is [ROADMAP.md](ROADMAP.md). Look an idea up with `grep -n`
> (`"| IDEA-13 "`) rather than reading the whole file. The terminal's ideas are in [../IDEAS.md](../IDEAS.md).

## How an idea moves on

- A new idea gets the next free `IDEA-n` from the last line of [../IDEAS.md](../IDEAS.md), which is then moved on: one
  sequence for both files. IDs are never reused or renumbered.
- When an idea is picked for a phase, write it as one or more requirements in the roadmap (§5, next free ID of the
  area) and mark the row here *"→ R-XXX-n"*. Remove the row once its requirements are done; the number stays used.
- An idea that would change a decision (either roadmap) says so; the decision is taken before it is built.

## Data on the host

| ID | Idea |
|---|---|
| IDEA-25 | *(Picked 2026-11-15 for Phase 17: → R-SQL-78..81, D32; aimed at the signed-on user, as Parley SQL has no 5250 session.)* IBM i Services views built on IDEA-13: the job log of the session's own job, the spooled files of the user (open one as text or PDF), and the message queue behind the "message waiting" indicator, from the `QSYS2` SQL services. |
| IDEA-34 | **A code editor of Parley's own** (the owner, 2026-11-14, from the ideas after Phase 13): an editor widget instead of egui's text field, for line numbers, a gutter with a ▶ per statement and the last run's rows, time or error, the line of an error from the position Db2 gives, folding and several cursors, and speed on long scripts (the text field lays out all the text every frame). Smaller ones that may come first: a line-number gutter beside the text field (only while lines do not wrap); completion of keywords, of columns through table aliases and of snippets; a procedure's or function's parameters while `CALL X(` is typed (`SYSPARMS`); scripts not saved coming back after a crash; formatting choices (indentation, keyword case, commas first). *Decided (D28, 2026-11-14): stay on the text field with a gutter of Parley's own; picked for Phase 14 (R-SQL-61..68): line numbers, ▶ and outcomes, completion when asked and with snippets, parameters, the word under the cursor, Ctrl+D, trailing blanks.* Left here: folding, several cursors, an error's exact position, completion of a WITH's names, scripts not saved coming back after a crash, formatting choices. *Picked 2026-11-15 for Phase 16: → R-SQL-74..77 (all but folding and several cursors, which stay here).* |

## The work area

| ID | Idea |
|---|---|
| IDEA-35 | *(Picked 2026-11-15 for Phase 19: → R-SQL-89..91, D35.)* **A fuller History** (Claude's suggestion, 2026-11-15, while planning Phase 18): with History as a tab under the editor (R-SQL-85), one row per statement with when it ran, how long it took, and the rows or the error, plus a filter field. The history would then keep more than the statement's text (R-SQL-7). |
| IDEA-36 | **Library groups that hold scripts** (Claude's suggestion, 2026-11-15, while planning Phase 18): a group of R-SQL-83 growing into a project, with saved `.sql` scripts beside its libraries. Would change D33 (groups only organise libraries). |

Next free ID: see the last line of [../IDEAS.md](../IDEAS.md).
