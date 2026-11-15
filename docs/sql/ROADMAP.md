# Parley SQL roadmap

> The plan for SQL on IBM i in Parley: the SQL window, run as Parley's SQL add-on and as the program `parley-sql`
> (Parley SQL), and the helper, host-server and assistant code under it. Completed requirements move to
> [REQUIREMENTS.md](REQUIREMENTS.md), and how each was verified is in [FEATURES.md](FEATURES.md). The terminal's
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
| Phase 5: SQL against the host | Done, verified (2026-11-14, Linux under WSL): the SQL window over a Java helper with IBM's JTOpen (D20): R-SQL-3..13. Built first: R-NET-8, R-SQL-1 and R-SQL-2 (the native sign-on, kept for the probe), verified on the owner's IBM i. |
| Phase 6: the SQL window, deeper | Done, verified (2026-11-14, Linux under WSL): more from results (R-SQL-14..16, R-SQL-18; R-SQL-17 dropped) and editor and scripts (R-SQL-19..23). |
| Phase 7: snippets and column headings | Done, verified (2026-11-14, Linux under WSL): snippets (R-SQL-24..27), a visible Run selection (R-SQL-28), IBM i column headings (R-SQL-29). Then Phase 8 (IDEA-29, IDEA-30), Phase 9 (IDEA-31), Phase 10 (IDEA-32, IDEA-33), chosen by the owner the same day. |
| Phase 8: know the database, IBM i tools | Done, verified (2026-11-14, Linux under WSL): checked on the owner's 7.3 as NODEJT400 (D22): CL commands (R-SQL-30), message help and the job log (R-SQL-31), library list and current schema (R-SQL-32), a second connection (R-SQL-33, D21), the schema browser (R-SQL-34), DDL (R-SQL-35), completion (R-SQL-36), names on hover (R-SQL-37). |
| Phase 9: parameters and more from results | Done, verified (2026-11-14, Linux under WSL): copy as Markdown, `INSERT` or JSON (R-SQL-38), hex and EBCDIC (R-SQL-39), a parameter form (R-SQL-40), compare results (R-SQL-41), charts (R-SQL-42); D23. |
| Phase 10: several systems and an SQL assistant | Done, verified (2026-11-14, Linux under WSL): one statement on several systems (R-SQL-43, D24), an SQL assistant from plain words (R-SQL-44, D25), as an add-on off by default (R-ADD-3). |
| Phase 11: Parley SQL | Done, verified (2026-11-14, Linux under WSL): the SQL window as a program of its own, `parley-sql` (R-SQL-45), with saved connections (R-SQL-46) and its own settings pages (R-SQL-47); D26. Parley's SQL add-on is unchanged. |
| Phase 12: the assistant's providers | Done, verified (2026-11-14, Linux under WSL): Settings ▸ SQL assistant in both programs, with the switch, the provider (Anthropic, OpenAI, Google Gemini, Mistral, llama.cpp on a local `llama-server`, or any OpenAI-compatible server), the model and address, and the API key in the keychain (R-SQL-48); D27. |
| Phase 13: the editor, handier | Done, verified (2026-11-14, Linux under WSL): asked by the owner: comment lines (R-SQL-52), indent and outdent (R-SQL-53), move and duplicate lines (R-SQL-54), brackets and quotes that close themselves (R-SQL-55), the matching bracket (R-SQL-56), go to a line (R-SQL-57), keyword case (R-SQL-58), zoom the editor (R-SQL-59); then Parley SQL's Shortcuts page (R-SQL-60). |
| Phase 14: the editor, deeper | Done, verified (2026-11-14, Linux under WSL): from IDEA-34 (the owner): on egui's text field with a gutter of Parley's own (D28): line numbers (R-SQL-61), a ▶ and the last outcome per statement (R-SQL-62), completion on demand and with snippets (R-SQL-63, R-SQL-64), a routine's parameters while it is called (R-SQL-65), the word under the cursor marked (R-SQL-66), Ctrl+D (R-SQL-67), trailing blanks trimmed on save (R-SQL-68). |
| Phase 15: data transfer | Done, verified (2026-11-15, Linux under WSL): every row to CSV or Excel (R-SQL-71, R-SQL-72), a CSV file into a table, all or nothing (R-SQL-73); D31. |
| Phase 16: the editor, safer | Done, verified (2026-11-15, Linux under WSL): scripts kept within seconds (R-SQL-74), an error's place marked (R-SQL-75), completion of a `WITH`'s names (R-SQL-76), formatting choices (R-SQL-77). |
| Phase 17: IBM i Services | Done, verified (2026-11-15, Linux under WSL; the probe on the owner's IBM i 7.3): a System node under the IBM i in the side panel's tree (D32): spooled files (R-SQL-78), the message queue (R-SQL-79), jobs and job logs (R-SQL-80); more starters (R-SQL-81). |
| Phase 18: the work area, rearranged | Done, verified (2026-11-15, Linux under WSL), built before Phase 17 at the owner's wish: the Libraries panel on the left (R-SQL-82), library groups (R-SQL-83), Snippets under Libraries (R-SQL-84), History beside Results and Messages (R-SQL-85); D33. |
| Phase 19: a fuller History | Done, verified (2026-11-15, Linux under WSL): every run kept with when, how long and how it ended (R-SQL-89), listed by day with search and a filter (R-SQL-90), and what can be done with an entry (R-SQL-91); D35. |

## 4. Phases

Phase numbers continue Parley's (Phases 0–4 are the terminal's); the next free one is on the last line of
[../ROADMAP.md](../ROADMAP.md).

