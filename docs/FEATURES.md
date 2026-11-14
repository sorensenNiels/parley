# Parley features

> Features that are implemented **and verified**, by requirement ID. The requirements they were built to are in
> [REQUIREMENTS.md](REQUIREMENTS.md); open work and new requirements are in [ROADMAP.md](ROADMAP.md).
> Look an ID up with `grep -n` rather than reading the whole file.
>
> Add an entry only when the requirement is fully met, has automated tests where practical, and has
> been checked in the running app on at least one OS. Note which OSes were checked. Then move the
> requirement's row from the roadmap to [REQUIREMENTS.md](REQUIREMENTS.md).

**Verified on:** Windows, macOS, Linux (desktop) and Linux under WSL2/WSLg, by the owner in the running app (2026-11-12);
Claude's checks ran on Linux/WSL2 against PUB400.COM (IBM i 7.5) and the owner's IBM i (sign-on screen only).

## In progress

- **R-SQL-45, R-SQL-46, R-SQL-47** (2026-11-14): Parley SQL, built (`make run-sql`). **Automated** (`make test`): in
  kittest, the header has the name, File and Help and a gear at the right end, and no Tools menu; the status bar says
  Offline; the gear opens Settings with Appearance, SQL and About only, and the SQL page keeps the dialog in place;
  a new connection is saved from the start screen with TLS on; the connection dialog keeps its place; Connect without
  the helper says why; an open window fills the main window under the header, says Not signed on, and Disconnect goes
  back to the start screen; a signed-on window has no Run on… although other connections are saved (D26); a connection needs a unique name and a host, keeps no password, and a rename keeps its
  history and pins. **Seen** in `--screenshot`s of the start screen and of a connection's sign-on in the main window
  (WSL2, dark). **Left:** the owner signs on and runs statements in Parley SQL on Windows, macOS and Linux, checks
  light and dark and the packaged program, then move them to REQUIREMENTS.md.
- **R-KEY-5** (2026-11-12): editing a macro in Parley, built. **Automated** (`make test`): in kittest, Edit opens the
  file's text, a change is saved, text that does not read names its line and cannot be saved, Cancel leaves the file,
  New macro… saves a macro written by hand, the editor keeps its place; in parley-app the text is written and read
  back with its comments. **Seen** in a `--screenshot` of the editor (WSL2, dark). **Left:** the owner edits and adds
  a macro on Windows, macOS and Linux, then move R-KEY-5 to REQUIREMENTS.md.
- **R-KEY-6** (2026-11-12): Stop beside REC in the status bar, built. **Automated** (`make test`): in kittest, with a
  session connected to a local port, Stop is there only while recording, clicking it ends the recording and asks for
  the name, and the boxes beside it keep their place. **Left:** the owner records a macro and stops it from the
  status bar, in light and dark, then move R-KEY-6 to REQUIREMENTS.md.
- **R-KEY-7** (2026-11-12): importing and exporting macro files through the system's file window (rfd, GTK 3 on
  Linux as Tortuga), built. **Automated** (`make test`): in parley-app, files are copied in as they are (comments,
  `æøå` in the name), a file that does not read is refused with its line, a name already taken is refused, export
  copies the file and a missing macro is an error; in kittest, Import… and Export… report what happened and the list
  shows an imported macro at once; Open folder is gone. `cargo deny` passes with the GTK crates. **Not seen:** the file window itself (it
  needs a click in the running app). **Left:** the owner imports and exports macros on Windows, macOS, Linux and
  WSL, then move R-KEY-7 to REQUIREMENTS.md.
- **R-SQL-1** (2026-11-13): host-server records and the port mapper, built (`parley_core::hostserver`,
  `net::hostserver`). **Automated** (`make test`): the sign-on replies recorded from the owner's IBM i and PUB400 read
  into their parameters; a record encodes and decodes to itself (`æøå` in a parameter); broken records (empty, short,
  wrong length, a template or parameter past the end, an unknown server) are errors; the port mapper is asked by name
  and its port used, `-` is "not known", odd replies are errors; with no port mapper the caller falls back. **Seen:**
  `make probe-sql` now uses this code and reads PUB400 as before. **Left:** nothing to see in the app until R-SQL-6.
