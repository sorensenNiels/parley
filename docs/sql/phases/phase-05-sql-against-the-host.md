# Phase 5 — SQL against the host

> Finished phase of Parley SQL. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

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

**Status:** Done, verified (2026-11-14, Linux under WSL): the SQL window over a Java helper with IBM's JTOpen (D20): R-SQL-3..13. Built first: R-NET-8, R-SQL-1 and R-SQL-2 (the native sign-on, kept for the probe), verified on the owner's IBM i.

## Requirements

| ID | Requirement |
|---|---|
| R-SQL-13 | **JTOpen, checked and shipped** (2026-11-13; D20). Before anything is built on it: JTOpen's licence (IBM Public License 1.0) is read for shipping the unmodified `jt400.jar` with an MIT/Apache app, and the result recorded in D20; if it does not allow that, the owner decides again. Then a pinned JTOpen release from Maven Central, its SHA-256 checked by the build, and Parley's helper jar go into the installers beside the app (cargo-packager resources, docs/RELEASING.md), with the IPL text beside the font licences and a line in README.md. Building the helper needs a JDK (11 or later) on the build machines and in CI; `make verify` compiles it. |
| R-SQL-12 | **Java, found or explained** (2026-11-13; D20). The SQL add-on runs the helper with Java 11 or later: the path set in Settings ▸ Add-ons, else `JAVA_HOME`, else `java` on the `PATH`. Settings ▸ Add-ons shows which Java was found and its version. Without one, Tools ▸ SQL… opens a short explanation with a link to a free Java (Eclipse Temurin) instead of failing; nothing else in Parley needs Java. |
| R-SQL-11 | **The SQL helper and its protocol** (2026-11-13; D20). A small Java program (`helpers/sql`, Parley's own code) that Parley starts when an SQL window opens and stops when it closes; it also ends when its input closes, so it never outlives Parley. They talk in JSON, one object per line on stdin/stdout, with a protocol version in the first exchange: connect, run, fetch more, cancel, commit, rollback, autocommit on/off, close; each answer is a result or an error with SQLSTATE, SQLCODE and the message text. The user and password go only through stdin, never on the command line or in the environment, and never reach a log or an error. The helper's stderr is kept only to show when it fails. Parley's side (`parley-core`, sans-I/O as D4) is tested against a fake helper; the helper is checked against the owner's IBM i. |
| R-SQL-3 | **A database connection** (2026-11-13; D20, was D19). The helper connects with JTOpen's JDBC driver to the session's host, over TLS when the session's TLS is on (Java's trust store plus the session's CA file, turned into a trust store for the helper; how this compares with D6's operating-system trust store is settled while building), with ISO dates and times, `*SQL` naming and the user's default library list, and reports the server's job name. One connection per SQL window. A refused sign-on is reported as JTOpen words it, and never retried. |
| R-SQL-4 | **Running statements and reading results** (2026-11-13; D20, was D19). The helper prepares and runs a statement; for a query it fetches rows in blocks of 500 up to the window's limit and sends them as text: DECIMAL and NUMERIC exactly (no floating point), dates, times and timestamps in ISO form, binary as hex, NULL apart from an empty string, LOBs as "(LOB)"; each column with its name, type, length, scale and whether it may be NULL. A statement that is not a query gives its row count. A running statement can be cancelled. Tested with `æøå` in CCSID 277 data and a value at each type's limits on the owner's IBM i. |
| R-SQL-5 | **Commit control** (2026-11-13). Autocommit is on when the window opens. With it off, statements run under commit control (`*CHG`) and Commit and Rollback are offered; closing the window or disconnecting with uncommitted changes asks Commit, Roll back or Cancel. |
| R-SQL-6 | **The SQL window** (2026-11-13). Tools ▸ SQL… and a bindable function `sql` open it for the connected session's host, as a window of its own beside the main one (an egui viewport), so the 5250 screen stays usable (the owner, 2026-11-13). Opening asks for the user and password (Parley keeps none, D16); the user defaults to the last one typed for this session in this run of Parley. The window's title names the session and the database job; without TLS a warning line says the statements and results travel unencrypted, and nothing more is asked (the owner, 2026-11-13). Disconnecting the session closes the window (asking first under R-SQL-5). |
| R-SQL-7 | **Writing and running** (2026-11-13). A monospace editor; Run (Ctrl+Enter) runs the selection, or the statement under the cursor (statements end with `;`); Run all (Ctrl+Shift+Enter) runs them in order and stops at the first error; Cancel (Esc) stops a running statement (R-SQL-4). A history of the statements run (the last 100 per saved session, kept in the settings between runs of Parley (the owner, 2026-11-13), never with results) is listed beside the editor; one click puts a statement back in the editor. *Changed 2026-11-15 by R-SQL-85 (the history is a tab under the editor).* *Changed 2026-11-15 by R-SQL-89 (every run kept, the last 500).* |
| R-SQL-8 | **Results** (2026-11-13). Each query's rows in a grid: column names with their types on hover, NULL shown apart from an empty string, numbers right-aligned, dates in ISO form. At most 1000 rows are fetched at first ("1000 rows shown · Fetch more"); a click on a column heading sorts the rows fetched. Copy a cell, the selected rows or all rows as tab-separated text (pastes as columns in a spreadsheet); Export… writes CSV (UTF-8) through the file window. Row counts, SQLSTATE and messages go to a Messages tab, an error also as a red line under the editor. |
| R-SQL-9 | **A guard for changes** (2026-11-13). Before running `DELETE` or `UPDATE` without `WHERE`, or `DROP`, `TRUNCATE` or `ALTER`, the window names the statement and asks to confirm. The check reads the statement's first words and the presence of `WHERE` outside strings and comments; it only guards against slips, and says so. |
| R-SQL-10 | **SQL is an add-on** (2026-11-13; D15). The SQL window is the add-on "SQL" in Settings ▸ Add-ons, off by default (the owner, 2026-11-13: only those who want SQL see it and need Java); while it is off, Tools ▸ SQL… and the `sql` function are not there, Java is never looked for and no helper runs (D20). |
| R-SQL-1 | **Host-server framing and the port mapper** (2026-11-13; D19; built, kept for the probe and later host-server features, not used by the SQL window, D20). `parley-core` builds and reads the host servers' records: the 20-byte header (length, header id, server id, CS instance, correlation, template length, request/reply id), the template, and the parameters as length/code point/data. The port mapper (449) is asked where `as-signon` and `as-database` listen (`-s` with TLS); when it cannot be reached, the usual ports are used (8476/8471, 9476/9471). A record that is short, longer than its data, or has a parameter running past its end is an error, never a panic. Tested with the replies `make probe-sql` recorded (R-NET-8). |
| R-SQL-2 | **Sign-on** (2026-11-13; D19; built and verified with DES on the owner's IBM i, kept for the probe, not used by the SQL window, D20). Exchange of attributes, then the sign-on request with a password substitute of the kind the host's password level asks for: levels 0–1 (DES) and 2–3 (SHA); another level is an error that names it. The password is a `Password` (hidden `Debug`), is turned into the substitute and dropped, and never reaches an error, a log or a trace. The host's answers are named: wrong password, user disabled, password expired (change it in a 5250 session), user unknown. The algorithms come from public documentation; where none is found, that is reported to the owner before anything is built from other sources. |
| R-NET-9 | **Record a sign-on and a query** (2026-11-13; for R-SQL-2 tests. *The sign-on part is built and was run on the owner's IBM i; the query part is dropped with D20, which needs no recorded database traffic.*) `make probe-sql HOST=… ARGS="--sign-on USER [--tls]"` asks for the password on the terminal (not echoed), signs on, runs `VALUES CURRENT SERVER` and a small `SELECT` from `QSYS2.SYSTABLES`, prints the results, and writes what passed both ways to a file with the password substitute and seeds blanked. Only the owner runs it, on systems they have an account on; Claude never does (CLAUDE.md). |
| R-NET-8 | **Probe a host's SQL routes** (2026-11-13, asked by the owner; for IDEA-13). `make probe-sql HOST=…` (`parley-core/examples/sqlprobe.rs`) asks, without signing on and without sending a user or password: the port mapper (449) where each host server listens; the sign-on server (8476, 9476 with TLS) its exchange of attributes (IBM i release, data stream level, password level); the database host server (8471, 9471) whether it takes a connection and its TLS; DRDA (446, 447 with TLS) EXCSAT (server class, release, manager levels); Mapepire (8076) a WebSocket upgrade without credentials; SSH (22, 2222) its banner. Certificates are checked as Parley checks them (D6); when that fails, the reason is printed and the service is identified over an unchecked connection on which nothing secret is sent. Replies are printed as hex too. A development tool, not part of the app; the route chosen gets its protocol code in `parley-core` with recorded fixtures. |

## Verification

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