### Phase 5 — SQL against the host (done)

The SQL window of IDEA-13. The database work runs in a small Java helper that uses IBM's JTOpen JDBC driver (D20):
IBM's own client covers every data type, TLS, all password levels, commit control and cancel, where a native client
(D19) would have to reverse-engineer the database server's requests. Parley talks to the helper in JSON lines and
keeps the window, the history and the safety checks.

1. **The helper** (first): the JTOpen licence checked and the jars shipped (R-SQL-13), Java found or explained
   (R-SQL-12), the helper and its protocol (R-SQL-11), a connection (R-SQL-3), statements and results (R-SQL-4),
   commit control (R-SQL-5). Checked against the owner's IBM i (no TLS); TLS when a host with TLS host servers and a
   user on it can be had.
2. **The SQL window** in the UI (R-SQL-6..10).

Already built on the way (D19): the host-server records and port mapper (R-SQL-1), the native sign-on (R-SQL-2) and
the probes (R-NET-8, R-NET-9's sign-on). They stay for `make probe-sql` and for later host-server features without
Java (such as IDEA-25's message queue); the SQL window does not use them.

Out of scope here: IBM i Services views (IDEA-25), data transfer (IDEA-26), stored procedure result sets beyond the
first, LOB contents (shown as "(LOB)"), bundling a Java runtime (Java is installed by the user, R-SQL-12).

### Phase 6 — The SQL window, deeper (done)

The owner chose two areas of the SQL add-on (2026-11-13), built in this order:
1. **More from results:** a tab per query (R-SQL-14), a row's values one under another (R-SQL-15), filtering and
   resizing in the grid (R-SQL-16), the rows fetched at a time and a query timeout as settings (R-SQL-18).
   *R-SQL-17, export to Excel, was dropped (the owner, 2026-11-13): CSV only, no new dependency.*
2. **Editor and scripts:** SQL in colour (R-SQL-19), several scripts in tabs (R-SQL-20), open and save `.sql`
   files (R-SQL-21), formatting (R-SQL-22), find and replace (R-SQL-23).

Not here: the schema browser and completion, the IBM i tools (library list, CL, job log) and the stand-alone
client (IDEA-28), for later phases.

### Phase 7 — Snippets and column headings (done)

The owner's ideas for the SQL add-on (2026-11-13), in this order: a visible Run selection (R-SQL-28), IBM i column
headings in the grid (R-SQL-29), then the snippets library (R-SQL-24..27). The owner also chose, for the phases
after it: the schema browser with completion and the IBM i tools (Phase 8: IDEA-29, IDEA-30), parameters and more
from results (Phase 9: IDEA-31), several systems at once and an SQL assistant (Phase 10: IDEA-32, IDEA-33, the
assistant only after a decision on what may leave the machine). Their requirements are written when each starts.

### Phase 8 — Know the database, IBM i tools (done)

From IDEA-29 and IDEA-30 (2026-11-13), in this order: the IBM i tools first, small and on the connection there is
(CL commands R-SQL-30, message help and the job log R-SQL-31, library list and current schema R-SQL-32); then the
second connection that reads the catalog (R-SQL-33, D21), the schema browser on it (R-SQL-34), the DDL of an object
(R-SQL-35), completion (R-SQL-36) and names on hover (R-SQL-37). The owner's choices: ACS's `CL:` prefix, the
library list plus pinned libraries in the browser, completion as one types.

### Phase 9 — Parameters and more from results (done)

From IDEA-31 (2026-11-14), in this order, the small ones first: copy a result as Markdown, `INSERT` statements or
JSON (R-SQL-38); binary values as hex and EBCDIC (R-SQL-39); a form for parameter markers (R-SQL-40, D23); compare
two results (R-SQL-41); quick charts (R-SQL-42). The owner's choices (D23): parameters as a prepared statement's
values, charts as bars and lines with `egui_plot`, rows compared by key columns.

### Phase 10 — Several systems and an SQL assistant (done)

From IDEA-32 and IDEA-33 (2026-11-14): run one read-only statement on several saved sessions, a result tab each
(R-SQL-43, D24); then an SQL assistant that turns plain words into SQL with the catalog's names only (R-SQL-44,
D25), an add-on that is off until turned on (R-ADD-3). The owner's choices: a tab per system, read-only statements
only across systems, names only to the model, Claude Opus 5.5.

### Phase 11 — Parley SQL (done)

From IDEA-28 (the owner, 2026-11-14): the SQL window offered on its own as **Parley SQL**, for people who need SQL
on IBM i but no 5250 session (R-SQL-45), still in this repository and still Parley's SQL add-on. Its main window has
a header with a gear for Settings at the right and a status bar, in Parley's look; saved connections replace saved
sessions (R-SQL-46); Settings shows only the pages that apply (R-SQL-47). The choices are D26.

### Phase 12 — The assistant's providers (done)

Asked by the owner (2026-11-14): the SQL assistant gets a settings page of its own, where it is turned on and off,
and works with the major AI providers and with a local model through llama.cpp (R-SQL-48). The choices are D27.
Also asked: the results below or beside the editor (R-SQL-49), the editor's height dragged (R-SQL-50) and the side
panel folded away (R-SQL-51).

### Phase 13 — The editor, handier (done)

Asked by the owner (2026-11-14): the small things a code editor is expected to do, on egui's text field as it is
(R-SQL-52..59). Bigger editor work (line numbers, a gutter, an editor widget of Parley's own) is IDEA-34. The text
changes are pure functions in `parley_core::sql::edit`, tested without the UI; the editor turns keys into them.

### Phase 14 — The editor, deeper (done)

From IDEA-34 (the owner, 2026-11-14), in this order: the gutter (R-SQL-61, R-SQL-62), completion (R-SQL-63,
R-SQL-64), a routine's parameters (R-SQL-65), the word under the cursor and Ctrl+D (R-SQL-66, R-SQL-67), trailing
blanks (R-SQL-68). The editor stays egui's text field; the gutter is drawn beside it from the rows it laid out
(D28). Folding and several cursors, which need an editor of Parley's own, stay in IDEA-34.

### Phase 15 — Data transfer (done)

From IDEA-26 (the owner, 2026-11-14): every row of a result or a table to a CSV or Excel file (R-SQL-71, R-SQL-72),
and a CSV file into a table, all or nothing (R-SQL-73), as Access Client Solutions' Data Transfer does (D31).
Download needs no change to the helper; upload adds one request to it.

### Phase 16 — The editor, safer (done)

From IDEA-34's leftovers that fit egui's text field (D28; the owner, 2026-11-15): scripts kept within seconds of a
change (R-SQL-74), the place of an error marked (R-SQL-75), completion of a `WITH`'s names (R-SQL-76), and formatting
choices (R-SQL-77). Folding and several cursors, which need an editor of Parley's own, stay in IDEA-34.

### Phase 17 — IBM i Services (done)

From IDEA-25 (the owner, 2026-11-15), aimed at the signed-on user since Parley SQL has no 5250 session (R-SQL-70): a
"System" node under the IBM i at the root of the side panel's tree (R-SQL-83), with the user's spooled files
(R-SQL-78), message queue (R-SQL-79) and jobs with their job logs (R-SQL-80). The tree is the way in, a list opens as a
tab under the editor, and a spooled file or job log opens in a viewer window of its own; all read-only on the second
connection (D32). Plus more IBM i Services starter snippets (R-SQL-81).

### Phase 18 — The work area, rearranged (done)

Asked by the owner (2026-11-15), built before Phase 17: the side panel moves to the left and becomes resizable
(R-SQL-82); its Database tab becomes Libraries, where the operator sorts libraries into groups per connection, such as
Development and Testing (R-SQL-83); Snippets sits below Libraries (R-SQL-84); History becomes a tab beside Results and
Messages (R-SQL-85). The right side panel goes (D33).

### Phase 19 — A fuller History (done)

From IDEA-35 (the owner, 2026-11-15): the History tab of R-SQL-85 becomes a log of every run, with when it ran, how long
it took and how it ended (R-SQL-89), shown as two-line rows grouped by day with a search and an All / Succeeded / Failed
filter (R-SQL-90); a click selects an entry, and it can be put in the editor, opened in a tab, run again, copied, saved
as a snippet or removed (R-SQL-91). D35.

### Later (unordered, to be planned)

Nothing yet; see [IDEAS.md](IDEAS.md).

## 5. Requirements

Open requirements, in the order they are planned, as `| ID | Requirement | Phase |`. The done ones are in [REQUIREMENTS.md](REQUIREMENTS.md); new
ones come from [IDEAS.md](IDEAS.md). Areas: `SQL` the SQL window, its helper and its host-server client (D19); a few
rows from Parley's areas belong here too: `ADD` (the assistant's add-on switch) and `NET` (the SQL probes).

| ID | Requirement | Phase |
|---|---|---|
| R-SQL-87 | **Explorer and Current library** (2026-11-15; the owner, D34). The side panel's upper section, which holds the IBM i, its System node, the library list and the library groups (R-SQL-83), is headed "Explorer", not "Libraries"; its buttons say so ("Fold Explorer", "Show Explorer and Snippets"). The top bar's menu (R-SQL-32) reads "Current library MYLIB", not "Schema MYLIB"; its hover says it is where unqualified tables are looked for, the SQL current schema (`SET SCHEMA`), and a library in its list is made "the current library". What runs on the host is unchanged. *Changes R-SQL-32, R-SQL-82, R-SQL-83 and R-SQL-84 (their names only).* | UI clean-up |
| R-SQL-88 | **Snippets as a tree** (2026-11-15; the owner, D34). The Snippets section (R-SQL-84) shows Mine, Shared and Did you know as groups that fold like the Explorer's library groups, each header with its count; a group with nothing to show, or nothing matching the search, is left out, and while a search is typed every group is open. Each snippet is one line: its title, and its tags in muted text on the right; while the pointer is on the line a Run button takes the tags' place. Double-click inserts it and the hover shows its about and SQL, as before; its right-click menu has Insert and Run, and for mine and shared ones Edit… and Delete… (asking first). *Changes R-SQL-25 (Run, Edit and Delete were buttons on every line, tags a line of their own).* | UI clean-up |

## 6. Decisions

Decisions D1–D18 (language, crates, TLS, the look, add-ons, no saved passwords, …) are in
[../ROADMAP.md](../ROADMAP.md) §7 and bind this work too.

| ID | Question | Decision | Alternatives considered |
|---|---|---|---|
| D19 | How does the SQL window (IDEA-13) reach the database? *Replaced by D20 for running SQL (2026-11-13); the sign-on it brought (R-SQL-1, R-SQL-2) stays for the probe.* | **A native client for the IBM i host servers** (the owner, 2026-11-13): the sign-on server (8476, 9476 with TLS) and the database host server (8471, 9471 with TLS), found through the port mapper (449), the protocol Access Client Solutions and IBM's ODBC driver use. Built sans-I/O in `parley-core` as the 5250 protocol is (D4), not ported from JTOpen (IBM Public License). No IBM document gives the request layouts, so the reference is Manticore (MIT, github.com/TheManticoreProject/Manticore), an independent Go client built by observing JTOpen as a black box (the owner, 2026-11-13); it is credited where its layouts are used, and every request is confirmed against a real IBM i with the recording the owner runs (R-NET-9). The password substitute is RFC 4777's. Sign-on covers password levels 0–1 (DES) and 2–3 (SHA), since the owner's IBM i is at level 0 and PUB400 at 3. TLS is used when the session's TLS is on and checked as D6; without TLS it still works, as plain TN5250 does, with a visible warning, since the owner's IBM i offers the host servers without TLS only. Tests use recorded traffic; TLS is tried against PUB400, whose host servers offer it. The probe (R-NET-8) found the host servers and DRDA on both systems, Mapepire only on PUB400. | Mapepire (needs a server installed; not on the owner's IBM i); DRDA on 446 (published standard, kept as the fallback); IBM's ODBC driver (each user installs it); SSH and `db2util`. |
| D20 | How does the SQL window run SQL? Replaces D19 for it; changes R-P-4. | **A Java helper process with IBM's JTOpen JDBC driver** (the owner, 2026-11-13). Parley starts a small Java program of its own when an SQL window opens and talks to it in JSON lines over stdin/stdout (R-SQL-11); the helper does the database work with `jt400.jar`, shipped unmodified beside it (R-SQL-13). Java 11 or later is installed by the user and only the SQL add-on needs it (R-SQL-12, D15); no Java runtime is bundled, so the installers stay small. A helper process rather than JNI: a JVM crash or out-of-memory ends only the helper, finding Java needs only a `java` executable, and the workspace keeps denying `unsafe` code. Why not D19: after the sign-on, the database server's requests and result formats are in no public document and no permissive open-source client, while JTOpen is IBM's own, complete client. **Licence** (read 2026-11-13 from the IBM Public License 1.0 text inside `jt400.jar` 21.0.7; the owner agreed to go ahead, a legal check by SoftDesign may follow before the first release with SQL): shipping the unmodified jar is allowed when the installers carry the licence text, IBM's copyright line and where its source is (`helpers/sql/NOTICE.txt`, README.md), and warranties and liability are disclaimed; Parley's own code, the helper included, is a separate module and not a derivative work, so it stays MIT/Apache. The jar is downloaded at build from Maven Central, pinned to 21.0.7 and its SHA-256 (`scripts/fetch-jt400.sh`), not kept in git (the owner). Open, settled while building: Java's trust store against D6's operating-system one (R-SQL-3). | The native client of D19; JNI into an embedded JVM; IBM's ODBC driver; Mapepire (needs a server on the IBM i); bundling a Java runtime (40–60 MB per installer). |
| D21 | Where do the schema browser and completion read the catalog? | **A second connection, signed on at Connect** (the owner, 2026-11-13). Parley keeps no password (D16), so the helper signs on twice with the password typed, then forgets it; the second job only reads the catalog, so browsing and completion never wait for a running query. Built as a second helper process (the same jar, no protocol change), started with the first and signed on **only after the first sign-on succeeds**, so a mistyped password counts once against QMAXSIGN, not twice. Cost: one more QZDASOINIT job and one more JVM per SQL window. Rejected: sharing the one connection (browsing waits behind a long query); one process with two connections (more Java for the same result). |
| D22 | May Claude sign on to an IBM i to check its own work? | **Yes, to the owner's test system as the test profile NODEJT400 only** (the owner, 2026-11-13). The probes (`sqlprobe`) read its password from the file `PARLEY_PASSWORD_FILE` names, which the owner writes and Claude never prints; read-only checks that print counts, not data rows, and only the library NODEJT400 is looked into. Anything that changes something is asked first. Never PUB400, never another profile. The Parley app still keeps no password (D16). |
| D23 | Phase 9: how do parameters, charts and comparing work? | **The owner, 2026-11-14:** parameter markers go to IBM i as a prepared statement's values (the helper's Run takes a `params` list; IBM i converts the text to each parameter's type; quotes are no problem and the history keeps the statement with its markers), not substituted into the text. Charts as bars and lines with the `egui_plot` crate (MIT/Apache-2.0, by egui's authors). Comparing matches rows by key columns (the first by default), not by position. |
| D24 | Phase 10: how does one statement run on several systems? | **The owner, 2026-11-14:** a result tab per system (Compare, R-SQL-41, sets two side by side); only statements that read may run across systems, so testing against production cannot change it. Each system signs on in the SQL window with a password typed for it and forgotten after (D16), in a helper process of its own (as D21's second connection); its first block of rows only, so no cursor stays open on a system the window does not show. |
| D25 | Phase 10: the SQL assistant: what leaves the machine, which model, how it is turned off? | **The owner, 2026-11-14:** names only: the question, the current schema and library list, and the names, types, headings and texts the model asks for through two tools (a library's tables, a table's columns), read through the catalog connection (D21); never row data, results, scripts or passwords. Claude Opus 5.5 (`claude-opus-5-5`) on Anthropic's Messages API (`2026-10-21`), called over HTTPS with `ureq` (no Rust SDK exists), with the server-side refusal fallback (`fallbacks: "default"`). The key is the user's own, read from `ANTHROPIC_API_KEY` when asking, never stored. An add-on, off by default (R-ADD-3, D15). |
| D26 | IDEA-28: how is the SQL window offered on its own? | **The owner, 2026-11-14: as Parley SQL, a program of its own in this repository, while staying Parley's SQL add-on.** Built as a second binary, `parley-sql`, in `parley-app` (whose shared desktop services become a small library) over `ParleySqlUi` in `parley-ui-egui`, which draws the same `SqlWindow` in its main window rather than in a window of its own; one window and one connection at a time, as in Parley; and (the owner, 2026-11-14) without the add-on's Run on… (R-SQL-43), so it never signs on to more than one system. It ships in Parley's installers for now (not an installer of its own). Its saved connections and settings are kept in its own eframe storage (`parley-sql`), apart from Parley's sessions, and keep the user but never a password (D16); the snippet folder is shared with Parley. | A choice at Parley's start between terminal and SQL; one program with a `--sql` option; sharing Parley's saved sessions (they carry terminal settings an SQL client does not need); an installer of its own (later, if wanted). |
| D27 | Phase 12: which AI providers does the SQL assistant use, where are their keys, and how is llama.cpp used? | **The owner, 2026-11-14:** a list of named providers (Anthropic, OpenAI, Google Gemini, Mistral, llama.cpp) plus "OpenAI-compatible" with an address of one's own, spoken through two formats: Anthropic's Messages API (as D25) and the OpenAI Chat Completions format, which the others speak (Gemini through its OpenAI endpoint). Keys go in the operating system's keychain (`keyring`, service "Parley SQL assistant", one entry per provider, shared by both programs), saved from the settings page; the provider's environment variable comes first. The settings keep only the provider, model and address. llama.cpp is a `llama-server` the operator runs (default `http://localhost:8080/v1`), not started or built into Parley. The switch is on the assistant's own page only. What is sent is still names only (D25); for llama.cpp it stays with the operator's server. *Changes D25's "never stored" for keys and its single provider.* | Environment variables only (D25); a key typed once per run; Parley starting `llama-server`; llama.cpp built into Parley (a C++ toolchain on every OS); Azure OpenAI (its own address and key header); the switch on both the Add-ons page and the new page. |
| D28 | Phase 14: what does the script editor grow on? | **egui's text field, with a gutter of Parley's own** (the owner, 2026-11-14). The text field gives the rows it laid out after it is drawn, so line numbers, a ▶ and an outcome per statement and error marks are drawn beside it, also with wrapping; the text changes stay in `parley_core::sql::edit`. Folding and several cursors are not possible this way and wait in IDEA-34. | An editor widget of Parley's own (text, cursor, selection, undo, IME and accessibility to write and keep); an egui code-editor crate (built on the same text field and behind egui's versions). |
| D29 | R-SQL-69: where do the settings of the SQL window Parley opens come from, once Parley's Settings has none of Parley SQL's? | **The owner, 2026-11-14: from Parley SQL's saved settings,** read (never written) by Parley from Parley SQL's eframe storage each time it opens the window, so they are set once, in Parley SQL. *Changes D26's "settings kept apart" for these settings only;* sessions, connections, history, scripts and pinned libraries stay apart. | Built-in defaults in Parley's window (no Java path, no assistant in Parley); Tools ▸ SQL… starting the Parley SQL program instead of a window in Parley; keeping both sets of settings. |
| D30 | R-SQL-70: what does Parley's SQL add-on open? | **The owner, 2026-11-14: Parley SQL itself, as a program of its own,** started by Parley from beside it on the session's host, exactly as when started on its own, with its own menus, settings and saved connections. Parley's SQL window, with Run on… (R-SQL-43) and the history Parley kept per session, is removed, not moved. *Changes D26* (Parley SQL is no longer also drawn inside Parley) *and D29* (Parley no longer reads Parley SQL's settings). | Parley's own window reading Parley SQL's settings (D29); Parley SQL opening on its start screen without a host; moving Parley's per-session history to Parley SQL. |
| D31 | Phase 15 (IDEA-26): how are rows downloaded and uploaded, and in which formats? | **The owner, 2026-11-14:** download writes CSV and Excel `.xlsx`; Excel is written in Rust with `rust_xlsxwriter` (MIT or Apache-2.0). Download fetches the result's open cursor to its end through the helper as it is (no new request) and writes each batch to the file as it arrives. Upload is all or nothing: one new helper request prepares an `INSERT` and runs the rows as JDBC batches in one transaction under commitment control, committing only when every row went in; it needs a journaled table, and says so when there is none. Upload reads CSV only. | Keep the good rows on upload (each row on its own, failures to a CSV file); a choice per upload; CSV only for download; Excel written by the helper (Apache POI, a large Java dependency); upload from Excel files. |
| D32 | Phase 17 (IDEA-25): where do the IBM i Services views live, and on which connection? | **The owner, 2026-11-15 (after D33 removed the side panel's tabs):** the tree is the way in, the bottom shows the list. A node "System" under the IBM i at the root of the side panel's tree holds Spooled files, Message queue and Jobs, with counts; they are also in the IBM i's and System's right-click menus. Clicking one opens its list as a closable tab under the editor, beside Results, Messages and History, in the result grid, so it gets the width and the grid's filter and Copy. A spooled file's text and a job log open in a viewer window of their own, not in the editor. Read on the second connection (D21), so the SQL job stays free for the operator's statements and Cancel; read-only; read when opened and on Refresh; nothing kept in the settings but whether System is open. | The lists in the tree only (too narrow for their columns, too long for 300 spooled files); the IBM i's right-click menu only (hard to find); a read-only editor tab for a spooled file or job log; the three nodes straight under the IBM i; a side-panel tab "IBM i" (proposed before D33); a menu of views opening result tabs with the user's own SQL; the main connection; spooled files as PDF (needs a converter on the host or in Parley). |
| D33 | Phase 18: how is the SQL window's work area laid out, and what are library groups? | **The owner, 2026-11-15:** one side panel, on the left, that can be resized and folded, with Libraries above Snippets, each section folding on its own; History is a tab beside Results and Messages, with what was run; the right side panel goes. Libraries are sorted into named groups (e.g. Development, Testing), kept per saved connection; a library may be in several. A group only organises what the browser, completion and hover show; it never changes the job's library list or current schema. The tree's root is the IBM i, so actions on the system get a place of their own (its right-click menu). Named with IBM i's word, "libraries" in "groups": not "collections" (IBM i's old SQL word for a schema), "databases" (an IBM i has one), "environments" (suggests another system), or "projects" or "workspaces" (suggest that scripts belong to them). *Changes D32: the "IBM i" tab became the System node under the IBM i (D32, decided again 2026-11-15).* *Changed by D34: the section is named "Explorer".* | An icon strip that switches the left panel between Libraries and Snippets; Snippets as a tab under the editor or in a right panel of its own; groups shared by every connection; a group that sets the library list or current schema; groups that hold scripts too (projects). |
| D34 | UI clean-up: what are the side panel's upper section and the top bar's library called, and how are snippets listed? | **The owner, 2026-11-15:** the upper section is "Explorer": it holds the IBM i and its System node as well as libraries, so "Libraries" undersold it. The top bar says "Current library", IBM i's word, as the panel does; underneath it is still the SQL current schema (`SET SCHEMA`), which is not the job's `*CURLIB`, and the hover says so. Snippets are a tree like the Explorer's: groups that fold, one line per snippet, Run on hover, the rest in a right-click menu. *Changes D33 (the name "Libraries").* | "Database" (the System node is not database content); the IBM i's name as the header (the tree's root shows it already); "Browser"; "Library" or "Default library" in the top bar; snippets on two lines each; a list with a preview of the chosen snippet below it. |
| D35 | Phase 19 (IDEA-35): what does the history keep, and how is it shown? | **The owner, 2026-11-15:** every run is an entry (a log), not one entry per statement, so which run failed is kept; an entry keeps the statement, when, how long and the outcome (the gutter's outcome of R-SQL-62, one type for both), never results; 500 runs per connection in Parley SQL's settings. Shown as two-line rows grouped by day, not in the result grid (it cannot colour SQL and reads as data) nor as list and detail (too wide beside the results). A click selects; double-click or Enter uses. Local dates come from `chrono` (MIT or Apache-2.0, its clock feature only), the time kept as UTC seconds. | One entry per statement with a run count; a log folding repeats; the result grid; list and detail side by side; a history file of its own per connection; `time` or `jiff` for dates. |

## 7. How to change this roadmap

- New SQL work gets a requirement in §5 with the next free `R-SQL` ID (the last line); IDs are never reused or renumbered.
- A requirement in another area (`ADD`, `NET`, …), a decision (`D`) or a new phase takes the next free number from the
  last line of [../ROADMAP.md](../ROADMAP.md), which is then moved on, so a number never means two things.
- A requirement that is implemented, tested and checked in the running app moves to [REQUIREMENTS.md](REQUIREMENTS.md),
  with how it was verified in [FEATURES.md](FEATURES.md).
- A choice that future work must respect becomes a decision in §6.
- An idea is not a plan: it goes in [IDEAS.md](IDEAS.md) until it is picked, then becomes requirements here.

Next free ID: `R-SQL-92` (other areas, decisions and phases: last line of [../ROADMAP.md](../ROADMAP.md); ideas: last line of [../IDEAS.md](../IDEAS.md)).