- **R-SQL-13** (2026-11-13): JTOpen checked, fetched and shipped, built. JTOpen's licence (IBM Public License 1.0,
  read from `jt400.jar` 21.0.7) allows shipping the unmodified jar with the notices (D20); `scripts/fetch-jt400.sh`
  fetches 21.0.7 from Maven Central and checks its SHA-256; the installers carry `helpers/` (the helper, JTOpen,
  `NOTICE.txt`, `JTOpen-LICENSE.html`) and the release workflow builds them with Java; Settings ▸ About and README.md
  name IBM's copyright; `make verify` builds the helper (JDK in the setup scripts, a Java job in CI). **Seen:** a
  Linux `.deb` holds them in `/usr/lib/parley/helpers`, where the app looks (WSL2). **Left:** the Windows and macOS
  installers, checked by the owner.
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
  keep their buttons in place, a name cannot leave its folder (parley-app). **Left:** `sqlprobe --snippets` on the
  owner's IBM i (each starter on 7.3); the owner tries the panel, in light and dark.
- **R-SQL-30..33** (2026-11-13): IBM i tools and the second connection, built. `CL:` statements run through
  `QSYS2.QCMDEXC` (coloured, kept as written in the history, DLT/CLR/END commands confirmed); Explain on an error
  with a message ID (job log first, then QSQLMSG/QCPFMSG), Job log as a result tab; the current schema and library
  list in the top bar (click sets the schema, Add runs `CL: ADDLIBLE`), read after Connect and after `SET`/`CL:`;
  a second helper signs on only after the first did, its job named in the top bar. **Automated:** parsing, guard,
  colour and format of CL; message IDs and format codes (`æøå`); Explain's fallback; the job context; the second
  connection's states and that a refused first sign-on stops it. `sqlprobe --tools` ran on the owner's 7.3. **Left:**
  the owner tries them in the app (two QZDASOINIT jobs in WRKACTJOB).
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
  offered from the cache, replacing the typed part, and the keys in a kittest. **Left:** the owner tries it.
