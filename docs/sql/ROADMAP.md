# Parley SQL roadmap

> The plan for SQL on IBM i in Parley: the SQL window, run as Parley's SQL add-on and as the program `parley-sql`
> (Parley SQL), and the helper, host-server and assistant code under it. Finished phases, with the text of their
> requirements and how each was verified, are in [phases/](phases/); [REQUIREMENTS.md](REQUIREMENTS.md) is the index of finished IDs. The terminal's
> plan is [../ROADMAP.md](../ROADMAP.md).
> Look things up with `grep -n` (an ID such as `"R-SQL-7 "`, `"| D20 "` or a heading) rather than reading the whole file.

**Status of this document:** split from Parley's roadmap on 2026-11-14; cleaned up for the final push on 2026-11-21
(D62). Every phase is done; open: the owner's check of R-SQL-92. Decided not now: [../PARKED.md](../PARKED.md).

## 1. Vision

Parley SQL is SQL for IBM i in Parley's look: write and run statements, read results, know the database and use the
IBM i tools around it, without IBM i Access Client Solutions. It runs inside Parley beside a 5250 session (the SQL
add-on) and on its own as `parley-sql` for people who need SQL but no green screen (D26).

## 2. Principles

Parley's principles apply ([../ROADMAP.md](../ROADMAP.md) §2): keyboard first (R-P-1), correct before clever (R-P-2),
secure by default with no password kept (R-P-3, D16), and one engine in `parley-core` (R-P-4), with the Java helper
as its recorded exception (D20).

## 3. Status

| Phase | State |
|---|---|
| [Phase 5: SQL against the host](phases/phase-05-sql-against-the-host.md) | Done (2026-11-14). |
| [Phase 6: the SQL window, deeper](phases/phase-06-the-sql-window-deeper.md) | Done (2026-11-14). |
| [Phase 7: snippets and column headings](phases/phase-07-snippets-and-column-headings.md) | Done (2026-11-14). |
| [Phase 8: know the database, IBM i tools](phases/phase-08-know-the-database-ibm-i-tools.md) | Done (2026-11-14). |
| [Phase 9: parameters and more from results](phases/phase-09-parameters-and-more-from-results.md) | Done (2026-11-14). |
| [Phase 10: several systems and an SQL assistant](phases/phase-10-several-systems-and-an-sql-assistant.md) | Done (2026-11-14). |
| [Phase 11: Parley SQL](phases/phase-11-parley-sql.md) | Done (2026-11-14). |
| [Phase 12: the assistant's providers](phases/phase-12-the-assistants-providers.md) | Done (2026-11-14). |
| [Phase 13: the editor, handier](phases/phase-13-the-editor-handier.md) | Done (2026-11-14). |
| [Phase 14: the editor, deeper](phases/phase-14-the-editor-deeper.md) | Done (2026-11-14). |
| [Phase 15: data transfer](phases/phase-15-data-transfer.md) | Done (2026-11-15). |
| [Phase 16: the editor, safer](phases/phase-16-the-editor-safer.md) | Done (2026-11-15). |
| [Phase 17: IBM i Services](phases/phase-17-ibm-i-services.md) | Done (2026-11-15). |
| [Phase 18: the work area, rearranged](phases/phase-18-the-work-area-rearranged.md) | Done (2026-11-15). |
| [Phase 19: a fuller History](phases/phase-19-a-fuller-history.md) | Done (2026-11-15). |
| [Phase 25: connection settings](phases/phase-25-connection-settings.md) | Done (2026-11-16). |
| [Phase 23: a library's SQL and backups](phases/phase-23-a-librarys-sql-and-backups.md) | Done (2026-11-16). |

## 4. Phases

Finished phases are in [phases/](phases/), one file each.

Phase numbers continue Parley's (Phases 0–4 are the terminal's); the next free one is on the last line of
[../ROADMAP.md](../ROADMAP.md).

### Later (unordered, to be planned)

Nothing: no SQL ideas are open ([IDEAS.md](IDEAS.md)); decided not now: [../PARKED.md](../PARKED.md).

## 5. Requirements

Open requirements, in the order they are planned, as `| ID | Requirement | Phase |`. The done ones are in [REQUIREMENTS.md](REQUIREMENTS.md); new
ones come from [IDEAS.md](IDEAS.md). Areas: `SQL` the SQL window, its helper and its host-server client (D19); a few
rows from Parley's areas belong here too: `ADD` (the assistant's add-on switch) and `NET` (the SQL probes).

| ID | Requirement | Phase |
|---|---|---|
| R-SQL-92 | **A compound statement is one statement** (2026-11-15; asked by the owner after the code audit. *Changes R-SQL-7:* statements end with `;` outside a compound statement. *Built and tested 2026-11-20; to be checked in the running app, see [FEATURES.md](FEATURES.md).*) A `;` inside `BEGIN … END` does not end the statement, so `CREATE PROCEDURE`, `CREATE FUNCTION`, `CREATE TRIGGER` (`BEGIN ATOMIC`) and a dynamic compound statement run whole, from Run, Run all and a selection. Nested `BEGIN … END`, `END IF`, `END WHILE`, `END LOOP`, `END FOR`, `END REPEAT`, `END CASE` and `CASE … END` expressions are told apart; `BEGIN` and `END` in strings, quoted names and comments count for nothing. A `BEGIN` never closed runs to the end of the text, and the host says what is wrong. A pure function in `parley_sql_core::text`, tested with procedures, triggers, functions, CASE expressions, `æøå` and unclosed text. | Code audit |

## 6. Decisions

The decisions, with the question, the choice and the alternatives, are in [DECISIONS.md](DECISIONS.md).

## 7. How to change this roadmap

- New SQL work gets a requirement in §5 with the next free `R-SQL` ID (the last line); IDs are never reused or renumbered.
- A requirement in another area (`ADD`, `NET`, …), a decision (`D`) or a new phase takes the next free number from the
  last line of [../ROADMAP.md](../ROADMAP.md), which is then moved on, so a number never means two things.
- While a requirement is built, how it is checked goes under "In progress" in [FEATURES.md](FEATURES.md).
- A requirement that is implemented, tested and checked in the running app moves, with its full row and how it
  was verified, to its phase's file in [phases/](phases/) (`phases/between-phases.md` if no phase names it), and
  gets an index row (ID, title, phase) in [REQUIREMENTS.md](REQUIREMENTS.md).
- When a phase is finished, its story (its §4 section and §3 state) moves to a new file
  `phases/phase-NN-<slug>.md`, and its §3 row links to it.
- A choice that future work must respect becomes a decision in [DECISIONS.md](DECISIONS.md).
- An idea is not a plan: it goes in [IDEAS.md](IDEAS.md) (by theme in [ideas/](ideas/)) until it is picked, then becomes requirements here.
- What is decided not to be built now moves with its text to [../PARKED.md](../PARKED.md) (D62).

Next free ID: `R-SQL-105` (other areas, decisions and phases: last line of [../ROADMAP.md](../ROADMAP.md); ideas: last line of [../IDEAS.md](../IDEAS.md)).
