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
| Phase 43: a start screen for many sessions | Planned 2026-11-22 with Parley 5250's (D64, [../ROADMAP.md](../ROADMAP.md)): R-SQL-105. |
| Phase 44: snippets, easier to make and use | Planned 2026-11-25 (IDEA-61..65, 67..75), and settings the owner asked for: R-SQL-106..127. |

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
| R-SQL-105 | **A start screen for many connections** (2026-11-22, asked by the owner; D64; as R-UI-40..42 for Parley 5250's sessions). Parley SQL's start screen lists the saved connections as Parley 5250's lists its sessions: a star marks a favourite, kept with the connection, and "Favourites" lists them first; "Recent" the last five connections made, newest first, kept in the settings, with "Save as connection…" for one not saved; the search (from four saved connections on) with the keyboard, Up, Down, Enter and Escape; "All connections" open while nothing is listed above it, else folding, kept as left. Quick connect beside the list (above it when the window is narrow): Host, a TLS switch (on at first) and Connect, a connection not saved, named by its host, its user asked at sign-on. No groups. | 43 |
| R-SQL-106 | **A New snippet button** (2026-11-25, asked by the owner: adding snippets was not easy to find; IDEA-61). The Snippets section's header has a `+` "New snippet…" (as the Explorer's has New group) that opens the snippet dialog, titled "New snippet", with the editor's selection, or empty without one. While the operator has no snippets of their own and is not searching, the panel says above the list how to make one: the `+`, or right-click in the editor. | 44 |
| R-SQL-107 | **Save as snippet from the editor** (2026-11-25, IDEA-62). Right-clicking the editor's text offers "Save as snippet…" for the selection, or the statement under the cursor (as the run bar's button, R-SQL-25), and an action on the Shortcuts page (R-SQL-60), Ctrl+Shift+S at first, does the same. | 44 |
| R-SQL-108 | **The placeholders a snippet will ask for** (2026-11-25, IDEA-67). Under its SQL the snippet dialog lists, as the SQL is typed, the placeholders it holds with their defaults ("Asks for: user (QSYSOPR), days"), or says that it asks for nothing, so a mistyped `${…}` is seen before saving. | 44 |
| R-SQL-109 | **The values form shows the SQL** (2026-11-25, IDEA-72). Below its fields the placeholder form (R-SQL-26) shows the snippet's SQL filled with the values as they are typed, exactly as it will run or be inserted, and offers both Insert and Run, the one it was opened for as the filled button. | 44 |
| R-SQL-110 | **Typed placeholders** (2026-11-25, asked by the owner; IDEA-69; *extends R-SQL-26*). A placeholder may name a type before its default: `${name as text}`, `as number`, `as date`, `as name` (`${user as text:QSYSOPR}`). Text goes in quoted, a `'` in it doubled (`O'Brien` → `'O''Brien'`); a number is a sign, digits and one decimal point or comma, written with a point; a date is a real `YYYY-MM-DD`, written quoted; a name is an SQL name (A–Z, 0–9, `_`, `$`, `#`, `@`, not starting with a digit or `_`, up to 128), put in capitals, or a delimited `"name"` kept as typed, written without quotes. Each field's hint says its type; a value refused is named under the fields and Insert and Run wait until it is put right. Without a type a value goes in as typed, as before. The snippet dialog (R-SQL-108) names an unknown type, and a text or date placeholder written between quotes in the SQL, as mistakes. A pure function in `parley_sql_core::snippet`, tested with `æøå`, quotes, empty values and dates that do not exist. | 44 |
| R-SQL-111 | **Values become placeholders while saving** (2026-11-25, IDEA-65). The snippet dialog lists the SQL's string and number literals (not those in comments, delimited names, placeholders or `X'…'`), each with "Ask for it", which puts a typed placeholder (R-SQL-110) in its place with the literal as its default: a string `as text`, or `as date` when it is `YYYY-MM-DD`; a number `as number`. It is named after the column it is compared with (`USER_NAME = 'QSYSOPR'` → `${user_name as text:QSYSOPR}`; `=`, `<>`, `<`, `>`, `<=`, `>=`, `LIKE`, `IN (`), otherwise `value`, and made unique with a number. A literal holding `}` is not offered. | 44 |
| R-SQL-112 | **A value inside a quoted string keeps the string whole** (2026-11-25, asked by the owner; *changes R-SQL-26:* values went in as typed). A placeholder without a type (or with one Parley does not know) written inside a `'…'` string in the SQL (`'${user}'`, `LIKE '%${text}%'`) has each `'` of its value doubled, so `O'Brien` ends neither the string nor the statement; anywhere else its value still goes in as typed. In the values form's SQL (R-SQL-109), Run and Insert alike. | 44 |
| R-SQL-113 | **Values remembered across restarts** (2026-11-25, asked by the owner; IDEA-73; *extends R-SQL-26:* values were kept until Parley closed). The values form offers, in a menu beside each field, the last five values used for that placeholder name on this connection, newest first; the field starts with the newest (else the default). They are kept in Parley SQL's settings by connection, as its history is, and move with it when it is renamed. A placeholder whose name holds `password`, `passwd`, `pwd` or `secret` (any case) is never kept, nor is an empty value. | 44 |
| R-SQL-114 | **List placeholders** (2026-11-25, IDEA-71; *extends R-SQL-110*). `${name as list}` takes values separated by commas, each trimmed, empty ones dropped, and writes each as text (quoted, a `'` doubled) separated by `, `, for an `IN (…)`; a list with no value is refused. | 44 |
| R-SQL-115 | **Names picked from the host** (2026-11-25, asked by the owner; IDEA-70; *extends R-SQL-110*). `${name as library}` and `${name as table}` are written as `as name` is; while such a field has the keyboard, the values form lists below the fields, to click, the names that start with what is typed (ignoring case), at most 50: for a library, the job's library list and the Explorer's groups (as completion offers them, R-SQL-36, R-SQL-83); for a table, the objects but indexes of the library in the snippet's first `as library` field when it holds a name, else of the job's current library, read from the host when not read yet (as the Explorer reads them). The list keeps its height. | 44 |
| R-SQL-116 | **Run a snippet from the keyboard** (2026-11-25, asked by the owner; IDEA-74). An action on the Shortcuts page (R-SQL-60), Ctrl+Shift+R at first, opens "Run snippet": a search field with the keyboard and the snippets below it, best fit first as the start screens find (forgiving a typo, D64; title first, then tags and about), mine, shared and the starters alike. Up and Down choose, Enter runs, Shift+Enter inserts at the cursor, a double-click runs, Escape or Cancel closes; a snippet with placeholders goes through the values form (R-SQL-26). | 44 |
| R-SQL-117 | **A snippet's result is named after it** (2026-11-25, IDEA-75). A result of a snippet run from the panel, the menu or Run snippet (R-SQL-116) is titled by the snippet's title and the values given ("Active jobs — QSYSOPR, 7"), and its tab has "Run again…", which opens the values form with the values last used (R-SQL-113) and runs it into a new tab. | 44 |
| R-SQL-118 | **Drag a snippet into the editor** (2026-11-25, IDEA-63). A snippet dragged from the panel and dropped on the editor's text goes in where it is dropped, through the values form when it has placeholders. | 44 |
| R-SQL-119 | **A title suggested when saving** (2026-11-25, IDEA-64). "Save as snippet" and "New snippet" with SQL start with a title and tags from the statement: the first table named, in words (`QSYS2.ACTIVE_JOB_INFO` → "Active job info"), after the verb when it is not a query ("Update orders", "Delete from orders", "Call qcmdexc"), and its library in lower case as a tag; empty when nothing is named. | 44 |
| R-SQL-120 | **Placeholders in any statement** (2026-11-25, asked by the owner; IDEA-68; *extends R-SQL-26*). While "Ask for ${…} values" is on in Settings ▸ SQL (on at first), SQL run from the editor (Run, Run all, a selection, a statement's ▶) that holds placeholders opens the values form first, with their types, the values used before and the names offered (R-SQL-110, R-SQL-113, R-SQL-115); Run all asks once for every statement's. Run then runs the filled SQL as Run all does, and parameter markers (R-SQL-40) are asked for after, as before; the History keeps the SQL as it ran. The form offers "Run as written", which runs the SQL with its `${…}` unchanged, and no Insert. While the setting is off, `${…}` goes to the host as written. | 44 |
| R-SQL-121 | **Keep a history, and how much** (2026-11-25, asked by the owner; D66; *changes R-SQL-89:* every run was kept, the last 500). Settings ▸ SQL has "Keep a history" (on at first) and "Runs kept" per connection (500 at first, 50 to 5000). While it is off, no run is added; what is kept stays until History's Clear (R-SQL-91). A smaller number drops the oldest at the next run. | 44 |
| R-SQL-122 | **Remember placeholder values or not** (2026-11-25, asked by the owner; D66; *changes R-SQL-113*). Settings ▸ SQL has "Remember values" (on at first). While it is off, values are offered again only until Parley SQL closes (as R-SQL-26 did) and none are written to the settings; those kept are removed from them. | 44 |
| R-SQL-123 | **Ask before risky statements, or not** (2026-11-25, asked by the owner; *changes R-SQL-9:* it always asked). Settings ▸ SQL has "Ask before risky statements" (on at first); while it is off, the statements R-SQL-9 asks about run without asking. | 44 |
| R-SQL-124 | **How NULL is shown** (2026-11-25, asked by the owner). Settings ▸ SQL has "NULL shows as" (`NULL` at first, any text up to 20 characters, empty allowed) for the grid and a row's values, and "NULL in CSV" (Empty, as before, or `NULL`) for Copy as CSV, Export and Download to CSV (R-SQL-38, R-SQL-71). | 44 |
| R-SQL-125 | **Stay on the tab after a run** (2026-11-25, asked by the owner; *changes R-SQL-14:* a run showed its outcome). Settings ▸ SQL has "Show the outcome after a run" (on at first): Results after a query, Messages after other statements, as today. While it is off, the tab shown stays. | 44 |
| R-SQL-126 | **CSV byte-order mark and header row** (2026-11-25, asked by the owner; *changes R-SQL-38, R-SQL-71:* both were always written). Settings ▸ SQL has "CSV starts with a byte-order mark" (on at first: Excel reads æøå) and "CSV has a header row" (on at first), for Copy as CSV, Export and Download to CSV. | 44 |
| R-SQL-127 | **Objects read per library** (2026-11-25, asked by the owner: the owner has libraries with more than 5000 objects, where the Explorer read 5000, a limit in the code). Settings ▸ SQL has "Objects per library" (5000 at first, 1000 to 100000) for the Explorer, completion and the names offered (R-SQL-115); a library cut short says "The first N only.". A changed number counts for libraries read after the change. | 44 |

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

Next free ID: `R-SQL-128` (other areas, decisions and phases: last line of [../ROADMAP.md](../ROADMAP.md); ideas: last line of [../IDEAS.md](../IDEAS.md)).