- **R-SQL-38, R-SQL-39** (2026-11-14): Copy as and binary values, built. Beside Copy, "Copy as" copies the selected
  rows (or all shown) as a Markdown table, `INSERT` statements (into the table the columns come from, else
  `TABLE_NAME`) or JSON; numbers unquoted, nulls kept, text escaped per format. In the row detail a `FOR BIT DATA`
  or `BINARY` value shows as hex in groups of four beside its EBCDIC text, in the session's code page or another
  chosen. **Automated:** each format's escaping, numbers and nulls (`æøå`, `O'Brien`, `|`); the dump's lines and
  unprintable bytes; Copy as takes the selection. **Checked:** JTOpen's type names for both on the owner's 7.3
  (`CHAR () FOR BIT DATA`, `BINARY`), `'Æble'` arriving as `7B829385`. **Left:** the owner tries both in the app.
- **R-SQL-40** (2026-11-14): a form for parameter markers, built. A statement with `?` or `:name` markers (not in
  strings, names or comments, never in `CL:`) opens a form first, a field per `?` and per name, each with a NULL
  switch; Run sends it prepared with the values (helper protocol 2, D23), the history keeps the markers, and the
  values come back next time for the same statement in that window; Cancel stops what was queued. **Automated:**
  markers and their slots (`:name` twice, `æøå`, quotes, comments, `12:30`), the request's `params`, the form's
  buttons in place and its values, the window waiting for it. **Checked:** `sqlprobe --tools` on the owner's 7.3
  as NODEJT400: text, a number, a name used twice and null came back right. **Left:** the owner tries it.
- **R-SQL-41, R-SQL-42** (2026-11-14): compare and charts, built. "Compare with…" over the result tabs ticks the key
  columns (the first by default) and lists the other results with the same column names; the comparison opens as
  a result of its own: rows only in the first, only in the second, and pairs that differ with the differing values
  marked; how many were the same is said. "Table | Chart" shows the rows shown as bars or lines (`egui_plot` 0.38,
  D23): the first text or date column along the bottom, up to four numeric columns at first, others under
  Columns. **Automated:** matching by key in any row and column order, several keys, duplicates, mismatched
  columns or keys (`æøå`); the comparison tab and its marks; chart data (labels, gaps, row numbers) and a kittest
  switching Table, Chart, Lines. **Left:** the owner tries both, in light and dark.
- **R-SQL-43** (2026-11-14): one statement on several systems, built. "Run on…" in the run row lists the other saved
  sessions; ticking one asks for its user and password (used once, D16) and signs on in a helper of its own;
  "Run on all (n)" runs the statement under the cursor here and on each, only if it is one statement that reads
  (`SELECT`, `VALUES`, `WITH`; not `CL:`, not with markers); every system's rows get a tab named after it, its first
  block only (the cursor is closed; "more not fetched" said), errors in Messages under the system's name.
  **Automated:** what reads, sign-on and answers per system with its name, ticking and unticking, the window's
  refusals and tabs, the sign-on dialog's buttons in place. **Left:** the owner tries it with two saved sessions.
- **R-SQL-44, R-ADD-3** (2026-11-14): the SQL assistant, built. "SQL assistant" in Settings ▸ Add-ons, off by
  default; on, an Ask row over the editor says what is sent and where. A question goes to Claude Opus 5.5
  (Messages API `2026-10-21` over `ureq`, refusal fallback `"default"`, effort `medium`) with the release, current
  schema and library list; the model looks up a library's tables and a table's columns through two tools answered
  from the schema browser's catalog (names, types, headings and texts only, D25) and answers through a third; the
  SQL opens in a new script tab under its explanation as comments, never run. The key comes from
  `ANTHROPIC_API_KEY` when asking and is dropped after. **Automated:** the request (model, tools, fallback, no
  sampling settings), replies (lookups with thinking blocks sent back unchanged, the answer, words only, refusal,
  cut off), HTTP errors said plainly, lookups answered from the cache and waiting while it is read, the script
  text, no key, cancel, the add-on switch. **Left:** a real question with the owner's key (it costs a few cents).
- **R-SQL-37** (2026-11-13): names on hover, built. Hovering a table or view name in the editor (or its alias)
  shows its qualified name, kind, text and up to 25 columns with type and heading, from the browser's cache (read
  when missing). **Automated:** the name and library under the pointer (not in comments, `æøå`), aliases, columns
  of an alias left alone. **Checked:** completion and hover against the owner's 7.3 as NODEJT400 (D22): 9 tables
  after `NODEJT400.`, 4 columns after `x.`, the hover text. **Left:** the owner tries both in the app.
- **R-SQL-14..16, R-SQL-18** (2026-11-13): more from results, built (`sql/results`, `sql/grid`). A tab per query
  (up to 10, the oldest closing first, each with its sort, filter and Fetch more); a row's values one under another
  beside the grid (double-click, or Enter on one selected row; Next and Previous); a filter over the rows fetched
  ("n of m rows shown", Copy and Export take the rows shown); column widths dragged at a heading's edge, fitted with
  a double-click; rows at a time and a query timeout in Settings, the timeout as JTOpen's `cancel` mechanism.
  **Automated:** tabs (newest shown, ten kept, fetch into the right tab, closing), filter with `æøå` and a hidden
  selection, the detail stepping and stopping at the ends, widths fitted within limits, limits in each request; drawn:
  a row's number selects it and Enter shows "Row 1 of 2". Two bugs found by these tests: the row's double-click area
  covered its number (no selection), and a focused number took Enter as a click. **Left:** the owner's use.
- **R-SQL-19..23** (2026-11-13): the editor, built (`sql/editor`, `sql::text::highlight` and `format`). Keywords,
  strings, names, numbers and comments in Tortuga's code colours (light and dark); scripts in tabs kept per saved
  session, with a dot when changed and a question before closing one; Open…, Save, Save as… through the file window
  (UTF-8 `.sql`); Format (Ctrl+Shift+F) on the selection or the statement under the cursor; find (Ctrl+F) and
  replace (Ctrl+H), Esc closing the bar before it cancels a statement. **Automated:** highlighting (strings,
  comments and names are never keywords), formatting (clauses, AND/OR, joins, nested selects, kept strings and
  comments, formatting twice changes nothing), find ignoring case for `ÆØÅ`, replace all, scripts kept and restored
  with whether they changed, closing asks only when changed; drawn: Ctrl+F and Esc, Ctrl+T. **Left:** the owner's use.
- **R-SQL-6..10** (2026-11-13): the SQL window, built (`sql`, `sql/grid` in the UI; `sql::text` and
  `sql::runner` in the engine). Its own OS window (egui viewport); sign-on asked each time, the password taken out of
  its field at once; Run (selection or statement under the cursor), Run all, Cancel; results in a grid (numbers
  right-aligned, NULL apart, sort, Fetch more, Copy as tab-separated text, Export… as CSV); Messages; Autocommit,
  Commit, Rollback, and a question when closing with changes not committed; a confirmation before DELETE or UPDATE
  without WHERE, DROP, TRUNCATE and ALTER; history kept per saved session (last 100); a warning line without TLS; the
  SQL add-on, off by default; Tools ▸ SQL… and the `sql` function. **Automated:** statement splitting and the
  guard against strings, names and comments; the runner with the real helper; the window's logic from answers as
  the helper gives them; the sign-in form and the ready window drawn (kittest), the Run button keeping its place.
  **Left:** the owner's use against their IBM i on each OS, then these requirements to REQUIREMENTS.md.
- **R-SQL-11** (2026-11-13): the SQL helper and its protocol, built (`helpers/sql`, `sql::protocol`,
  `sql::helper`). **Automated:** `make helper` compiles it with warnings as errors, runs its self-test (JSON both
  ways, broken JSON, hex, times with seconds) and Rust tests that start it (hello in protocol 1; a statement before
  connecting is an SQL error); the protocol is tested sans-I/O with answers the helper gave. **Seen:** a connect to a
  host where nothing listens answers SQLSTATE 08001 after the 30 s login timeout (5 s in the test), every request
  is answered before the helper ends, and the password is in neither its output nor its errors. **Left:** the
  owner's run of `make probe-sql HOST=… ARGS="--sql USER"` against their IBM i (R-SQL-3, R-SQL-4).
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
  PUB400 (R-NET-8), and SHA-1 rests on the RFC 4777 test vector. **Left:** the SQL window (R-SQL-6) for the app.
- **R-NET-8** (2026-11-13): `make probe-sql`, built and run against the owner's IBM i and PUB400 (WSL2): the port
  mapper, the sign-on server's attributes (release, password level), the database host server, DRDA's EXCSAT, Mapepire
  and SSH were each identified or reported unreachable, with no sign-on. **Left:** the owner's review, then move
  R-NET-8 to REQUIREMENTS.md.
- **R-KEY-8** (2026-11-13): a key typed in a protected area is ignored, with no error to Reset (D18), built.
  **Automated** (`make test`): in parley-core, a character (also `æ`), Delete, Backspace, Erase EOF and Field Exit at a
  protected position leave the keyboard unlocked and the screen unchanged, and Tab then goes to the next input field.
  **Left:** the owner types outside a field on a real screen on each OS.
- **R-UI-13** (2026-11-12): icons from the bundled Phosphor font (D17), built. **Automated** (`make test`): every
  icon in `icons::ALL` is in the icon font, which is the only font in its family and stays out of the text families
  (Inter has glyphs of its own at some of the same codepoints). **Seen** in `--screenshot`s on PUB400's sign-on screen
  (WSL2, light and dark). **Left:** the owner checks the menus, the Keys menu and the status bar on Windows, macOS and Linux.
- **R-UI-14** (2026-11-12): the interactive status bar, built. **Automated** (`make test`): the state, TLS, keyboard
  map, code page, size and cursor show, INS only in insert mode; a narrow bar drops size and code page but keeps TLS
  and the cursor; the clock times the host from an AID key to the unlocked keyboard; the lock is noticed in the frame Enter
  goes down, the hourglass has its own slot beside the check, which stays, and Keys does not move; the REC pill and Stop keep the
  state pill in place (R-KEY-6); choosing a map in the chip saves it to the session. **Seen** in `--screenshot`s (WSL2,
  light and dark: Ready, 125 ms, 24×80, CCSID 37, Default, 5, 25). **Left:** Connect again after a disconnect, the map
  menu and the hovers, checked by the owner in the running app on each OS.
- **R-UI-15** (2026-11-12): one header with the session pill, built. **Automated** (`make test`): menus and Help are
  there before connecting; the pill shows host:port and its menu disconnects; Connect to another session while
  connected closes the open one first. **Seen** in `--screenshot`s (WSL2, light and dark). **Left:** the owner opens
  the menus (icons, keys) and switches sessions from the pill on each OS.
- **R-UI-16** (2026-11-12, changed 2026-11-13): the 5250 keys in a Keys menu in the status bar, built. **Automated**
  (`make test`): Attention and F15 are sent from the menu, the menu lists System Request and F24 while connected, and
  there is no Keys button before connecting. **Seen** in a `--screenshot` (WSL2, light): Ready, Keys, then the chips.
  **Not seen:** the open menu. **Left:** the owner uses the menu on each OS.
- **R-UI-17** (2026-11-12): the keyboard shortcut sheet, built. **Automated** (`make test`): the Default map's keys
  under Settings' headings, Ctrl+/ opens it from the screen and it stays open the next frame, Esc closes it (not on
  the frame it opens), both close buttons, Change keys… opens Settings ▸ Keyboard, the filter, and the dialog keeps
  its place. **Not seen:** the sheet itself in a screenshot. **Left:** the owner opens it with Ctrl+/ (also on a
  Danish keyboard, where / is Shift+7) on each OS.

## Implemented

### Phase 1: connect and show a screen (2026-11-12) — R-NET-1..3, R-DS-1..4, R-EBC-1, R-KEY-1, R-UI-1..4, R-NF-1

Verified by the owner in the running app on Windows, macOS, Linux and WSL2 (2026-11-12).

How it was checked:
- **Automated** (`make test`): telnet negotiation and framing; record header; every command and order in R-DS-1 and
  R-DS-4 with hand-built records; field rules and keys; the exact bytes of Enter, Read Immediate, the query reply and
  Save Screen; CCSID 37/277/1140/1142 tables (generated from iconv by `scripts/gen-ebcdic.py`); 6000 random records
  without a panic; a whole session over TCP against a fake host (`tests/connection.rs`); the terminal widget and form
  in egui_kittest (typing, Enter, a click moving the cursor, the screen exposed to assistive technology).
- **Against PUB400.COM** (`cargo run -p parley-core --example probe`): TN5250 negotiation on port 23 and TLS on 992
  (certificate checked against the OS store); the sign-on screen shown with both input fields and the cursor where the
  host put it; CCSID 277 (`KBDTYPE` DMB) and the 3477-FC terminal type accepted; Help answered by a Write Error Code
  ("Function key not allowed."), Reset putting the line back; Enter with empty fields answered by
  "CPF1296 - Sign-on information required." (the host accepted Parley's input record).
- **Running app** (`parley pub400.com --screenshot FILE`): the sign-on screen drawn with the host's colours, reverse
  image and underlines, the block cursor in the user field, "Ready" in the status line.


### Phase 2: saved sessions, auto sign-on and the look (2026-11-12) — R-NET-4, R-NET-5, R-UI-5..12

Verified by the owner in the running app on Windows, macOS, Linux and WSL2 (2026-11-12), except auto sign-on (R-NET-4) on
Linux. *R-NET-4 was removed afterwards with the saved passwords (R-NET-7, D16); its checks below are history.*

How it was checked:
- **Automated**: the DES and SHA-1 password substitutes against RFC 4777's test vectors (sections 5.1 and 5.2) and
  the RFC's example negotiation byte for byte; no credentials without a host seed; a truncated or escaped seed;
  typed sign-on sends the same record as an operator typing, only once; keep-alive bytes; in egui_kittest, the start
  screen, saving a session with its password in a (memory) keychain only, required name and host, delete and rename
  moving the password, one session marked for program start; the bundled terminal fonts measured as monospace (and
  Inter not), a missing font falling back, choosing a font in the settings, OK keeping it and Cancel restoring; the
  close button (×) of the settings, the session dialog and the shortcuts panel (R-UI-7); settings pages, Restore
  defaults per page, and the dialog keeping its place from frame to frame (R-UI-8); R-UI-9: Inter / Inter SemiBold /
  JetBrains Mono families, thin floating scroll bars, every symbol the UI draws present in the bundled fonts, the
  session dialog keeping its place. Screenshots (Linux/WSLg): start screen, PUB400 connected, session dialog and
  settings dialog in dark, settings dialog in light. R-UI-10: the name (and icon) in the menu bar and Help at its right
  edge, in kittest and a screenshot. R-UI-11/12: version comparison, GitHub's answer read from a fixture (newer, same,
  404, error, garbage), unsafe download URLs left out, the `curl` command, the request naming only the app (against a
  local fake server); in kittest with a fake checker: the menu bar link, no check when turned off, Check for updates
  saying what it found, a failure silent at start-up but shown on Check now, the download command menu on Linux, and
  no checker meaning no check. Against GitHub (once, by hand): the About page said "up to date" for 0.2.1, and a check
  as 0.1.0 found 0.2.0 with the `.deb` and AppImage `curl` commands.
- **Running app** (Linux/WSLg screenshots): the start screen lists the session migrated from the 0.1 settings; the
  session dialog with every option. WSL has no Secret Service, so the dialog says the keychain is unavailable.
  R-UI-6: the start screen, session dialog (two columns, fits a 1000×640 window), settings dialog with the font
  preview, and PUB400's sign-on screen in IBM Plex Mono with the roomier bars; `make package` puts the OFL files in
  the .deb (66 ms to list this machine's installed monospace fonts).

### Phase 3: keyboard maps and macros (2026-11-12) — R-DS-5, R-KEY-2..4, R-ADD-1..2, R-NET-7

**R-DS-5** (2026-11-12): verified by the owner on their IBM i in the running app on Windows, macOS, Linux and WSL2
(signed on; a spooled file shown at 27×132 in a wide session).

How it was checked:
- **Against the manual**: IBM's 5494 Remote Control Unit Functions Reference (SC30-3533-02, chapter 12), cited next to
  the code. Start of Header matched (clears the format table; error row in byte 4) except that length 0 is now an
  error. Nulls as blanks and the negative zone matched; Read MDT Alternate now strips trailing nulls and sends only the
  address for an all-null field. The query reply sent a 2-byte model where the manual has 3, so every later byte was
  one early and the screen sizes landed in capability byte 0; fixed, with 256 input fields and the colour bit.
- **Automated** (`make test`): a test per fix in `tests/session.rs` (`*_r_ds_5`), each failing without its fix.
- **Against PUB400.COM**: the same sign-on screen at 24×80 and 27×132 before and after (it sends no Query before
  sign-on).

**R-KEY-3, R-KEY-4** (2026-11-12): recording and running macros; D14 for the files. **Automated**
(`make test`): the file format (round trip, every key name, comments, errors naming the line); recording (text, keys,
`wait` after Enter, the password left out, refused keys and their Reset left out, clicks left out); playing (waiting
for the host, asking for the password, Reset stopping it, the 30 s time-out, stopping at an error, starting on
the first screen); the macro folder (save, list, rename, delete, a bad file listed with its line); in kittest the save,
password and Macros dialogs (each keeping its place), Run macro in the session dialog, and a renamed session taking
its macros along. **Against PUB400.COM** (`probe --macro`): a macro pressing Help waited for the host and stopped at
its "Function key not allowed" with the error on screen. **In the running app** (WSL2, the owner): a recorded
sign-on macro as the session's Run macro, without auto sign-on, signs on and asks for the password (after a fix: it
used to wait for a screen past sign-on that it brings itself). **Verified** by the owner in the running app on Windows, macOS and Linux (2026-11-12).

**R-ADD-1, R-ADD-2** (2026-11-12): Settings ▸ Add-ons, with Macros as the first add-on (on by default);
D15. **Automated** (`make test`): each add-on starts with its default, only a change from it is kept, unknown ids
still load (`addons.rs`); in kittest the Add-ons page turning Macros off on Save, Cancel leaving it as it was, the
dialog keeping its place; while off no macro items in Tools, no Run macro in the session dialog, the macro folder not
read (and read again when turned on), a session's Run macro kept but not run on connect; turned off while connected,
a recording asks for its name and a running macro stops (against a local port that never answers). **Verified** by the owner in the running app on Windows, macOS and Linux (2026-11-12).

**R-KEY-2** (2026-11-12): keyboard maps. **Automated** (`make test`): the Default map gives R-KEY-1's keys
(Ctrl alone and Ctrl+R Reset, Ctrl+Plus with or without Shift); a map of one's own changes them, runs a macro, and
can drop Ctrl alone; keys read back as written, unknown bindings are kept and ignored; keys that type a character
without Ctrl, or with Ctrl+Alt (AltGr), are refused; a deleted map falls back to Default; renaming a macro moves its
keys. In kittest: the Default map has nothing to change; a copy gets a new key by clicking + and pressing it; a key
another function has moves only after "Use it for …"; renaming a map moves the sessions that use it; deleting it
gives them Default; the session dialog chooses the map; the Keyboard page keeps the dialog in place. **Seen** in a
`--screenshot` of the page (WSL2, dark). **Verified** by the owner in the running app on Windows, macOS and Linux (2026-11-12).

**R-NET-7** (2026-11-12): auto sign-on and saved passwords removed (D16; R-NET-4 and the planned R-NET-6 go).
**Automated** (`make test`): telnet's answer to RFC 4777's request with a host seed holds only the device name; a
session on a sign-on screen sends nothing until a key; a macro's `password` step always asks; the session dialog
asks for no user or password; `Password`'s `Debug` hides it. The RFC 4777 code and the `des`, `sha1` and
`getrandom` dependencies are gone. **Verified** by the owner in the running app on Windows, macOS and Linux (2026-11-12).
