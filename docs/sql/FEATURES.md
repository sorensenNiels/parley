# Parley SQL features

> SQL features (the SQL window, Parley's SQL add-on and Parley SQL) that are implemented **and verified**, by
> requirement ID. The requirements they were built to are in [REQUIREMENTS.md](REQUIREMENTS.md); open work and new
> requirements are in [ROADMAP.md](ROADMAP.md). Look an ID up with `grep -n` rather than reading the whole file.
>
> Add an entry only when the requirement is fully met, has automated tests where practical, and has
> been checked in the running app on at least one OS. Note which OSes were checked. Then move the
> requirement's row from the roadmap to [REQUIREMENTS.md](REQUIREMENTS.md).

**Verified on:** Linux under WSL (the owner, 2026-11-14), and the owner's IBM i 7.3 as NODEJT400 (D22). Windows and macOS not yet.

## In progress

- **R-SQL-87, R-SQL-88** (2026-11-15): Explorer and Current library; Snippets as a tree. Built; **automated**
  (`make test`): the side panel's tests fold and open "Explorer", the top bar's test clicks "Current library MSGSUDTA",
  a snippet's Run shows only while the pointer is on its line, and a starter's right-click menu inserts it and has no
  Edit or Delete. Not yet checked in the running app.

## Implemented

- **R-SQL-89..91** (2026-11-15): a fuller History (Phase 19). **Automated** (`make test`): every run kept with
  its time, how long and how it ended, also a statement run again, the last 500, an error's first line only, the
  history kept before read as entries without a time (core); runs listed newest first under Today and Earlier, the
  search and Failed narrowing the list and "Nothing found.", a click selecting and Enter adding to the script, the
  arrows moving the selection, Insert at the cursor, Clear history asking first, Remove and Clear for one connection
  only (UI). **Checked in the running app** by the owner, Linux under WSL
  (2026-11-15).
- **R-SQL-78..81** (2026-11-15): IBM i Services (Phase 17). **Automated** (`make test`): each view's SQL reads
  the signed-on user's own within 1000 entries with a heading per column, names from the host quoted and a spooled file
  number that is not one refused, rows becoming a viewer's text, new messages counted after the newest seen (core);
  the window's own jobs marked, a failed list saying why, a spooled file opening in a viewer with its text, Find going
  round, and from the tree: System's labels with counts, a click opening the list as a tab, Open showing the viewer, the
  tab closing (UI); the 22 starters read and fill. **On the owner's IBM i** (7.3, read-only as NODEJT400, D22):
  `make probe-sql ARGS="--tools NODEJT400"` ran the three lists, a job log, the newest spooled file's text and every
  starter, the output checked by the owner (2026-11-15). **Checked in the running app** by the owner, Linux under WSL
  (2026-11-15).
- **R-SQL-86** (2026-11-15): no Help menu in Parley SQL. **Automated** (`make test`): the header test finds the
  name, File and the gear and no Help; the gear opens Settings with About. **Checked in the running app** by a
  screenshot, Linux under WSL (2026-11-15).

- **R-SQL-82..85** (2026-11-15): the work area, rearranged (Phase 18). **Automated** (`make test`): the panel
  on the left folds and opens by its buttons and Ctrl+B and keeps its edge with a long name open (R-SQL-82); groups
  named once ignoring case, renamed, moved, libraries in several groups, `æøå` and bad names refused, pinned libraries
  becoming "Pinned" in the settings and following a rename, the IBM i as the tree's root (R-SQL-83); the sections'
  heights within their limits, folding on their own (R-SQL-84); History as a tab putting a statement back (R-SQL-85).
  **Checked in the running app** by the owner, Linux under WSL
  (2026-11-15).
- **R-SQL-74..77** (2026-11-15): the editor, safer (Phase 16). **Automated** (`make test`): typing reaches the
  kept settings in the same frame and Parley SQL saves every 5 seconds (R-SQL-74); the name an error stopped at read
  from SQL0104, SQL0199, SQL0204..SQL0206 and none from `<END-OF-STATEMENT>` or other messages, its place as a whole
  word outside strings and comments, a delimited name with its quotes, a symbol, `æøå`; the underline in the error
  colour and none once stale, a failed run marking the token, the link selecting it and an edit dropping it (R-SQL-75);
  a `WITH`'s names and columns from a column list, `AS`, a trailing name or the column, numbers and expressions left
  out, an unclosed one, `RECURSIVE`, completion offering its names and columns and an alias found before an inner
  table of the same name (R-SQL-76); Format SQL in each style with the default unchanged, and the SQL page's
  Formatting group saved with Save in place (R-SQL-77). **Checked in the running app** by the owner, Linux under WSL (2026-11-15).

- **R-SQL-71..73** (2026-11-15): data transfer (Phase 15, D31), built. **Automated** (`make test`, `make helper`): in
  `parley-core` CSV lines quoted as RFC 4180, a CSV download written batch by batch, a stopped download leaving no file,
  a folder that is not there named in the error, an Excel workbook (checked once by unzipping: a frozen bold header,
  DECIMAL as a number, DATE and TIMESTAMP as Excel dates, a 19-digit BIGINT as text), significant digits, sheet names;
  the CSV reader (quotes, separators and line breaks in quotes, BOM, blank lines, an unclosed quote naming its line),
  the separator found, columns matched by name or position, the columns a table needs, NULL for empty fields, the
  upload's requests and answers; in the helper's self-test the failed row of a batch. In the UI a tab's download
  writing the rows fetched then the rest, Cancel removing the file, a table's download with IBM i headings, a host
  error removing the file, an upload committing and saying how many, a failed row named by its line with SQL7008
  explained, Cancel rolling back, and both dialogs keeping their buttons in place. **Checked in the running app** by
  the owner, Linux under WSL (2026-11-15): downloads to CSV and Excel and an upload into a table. Claude did not run
  them against an IBM i (upload writes, and Claude may only read on the owner's system, D22).

- **R-SQL-70** (2026-11-14): Parley's SQL… starts Parley SQL (D30), built. **Automated** (`make test`): Parley SQL's
  command line read back from what Parley passes (name, host, TLS, CA file, CCSID; æøå in the name), a host alone
  named after itself with TLS, bad options as errors; Parley SQL uses the saved connection to the same host and TLS
  or saves a new one under a free name; in kittest Tools ▸ SQL… starts it once with the session's host and no user,
  says why when it cannot start, and starts nothing without a connection. Run on… (R-SQL-43) and its code are gone.
  Seen in a screenshot: `parley-sql --host pub400.com --name PUB400 --tls --ccsid 277` opens its sign-on. **Checked
  in the running app** by the owner, Linux under WSL (2026-11-14).

- **Checked on the owner's IBM i** (2026-11-14, 7.3, as NODEJT400 without TLS, D22, `sqlprobe`): `--sql` signs on
  through the helper and JTOpen 21.0.7 and reads every type, more rows in blocks, and SQL0204 as an error (R-SQL-3,
  R-SQL-4, R-SQL-11..13); `--tools` runs CL with CPF2103 read, message help from the job log and the message file in
  Danish, the job log, library list and current schema, objects, columns, DDL and parameter markers (R-SQL-30..35,
  R-SQL-40); `--snippets` runs all 13 starters (R-SQL-27); the `SYSPARMS` query of R-SQL-65 reads `QSYS2.QCMDEXC` as
  three signatures, kept as a test. The parts seen only in the window are left to the owner's check.

- **R-SQL-61..68** (2026-11-14): the editor, deeper (Phase 14, D28), built. **Automated** (`make test`): in
  `parley-core` the word under the cursor and its places (quoted, unquoted, æøå, not in strings), Ctrl+D's next place
  with wrapping, trailing blanks trimmed but not inside a string, the routine call and argument at the cursor
  (nested, quoted, keywords and strings left out), the `SYSPARMS` query and its signatures; in the UI the outcomes kept
  by statement text and said on hover, a statement's first line, the ▶ that selects and runs its statement, the
  outcome of a run and of an error, Ctrl+Space and a list that stays open while typing, a snippet chosen from the list,
  keywords kept in a long list, Ctrl+D, Save trimming the file and the script, the word's places on a background, the
  call found and Esc hiding its box. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-60** (2026-11-14): Parley SQL's Shortcuts page, built. **Automated** (`make test`): every action has its
  own keys, which read back and are allowed; only changed actions are kept; typing and text-field keys are refused;
  Ctrl+Shift+F is taken as Format, not Find; a free key replaces or joins at once, a taken one moves only when
  confirmed; in kittest the page changes Go to line to Ctrl+Shift+G by click and press, Parley SQL's settings list
  the page and keep Save in place, and a window given F3 for Find opens the find bar on F3 and not on Ctrl+F.
  **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-52..59** (2026-11-14): the editor, handier (Phase 13), built. **Automated** (`make test`): the text
  changes in `parley_core::sql::edit` (comment, indent, outdent, Tab stops, move and duplicate lines, pairs, the
  matching bracket, keyword case, line starts), with non-ASCII text and edges; in kittest Ctrl+/, Tab, Shift+Tab,
  Alt+↓ and Ctrl+Shift+D reach the text, `(` closes itself only with the setting on, Ctrl+G goes to a line,
  Ctrl+Shift+U and the zoom keys work, the bracket pair is marked and a lone one in the error colour. Fixed on the way:
  the editor kept the keyboard when Automatic moved the results (R-SQL-49), and Enter in the find bar no longer types
  a line break over the match (R-SQL-23). **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-50, R-SQL-51** (2026-11-14): the editor's height and folding the side panel, built. **Automated**
  (`make test`): the editor's height stays within its limits; in kittest dragging the divider below the editor down
  by 80 points moves the results down by as much, dragging it past the bottom leaves the results in the window, and
  the side panel folds (the editor gets wider, History is gone) and opens again. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-49** (2026-11-14): results below or beside the editor, built. **Automated** (`make test`): Automatic is
  below under 1100 points and beside from there; each column keeps its least width; the setting is kept by name and chosen on the Appearance page; in
  kittest the Results tab is under the Run button when Below and to its right when Beside, where the editor fills the column down to the Run buttons, with a divider that stays
  in place, and Automatic shows the divider only in a wide window. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-48** (2026-11-14): the SQL assistant's settings page and providers, built. **Automated** (`make test`):
  each provider has an address and model unless given, a local one needs no key; the Anthropic and OpenAI formats
  build their requests and read tool calls, answers, words, refusals and cut-off replies from hand-built fixtures
  (arguments as text or as an object, a call without an id); HTTP errors are said by provider (Gemini's list form);
  without a needed key or an address nothing is sent; the key comes from the environment, then the keychain, and
  never shows in `Debug` or the settings; in kittest the page turns the assistant on and keeps its place, a key is
  saved to and removed from a keychain in memory, llama.cpp shows its address and the `--jinja` hint, and Parley's
  Add-ons page no longer has the switch. **Seen** (WSL2): the engine, through `examples/askprobe.rs` and a
  made-up library, asked the owner's `llama-server` (b8681, Qwen2.5-Coder-7B Q4_K_M): it listed the tables, read two
  tables' columns and answered with a correct join in four rounds (~40 s). Without `tool_choice: "required"` that model
  wrote its calls as text, so llama.cpp now forces calls. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-45, R-SQL-46, R-SQL-47** (2026-11-14): Parley SQL, built (`make run-sql`). **Automated** (`make test`): in
  kittest, the header has the name, File and Help and a gear at the right end, and no Tools menu; the status bar says
  Offline; the gear opens Settings with Appearance, SQL and About only, and the SQL page keeps the dialog in place;
  a new connection is saved from the start screen with TLS on; the connection dialog keeps its place; Connect without
  the helper says why; an open window fills the main window under the header, says Not signed on, and Disconnect goes
  back to the start screen; a signed-on window has no Run on… although other connections are saved (D26); a connection needs a unique name and a host, keeps no password, and a rename keeps its
  history and pins. **Seen** in `--screenshot`s of the start screen and of a connection's sign-on in the main window
  (WSL2, dark). **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-1** (2026-11-13): host-server records and the port mapper, built (`parley_core::hostserver`,
  `net::hostserver`). **Automated** (`make test`): the sign-on replies recorded from the owner's IBM i and PUB400 read
  into their parameters; a record encodes and decodes to itself (`æøå` in a parameter); broken records (empty, short,
  wrong length, a template or parameter past the end, an unknown server) are errors; the port mapper is asked by name
  and its port used, `-` is "not known", odd replies are errors; with no port mapper the caller falls back. **Seen:**
  `make probe-sql` now uses this code and reads PUB400 as before. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-13** (2026-11-13): JTOpen checked, fetched and shipped, built. JTOpen's licence (IBM Public License 1.0,
  read from `jt400.jar` 21.0.7) allows shipping the unmodified jar with the notices (D20); `scripts/fetch-jt400.sh`
  fetches 21.0.7 from Maven Central and checks its SHA-256; the installers carry `helpers/` (the helper, JTOpen,
  `NOTICE.txt`, `JTOpen-LICENSE.html`) and the release workflow builds them with Java; Settings ▸ About and README.md
  name IBM's copyright; `make verify` builds the helper (JDK in the setup scripts, a Java job in CI). **Seen:** a
  Linux `.deb` holds them in `/usr/lib/parley/helpers`, where the app looks (WSL2). **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-12** (2026-11-13): Java found or explained, built (`sql::java`, Settings ▸ Add-ons). A path named in
  the settings is used alone (a wrong one is named, not passed over), else `JAVA_HOME`, then `PATH`; version 11 or
  later. **Automated:** versions read from `java -version` (1.8, 11, 21, early access); with SQL on, Add-ons shows
  the Java field, what was found and the helper's folder, and hides them again when off; the dialog keeps its place;
  without the helper or with a Java that does not run, Tools ▸ SQL… explains and links to Eclipse Temurin.

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

- **R-SQL-38, R-SQL-39** (2026-11-14): Copy as and binary values, built. Beside Copy, "Copy as" copies the selected
  rows (or all shown) as a Markdown table, `INSERT` statements (into the table the columns come from, else
  `TABLE_NAME`) or JSON; numbers unquoted, nulls kept, text escaped per format. In the row detail a `FOR BIT DATA`
  or `BINARY` value shows as hex in groups of four beside its EBCDIC text, in the session's code page or another
  chosen. **Automated:** each format's escaping, numbers and nulls (`æøå`, `O'Brien`, `|`); the dump's lines and
  unprintable bytes; Copy as takes the selection. **Checked:** JTOpen's type names for both on the owner's 7.3
  (`CHAR () FOR BIT DATA`, `BINARY`), `'Æble'` arriving as `7B829385`. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-40** (2026-11-14): a form for parameter markers, built. A statement with `?` or `:name` markers (not in
  strings, names or comments, never in `CL:`) opens a form first, a field per `?` and per name, each with a NULL
  switch; Run sends it prepared with the values (helper protocol 2, D23), the history keeps the markers, and the
  values come back next time for the same statement in that window; Cancel stops what was queued. **Automated:**
  markers and their slots (`:name` twice, `æøå`, quotes, comments, `12:30`), the request's `params`, the form's
  buttons in place and its values, the window waiting for it. **Checked:** `sqlprobe --tools` on the owner's 7.3
  as NODEJT400: text, a number, a name used twice and null came back right. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-41, R-SQL-42** (2026-11-14): compare and charts, built. "Compare with…" over the result tabs ticks the key
  columns (the first by default) and lists the other results with the same column names; the comparison opens as
  a result of its own: rows only in the first, only in the second, and pairs that differ with the differing values
  marked; how many were the same is said. "Table | Chart" shows the rows shown as bars or lines (`egui_plot` 0.38,
  D23): the first text or date column along the bottom, up to four numeric columns at first, others under
  Columns. **Automated:** matching by key in any row and column order, several keys, duplicates, mismatched
  columns or keys (`æøå`); the comparison tab and its marks; chart data (labels, gaps, row numbers) and a kittest
  switching Table, Chart, Lines. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-43** (2026-11-14): one statement on several systems, built. "Run on…" in the run row lists the other saved
  sessions; ticking one asks for its user and password (used once, D16) and signs on in a helper of its own;
  "Run on all (n)" runs the statement under the cursor here and on each, only if it is one statement that reads
  (`SELECT`, `VALUES`, `WITH`; not `CL:`, not with markers); every system's rows get a tab named after it, its first
  block only (the cursor is closed; "more not fetched" said), errors in Messages under the system's name.
  **Automated:** what reads, sign-on and answers per system with its name, ticking and unticking, the window's
  refusals and tabs, the sign-on dialog's buttons in place. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-44, R-ADD-3** (2026-11-14): the SQL assistant, built. "SQL assistant" in Settings ▸ Add-ons, off by
  default; on, an Ask row over the editor says what is sent and where. A question goes to Claude Opus 5.5
  (Messages API `2026-10-21` over `ureq`, refusal fallback `"default"`, effort `medium`) with the release, current
  schema and library list; the model looks up a library's tables and a table's columns through two tools answered
  from the schema browser's catalog (names, types, headings and texts only, D25) and answers through a third; the
  SQL opens in a new script tab under its explanation as comments, never run. The key comes from
  `ANTHROPIC_API_KEY` when asking and is dropped after. **Automated:** the request (model, tools, fallback, no
  sampling settings), replies (lookups with thinking blocks sent back unchanged, the answer, words only, refusal,
  cut off), HTTP errors said plainly, lookups answered from the cache and waiting while it is read, the script
  text, no key, cancel, the add-on switch. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-37** (2026-11-13): names on hover, built. Hovering a table or view name in the editor (or its alias)
  shows its qualified name, kind, text and up to 25 columns with type and heading, from the browser's cache (read
  when missing). **Automated:** the name and library under the pointer (not in comments, `æøå`), aliases, columns
  of an alias left alone. **Checked:** completion and hover against the owner's 7.3 as NODEJT400 (D22): 9 tables
  after `NODEJT400.`, 4 columns after `x.`, the hover text. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-14..16, R-SQL-18** (2026-11-13): more from results, built (`sql/results`, `sql/grid`). A tab per query
  (up to 10, the oldest closing first, each with its sort, filter and Fetch more); a row's values one under another
  beside the grid (double-click, or Enter on one selected row; Next and Previous); a filter over the rows fetched
  ("n of m rows shown", Copy and Export take the rows shown); column widths dragged at a heading's edge, fitted with
  a double-click; rows at a time and a query timeout in Settings, the timeout as JTOpen's `cancel` mechanism.
  **Automated:** tabs (newest shown, ten kept, fetch into the right tab, closing), filter with `æøå` and a hidden
  selection, the detail stepping and stopping at the ends, widths fitted within limits, limits in each request; drawn:
  a row's number selects it and Enter shows "Row 1 of 2". Two bugs found by these tests: the row's double-click area
  covered its number (no selection), and a focused number took Enter as a click. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-19..23** (2026-11-13): the editor, built (`sql/editor`, `sql::text::highlight` and `format`). Keywords,
  strings, names, numbers and comments in Tortuga's code colours (light and dark); scripts in tabs kept per saved
  session, with a dot when changed and a question before closing one; Open…, Save, Save as… through the file window
  (UTF-8 `.sql`); Format (Ctrl+Shift+F) on the selection or the statement under the cursor; find (Ctrl+F) and
  replace (Ctrl+H), Esc closing the bar before it cancels a statement. **Automated:** highlighting (strings,
  comments and names are never keywords), formatting (clauses, AND/OR, joins, nested selects, kept strings and
  comments, formatting twice changes nothing), find ignoring case for `ÆØÅ`, replace all, scripts kept and restored
  with whether they changed, closing asks only when changed; drawn: Ctrl+F and Esc, Ctrl+T. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-6..10** (2026-11-13): the SQL window, built (`sql`, `sql/grid` in the UI; `sql::text` and
  `sql::runner` in the engine). Its own OS window (egui viewport); sign-on asked each time, the password taken out of
  its field at once; Run (selection or statement under the cursor), Run all, Cancel; results in a grid (numbers
  right-aligned, NULL apart, sort, Fetch more, Copy as tab-separated text, Export… as CSV); Messages; Autocommit,
  Commit, Rollback, and a question when closing with changes not committed; a confirmation before DELETE or UPDATE
  without WHERE, DROP, TRUNCATE and ALTER; history kept per saved session (last 100); a warning line without TLS; the
  SQL add-on, off by default; Tools ▸ SQL… and the `sql` function. **Automated:** statement splitting and the
  guard against strings, names and comments; the runner with the real helper; the window's logic from answers as
  the helper gives them; the sign-in form and the ready window drawn (kittest), the Run button keeping its place.
  **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-11** (2026-11-13): the SQL helper and its protocol, built (`helpers/sql`, `sql::protocol`,
  `sql::helper`). **Automated:** `make helper` compiles it with warnings as errors, runs its self-test (JSON both
  ways, broken JSON, hex, times with seconds) and Rust tests that start it (hello in protocol 1; a statement before
  connecting is an SQL error); the protocol is tested sans-I/O with answers the helper gave. **Seen:** a connect to a
  host where nothing listens answers SQLSTATE 08001 after the 30 s login timeout (5 s in the test), every request
  is answered before the helper ends, and the password is in neither its output nor its errors. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-2** (2026-11-13): sign-on to the sign-on server, built (`hostserver::signon`, `hostserver::substitute`,
  `net::hostserver::sign_on`). The RFC 4777 substitute (DES, SHA-1) is restored from R-NET-7 with its RFC test
  vectors; the request layouts and return codes are Manticore's (D19). **Automated** (`make test`): the exchange of
  attributes is byte for byte the request both hosts answered; the recorded replies give release, seed, password
  level and job; DES and SHA-1 sign-ons carry type 1 or 3, CCSID 13488, the substitute and the user, never the
  password; user names fold Danish letters onto `# @ $`; a password the level cannot carry is refused before
  sending; refusals are sentences (wrong password, the next one disables, expired, …); against a fake sign-on
  server on a local port a sign-on is attempted once and never retried, and the transcript keeps no substitute and
  no seeds. **Real host** (2026-11-13): the owner signed on to their IBM i (7.3, password level 0, DES, no TLS)
  with `make probe-sql … ARGS="--sign-on USER"` at the first attempt; the request is now a fixture the encoder must
  match byte for byte, and the reply (signed on, CCSID 277) another. **Not verified:** SHA-1 against a real host and
  a sign-on over TLS (no PUB400 user to be had); the TLS handshake and exchange of attributes over TLS were seen on
  PUB400 (R-NET-8), and SHA-1 rests on the RFC 4777 test vector. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-NET-8** (2026-11-13): `make probe-sql`, built and run against the owner's IBM i and PUB400 (WSL2): the port
  mapper, the sign-on server's attributes (release, password level), the database host server, DRDA's EXCSAT, Mapepire
  and SSH were each identified or reported unreachable, with no sign-on. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).