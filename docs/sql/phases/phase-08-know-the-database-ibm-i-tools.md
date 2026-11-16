# Phase 8 — Know the database, IBM i tools

> Finished phase of Parley SQL. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

From IDEA-29 and IDEA-30 (2026-11-13), in this order: the IBM i tools first, small and on the connection there is
(CL commands R-SQL-30, message help and the job log R-SQL-31, library list and current schema R-SQL-32); then the
second connection that reads the catalog (R-SQL-33, D21), the schema browser on it (R-SQL-34), the DDL of an object
(R-SQL-35), completion (R-SQL-36) and names on hover (R-SQL-37). The owner's choices: ACS's `CL:` prefix, the
library list plus pinned libraries in the browser, completion as one types.

**Status:** Done, verified (2026-11-14, Linux under WSL): checked on the owner's 7.3 as NODEJT400 (D22): CL commands (R-SQL-30), message help and the job log (R-SQL-31), library list and current schema (R-SQL-32), a second connection (R-SQL-33, D21), the schema browser (R-SQL-34), DDL (R-SQL-35), completion (R-SQL-36), names on hover (R-SQL-37).

## Requirements

| ID | Requirement |
|---|---|
| R-SQL-30 | **CL commands** (2026-11-13). A statement that starts with `CL:` (as in ACS Run SQL Scripts, so its scripts run as they are) runs the rest as a CL command through `QSYS2.QCMDEXC` in the SQL job; it is coloured as a command, goes into the history, and its completion or error message goes to Messages. Run all runs it in its place. |
| R-SQL-31 | **Message help and the job log** (2026-11-13). An SQL error in Messages shows its message ID (SQL0204), the first-level text, and, on asking, the second-level text (cause and recovery) from `QSYS2.MESSAGE_FILE_DATA` for QSQLMSG. "Job log" shows the SQL job's newest messages (`QSYS2.JOBLOG_INFO` for `*`), so a failed statement's detail can be read without a 5250 session. |
| R-SQL-32 | **Library list and current schema** (2026-11-13). The top bar shows the SQL job's current schema; a menu lists its library list (`QSYS2.LIBRARY_LIST_INFO`) and sets the current schema (`SET SCHEMA`) or adds a library (`ADDLIBLE`). Read again after a statement that may change them (`SET SCHEMA`, `CL:`). |
| R-SQL-33 | **A second connection for the catalog** (2026-11-13; D21). At Connect the helper signs on twice with the password typed, then forgets it: the first job runs the operator's statements, the second only reads the catalog for R-SQL-34..37, so browsing never waits for a running query. Each answer says which job it came from; the top bar names both jobs. If the second sign-on fails, the window works without the browser and says why. |
| R-SQL-34 | **The schema browser** (2026-11-13). A Database tab beside History and Snippets: the library list and the libraries pinned (kept per session; any library can be found by name and pinned), each opening to tables, views, indexes, procedures and functions (`QSYS2.SYSTABLES`, `SYSINDEXES`, `SYSROUTINES`), each table to its columns with type, heading and text. A search filters by name; double-click inserts the qualified name at the cursor; read lazily, a level at a time, with Refresh. *Changed 2026-11-15 by R-SQL-82 and R-SQL-83 (Libraries on the left, groups in place of pinned libraries).* |
| R-SQL-35 | **The DDL of an object** (2026-11-13). On an object in the browser, "Show SQL" opens its DDL (`QSYS2.GENERATE_SQL`) in a new script tab; "Select first rows" puts a `SELECT … FETCH FIRST 100 ROWS ONLY` there and runs it. |
| R-SQL-36 | **Completion** (2026-11-13). As one types a name in the editor, after a `.` or a few letters, a list offers the libraries, tables and views known to the browser, and, after `table.` or an alias declared in the statement, its columns (with type and heading); Up/Down choose, Tab or Enter accepts, Esc closes, typing goes on. SQL keywords are offered too. Names come from the browser's cache, read on the second connection when missing; never blocks typing. |
| R-SQL-37 | **Names on hover** (2026-11-13). Hovering a table or view name in the editor shows its text and columns (name, type, heading), from the browser's cache. |

## Verification

- **R-SQL-30..33** (2026-11-13): IBM i tools and the second connection, built. `CL:` statements run through
  `QSYS2.QCMDEXC` (coloured, kept as written in the history, DLT/CLR/END commands confirmed); Explain on an error
  with a message ID (job log first, then QSQLMSG/QCPFMSG), Job log as a result tab; the current schema and library
  list in the top bar (click sets the schema, Add runs `CL: ADDLIBLE`), read after Connect and after `SET`/`CL:`;
  a second helper signs on only after the first did, its job named in the top bar. **Automated:** parsing, guard,
  colour and format of CL; message IDs and format codes (`æøå`); Explain's fallback; the job context; the second
  connection's states and that a refused first sign-on stops it. `sqlprobe --tools` ran on the owner's 7.3. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-34, R-SQL-35** (2026-11-13): the schema browser, built. A Database tab beside History and Snippets: the
  SQL job's library list and the libraries pinned (kept per saved session), each opening to tables, views,
  indexes, procedures and functions, a table or view to its columns (type; heading and text on hover); read on
  the second connection, a level at a time, with a filter and Refresh. Double-click inserts the name; the
  right-click menu has Insert name, Show SQL (`QSYS2.GENERATE_SQL` into a new script tab) and Select first rows.
  **Automated:** the catalog queries' quoting, rows to objects and columns (`æøå`), DDL lines, the question
  queue, pinning, and a kittest that opens a library to its tables. **Checked:** `sqlprobe --tools` on the
  owner's 7.3 as NODEJT400 (D22); the owner in the app (pinning).

- **R-SQL-36** (2026-11-13): completion as you type, built. After two letters or a `.`, a list under the cursor offers
  the columns of the statement's tables (by alias too, with type and heading), the tables and views of the current
  schema or of `library.`, the libraries and SQL keywords; Up/Down, Tab or Enter, Esc. Names come from the
  browser's cache; what is missing is asked on the second connection and offered when answered. **Automated:**
  the name typed and its qualifier (not in text or comments, `æøå`), tables and aliases from FROM/JOIN, what is
  offered from the cache, replacing the typed part, and the keys in a kittest. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-37** (2026-11-13): names on hover, built. Hovering a table or view name in the editor (or its alias)
  shows its qualified name, kind, text and up to 25 columns with type and heading, from the browser's cache (read
  when missing). **Automated:** the name and library under the pointer (not in comments, `æøå`), aliases, columns
  of an alias left alone. **Checked:** completion and hover against the owner's 7.3 as NODEJT400 (D22): 9 tables
  after `NODEJT400.`, 4 columns after `x.`, the hover text. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).
