# Parley SQL roadmap

> The plan for SQL on IBM i in Parley: the SQL window, run as Parley's SQL add-on and as the program `parley-sql`
> (Parley SQL), and the helper, host-server and assistant code under it. Finished phases, with the text of their
> requirements and how each was verified, are in [phases/](phases/); [REQUIREMENTS.md](REQUIREMENTS.md) is the index of finished IDs. The terminal's
> plan is [../ROADMAP.md](../ROADMAP.md).
> Look things up with `grep -n` (an ID such as `"R-SQL-7 "`, `"| D20 "` or a heading) rather than reading the whole file.

**Status of this document:** split from Parley's roadmap on 2026-11-14, with Phases 5–12, every `R-SQL` requirement,
the SQL probes (R-NET-8, R-NET-9), the assistant's add-on switch (R-ADD-3) and decisions D19–D27, moved as written.

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
| [Phase 5: SQL against the host](phases/phase-05-sql-against-the-host.md) | Done, verified (2026-11-14, Linux under WSL): the SQL window over a Java helper with IBM's JTOpen (D20): R-SQL-3..13. Built first: R-NET-8, R-SQL-1 and R-SQL-2 (the native sign-on, kept for the probe), verified on the owner's IBM i. |
| [Phase 6: the SQL window, deeper](phases/phase-06-the-sql-window-deeper.md) | Done, verified (2026-11-14, Linux under WSL): more from results (R-SQL-14..16, R-SQL-18; R-SQL-17 dropped) and editor and scripts (R-SQL-19..23). |
| [Phase 7: snippets and column headings](phases/phase-07-snippets-and-column-headings.md) | Done, verified (2026-11-14, Linux under WSL): snippets (R-SQL-24..27), a visible Run selection (R-SQL-28), IBM i column headings (R-SQL-29). Then Phase 8 (IDEA-29, IDEA-30), Phase 9 (IDEA-31), Phase 10 (IDEA-32, IDEA-33), chosen by the owner the same day. |
| [Phase 8: know the database, IBM i tools](phases/phase-08-know-the-database-ibm-i-tools.md) | Done, verified (2026-11-14, Linux under WSL): checked on the owner's 7.3 as NODEJT400 (D22): CL commands (R-SQL-30), message help and the job log (R-SQL-31), library list and current schema (R-SQL-32), a second connection (R-SQL-33, D21), the schema browser (R-SQL-34), DDL (R-SQL-35), completion (R-SQL-36), names on hover (R-SQL-37). |
| [Phase 9: parameters and more from results](phases/phase-09-parameters-and-more-from-results.md) | Done, verified (2026-11-14, Linux under WSL): copy as Markdown, `INSERT` or JSON (R-SQL-38), hex and EBCDIC (R-SQL-39), a parameter form (R-SQL-40), compare results (R-SQL-41), charts (R-SQL-42); D23. |
| [Phase 10: several systems and an SQL assistant](phases/phase-10-several-systems-and-an-sql-assistant.md) | Done, verified (2026-11-14, Linux under WSL): one statement on several systems (R-SQL-43, D24), an SQL assistant from plain words (R-SQL-44, D25), as an add-on off by default (R-ADD-3). |
| [Phase 11: Parley SQL](phases/phase-11-parley-sql.md) | Done, verified (2026-11-14, Linux under WSL): the SQL window as a program of its own, `parley-sql` (R-SQL-45), with saved connections (R-SQL-46) and its own settings pages (R-SQL-47); D26. Parley's SQL add-on is unchanged. |
| [Phase 12: the assistant's providers](phases/phase-12-the-assistants-providers.md) | Done, verified (2026-11-14, Linux under WSL): Settings ▸ SQL assistant in both programs, with the switch, the provider (Anthropic, OpenAI, Google Gemini, Mistral, llama.cpp on a local `llama-server`, or any OpenAI-compatible server), the model and address, and the API key in the keychain (R-SQL-48); D27. |
| [Phase 13: the editor, handier](phases/phase-13-the-editor-handier.md) | Done, verified (2026-11-14, Linux under WSL): asked by the owner: comment lines (R-SQL-52), indent and outdent (R-SQL-53), move and duplicate lines (R-SQL-54), brackets and quotes that close themselves (R-SQL-55), the matching bracket (R-SQL-56), go to a line (R-SQL-57), keyword case (R-SQL-58), zoom the editor (R-SQL-59); then Parley SQL's Shortcuts page (R-SQL-60). |
| [Phase 14: the editor, deeper](phases/phase-14-the-editor-deeper.md) | Done, verified (2026-11-14, Linux under WSL): from IDEA-34 (the owner): on egui's text field with a gutter of Parley's own (D28): line numbers (R-SQL-61), a ▶ and the last outcome per statement (R-SQL-62), completion on demand and with snippets (R-SQL-63, R-SQL-64), a routine's parameters while it is called (R-SQL-65), the word under the cursor marked (R-SQL-66), Ctrl+D (R-SQL-67), trailing blanks trimmed on save (R-SQL-68). |
| [Phase 15: data transfer](phases/phase-15-data-transfer.md) | Done, verified (2026-11-15, Linux under WSL): every row to CSV or Excel (R-SQL-71, R-SQL-72), a CSV file into a table, all or nothing (R-SQL-73); D31. |
| [Phase 16: the editor, safer](phases/phase-16-the-editor-safer.md) | Done, verified (2026-11-15, Linux under WSL): scripts kept within seconds (R-SQL-74), an error's place marked (R-SQL-75), completion of a `WITH`'s names (R-SQL-76), formatting choices (R-SQL-77). |
| [Phase 17: IBM i Services](phases/phase-17-ibm-i-services.md) | Done, verified (2026-11-15, Linux under WSL; the probe on the owner's IBM i 7.3): a System node under the IBM i in the side panel's tree (D32): spooled files (R-SQL-78), the message queue (R-SQL-79), jobs and job logs (R-SQL-80); more starters (R-SQL-81). |
| [Phase 18: the work area, rearranged](phases/phase-18-the-work-area-rearranged.md) | Done, verified (2026-11-15, Linux under WSL), built before Phase 17 at the owner's wish: the Libraries panel on the left (R-SQL-82), library groups (R-SQL-83), Snippets under Libraries (R-SQL-84), History beside Results and Messages (R-SQL-85); D33. |
| [Phase 19: a fuller History](phases/phase-19-a-fuller-history.md) | Done, verified (2026-11-15, Linux under WSL): every run kept with when, how long and how it ended (R-SQL-89), listed by day with search and a filter (R-SQL-90), and what can be done with an entry (R-SQL-91); D35. |
| [Phase 25: connection settings](phases/phase-25-connection-settings.md) | Done (2026-11-16), checked by the owner on Windows, macOS, Linux and WSL: naming, libraries, commitment control, formats and the CSV delimiter (R-SQL-97..100), chosen by the owner from a review of fixed values. |
| [Phase 23: a library's SQL and backups](phases/phase-23-a-librarys-sql-and-backups.md) | Done (2026-11-16), checked by the owner on Windows, macOS, Linux and WSL: a library's SQL (R-SQL-93), a table with its indexes and views (R-SQL-94), rows as `INSERT` statements (R-SQL-95), a library backed up as SQL (R-SQL-96); D39. |

## 4. Phases

Finished phases are in [phases/](phases/), one file each.

Phase numbers continue Parley's (Phases 0–4 are the terminal's); the next free one is on the last line of
[../ROADMAP.md](../ROADMAP.md).

### Later (unordered, to be planned)

Nothing yet; see [IDEAS.md](IDEAS.md).

## 5. Requirements

Open requirements, in the order they are planned, as `| ID | Requirement | Phase |`. The done ones are in [REQUIREMENTS.md](REQUIREMENTS.md); new
ones come from [IDEAS.md](IDEAS.md). Areas: `SQL` the SQL window, its helper and its host-server client (D19); a few
rows from Parley's areas belong here too: `ADD` (the assistant's add-on switch) and `NET` (the SQL probes).

| ID | Requirement | Phase |
|---|---|---|
| R-SQL-92 | **A compound statement is one statement** (2026-11-15; asked by the owner after the code audit. *Changes R-SQL-7:* statements end with `;` outside a compound statement.) A `;` inside `BEGIN … END` does not end the statement, so `CREATE PROCEDURE`, `CREATE FUNCTION`, `CREATE TRIGGER` (`BEGIN ATOMIC`) and a dynamic compound statement run whole, from Run, Run all and a selection. Nested `BEGIN … END`, `END IF`, `END WHILE`, `END LOOP`, `END FOR`, `END REPEAT`, `END CASE` and `CASE … END` expressions are told apart; `BEGIN` and `END` in strings, quoted names and comments count for nothing. A `BEGIN` never closed runs to the end of the text, and the host says what is wrong. A pure function in `parley_sql_core::text`, tested with procedures, triggers, functions, CASE expressions, `æøå` and unclosed text. | Code audit |

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

Next free ID: `R-SQL-105` (other areas, decisions and phases: last line of [../ROADMAP.md](../ROADMAP.md); ideas: last line of [../IDEAS.md](../IDEAS.md)).
