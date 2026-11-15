# Parley SQL — product requirements

> What Parley SQL is, who it is for and why, in one place. It summarises the plan; it does not replace it.
> The detailed, binding text is in [ROADMAP.md](ROADMAP.md) (open requirements and decisions) and
> [REQUIREMENTS.md](REQUIREMENTS.md) (done ones), and [FEATURES.md](FEATURES.md) says how each was verified.
> Requirement IDs (`R-SQL-7`) and decisions (`D20`) below point there. The terminal is [../PRD.md](../PRD.md).

**Status:** written 2026-11-14 when the SQL plan got documents of its own; updated after Phase 14. Everything below
is built and verified on Linux under WSL against the owner's IBM i 7.3 (2026-11-14); Windows and macOS not yet.

## 1. Summary

Parley SQL is SQL for IBM i: write statements, run them, read and work with the results, find your way around the
database and use the IBM i tools around it. It is one SQL window with two ways in: Parley's **SQL add-on**, beside a
5250 session (R-SQL-6, R-SQL-10), and **Parley SQL** (`parley-sql`), a program of its own for people who need SQL but
no green screen (R-SQL-45, D26). Both look like Parley and keep no password (D16).

## 2. Problem

IBM i developers and administrators reach the database through IBM i Access Client Solutions' Run SQL Scripts, a
large Java application, or through generic database tools that know nothing about IBM i: no CL commands, no
library list, no column headings, no message help. Parley SQL keeps those IBM i habits (ACS's `CL:` prefix, the
library list, `LABEL ON` headings) in a fast desktop window.

## 3. Users

| Who | What they need |
|---|---|
| **IBM i developers** | Run and format SQL, scripts in tabs and files, completion, the schema browser and DDL, parameters, snippets. |
| **Administrators and support** | IBM i Services at hand ("did you know" snippets), the job log and message help, one query on several systems. |
| **People without a 5250 habit** | Parley SQL on its own, saved connections, and an assistant that writes SQL from plain words. |

## 4. Goals and non-goals

**Goals**

1. Run any statement IBM i accepts, with every data type shown correctly, through IBM's own client (D20).
2. Safe by default: commit control, a guard for changes without `WHERE`, read-only across systems (R-SQL-5,
   R-SQL-9, D24).
3. Know the database without waiting for a running query: a second connection for the catalog (D21).
4. Nothing leaves the machine but to the IBM i, unless the operator turns the assistant on; then names only (D25, D27).

**Non-goals** (decided; changing one needs a new decision)

- Bundling a Java runtime: Java 11+ is installed by the user (R-SQL-12, D20).
- Export to Excel (R-SQL-17 dropped: CSV only).
- Several connections at once in Parley SQL (D26); Run on… (R-SQL-43) went with Parley's own SQL window (R-SQL-70).

## 5. What Parley SQL does

| Area | Capability | IDs | State |
|---|---|---|---|
| Connection | A Java helper with JTOpen, Java found or explained; sign-on, TLS, commit control | R-SQL-3..5, R-SQL-11..13 | Verified |
| Writing | Editor in colour, scripts in tabs and files, format, find and replace, a visible Run selection | R-SQL-7, R-SQL-19..23, R-SQL-28 | Verified |
| Results | Tabs, row detail, filter, headings, Copy as, hex, compare, charts; below or beside the editor | R-SQL-8, R-SQL-14..16, R-SQL-29, R-SQL-38, R-SQL-39, R-SQL-41, R-SQL-42, R-SQL-49 | Verified |
| The database | Schema browser, DDL, completion, names on hover | R-SQL-33..37 | Verified |
| IBM i tools | `CL:` commands, message help and job log, library list and schema | R-SQL-30..32 | Verified |
| Reuse | Snippets with placeholders, starters from IBM i Services; parameter markers | R-SQL-24..27, R-SQL-40 | Verified |
| Several systems | One read-only statement on several saved sessions; removed with Parley's SQL window (R-SQL-70) | R-SQL-43 | Removed |
| Assistant | Plain words to SQL, names only, from Anthropic, OpenAI, Gemini, Mistral or a local llama.cpp | R-SQL-44, R-SQL-48, R-ADD-3 | Verified |
| Parley SQL | The program on its own: saved connections, its own settings | R-SQL-45..47 | Verified |
| From Parley | Parley's Tools ▸ SQL… starts Parley SQL on the session's host | R-SQL-70 | Verified |
| Data transfer | Every row to CSV or Excel; a CSV file into a table, all or nothing | R-SQL-71..73 | Verified |
| Editor, safer | Scripts kept within seconds, an error's place marked, a `WITH`'s names completed, formatting choices | R-SQL-74..77 | Verified |
| IBM i Services | Under System in the tree: the user's spooled files, message queue and jobs, listed under the editor; a spooled file or job log in a viewer window; more starters | R-SQL-78..81 | Verified |
| Layout | Results below or beside the editor, the editor's height, the side panel folded | R-SQL-49..51 | Verified |
| Work area | A side panel on the left with the IBM i as the tree's root, library groups per connection (Development, Testing, …) and Snippets below; History as a tab beside Results and Messages (D33) | R-SQL-82..85 | Verified |
| History | Every run with when, how long and how it ended, by day, with search and a filter; insert, run again, copy, remove (D35) | R-SQL-89..91 | Verified |
| Editing | Comment, indent, move and duplicate lines, pairs, matching bracket, go to line, keyword case, zoom | R-SQL-52..59 | Verified |
| Editor, deeper | Gutter with line numbers, ▶ and outcomes; completion on Ctrl+Space and with snippets; a routine's parameters; the word under the cursor, Ctrl+D; trailing blanks trimmed (D28) | R-SQL-61..68 | Verified |
| Shortcuts and settings | Parley SQL's Shortcuts page; Parley SQL's settings kept in Parley SQL only | R-SQL-60, R-SQL-69 | Verified |

## 6. Solution overview

The window lives in `parley-ui-egui` (`sql`); its engine in `parley_core::sql`. The database work runs in a small
Java helper, started per window and spoken to in JSON lines over stdin/stdout, which uses IBM's JTOpen JDBC driver
(D20). A second helper reads the catalog (D21). Parley SQL is a second binary over the same window (D26). More on the
design: [../ARCHITECTURE.md](../ARCHITECTURE.md).

## 7. Release plan

- **Phases 5–12 (built 2026-11-13..14):** see [ROADMAP.md](ROADMAP.md) §3.
- **Next:** the owner's checks in the running app, so the requirements can move to REQUIREMENTS.md; then a phase
  picked from [IDEAS.md](IDEAS.md) (IBM i Services views, data transfer).

## 8. Risks and open questions

| Risk or question | How it is handled |
|---|---|
| JTOpen's licence (IBM Public License 1.0) shipped with an MIT/Apache app. | Read and recorded in D20; a legal check by SoftDesign may follow before the first release with SQL. |
| Users must install Java. | Found or explained (R-SQL-12); only SQL needs it. |
| Java's trust store differs from the operating system's (D6). | Open in D20, settled while building R-SQL-3. |
| The assistant sends names to a provider. | Off by default, says what is sent; local llama.cpp keeps it on the machine (D25, D27). |
