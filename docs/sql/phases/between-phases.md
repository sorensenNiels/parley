# Between phases

> Requirements of Parley SQL built and verified outside any phase: none of the roadmap's phases names them. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

Asked for on their own, between phases; see each requirement's date.

## Requirements

| ID | Requirement |
|---|---|
| R-SQL-69 | **Parley SQL's settings in Parley SQL only** (2026-11-14; asked by the owner; D29. *Changes R-SQL-12, R-SQL-48 and R-SQL-49.*) Parley's Settings has none of Parley SQL's settings: no SQL assistant page, no Java, rows at a time, query timeout, shared snippets, Java found or SQL helper on Add-ons, and no "SQL window" group on Appearance. Its Add-ons page keeps the SQL add-on switch and, while it is on, says that the SQL window uses Parley SQL's settings and where to change them. Each time Parley opens the SQL window it reads Parley SQL's saved settings and uses their Java, rows at a time, query timeout, results placement, bracket closing, shared snippets and SQL assistant (switch, provider, model, address); it never changes them. Without saved ones (Parley SQL never run, or a file that cannot be read) it uses the defaults. Parley keeps its own history, scripts, pinned libraries and column headings per session, and its SQL window keeps the default shortcuts (R-SQL-60). |
| R-SQL-70 | **Parley's SQL… starts Parley SQL** (2026-11-14; asked by the owner; D30. *Changes R-SQL-6, R-SQL-10, R-SQL-43 and R-SQL-69:* Parley has no SQL window of its own.) While the SQL add-on is on and a session is connected, Tools ▸ SQL… (and its key) starts the program Parley SQL (R-SQL-45), installed beside Parley, exactly as when it is started on its own: its own window, menus, settings, connections and storage. Parley passes the session's name, host, TLS, CA file and CCSID on Parley SQL's command line (`--host`, `--name`, `--tls`/`--no-tls`, `--ca-file`, `--ccsid`); Parley SQL uses its saved connection with the same host and TLS (keeping its user), or saves a new one under the session's name (with the host added when the name is taken), and opens its sign-on at once. Parley knows no user and passes no password (D16). Each SQL… starts another Parley SQL. When Parley SQL is not beside Parley or cannot start, Parley says so in a notice. Parley's Add-ons page says that SQL… starts Parley SQL, whose settings are there; Parley no longer reads Parley SQL's settings (R-SQL-69). Run on… (R-SQL-43) and the SQL history, scripts and pinned libraries Parley kept per session go with Parley's window; Parley SQL keeps its own per connection. `make run` builds Parley SQL too. |
| R-SQL-86 | **No Help menu in Parley SQL** (2026-11-15, asked by the owner; as Parley's R-UI-18. *Changes R-SQL-45:* the Help menu goes.) Parley SQL's header has the icon and name and File only; what Help held, the version and Check for updates, is on Settings ▸ About (R-SQL-47). |
| R-SQL-87 | **Explorer and Current library** (2026-11-15; the owner, D34). The side panel's upper section, which holds the IBM i, its System node, the library list and the library groups (R-SQL-83), is headed "Explorer", not "Libraries"; its buttons say so ("Fold Explorer", "Show Explorer and Snippets"). The top bar's menu (R-SQL-32) reads "Current library MYLIB", not "Schema MYLIB"; its hover says it is where unqualified tables are looked for, the SQL current schema (`SET SCHEMA`), and a library in its list is made "the current library". What runs on the host is unchanged. *Changes R-SQL-32, R-SQL-82, R-SQL-83 and R-SQL-84 (their names only).* |
| R-SQL-88 | **Snippets as a tree** (2026-11-15; the owner, D34). The Snippets section (R-SQL-84) shows Mine, Shared and Did you know as groups that fold like the Explorer's library groups, each header with its count; a group with nothing to show, or nothing matching the search, is left out, and while a search is typed every group is open. Each snippet is one line: its title, and its tags in muted text on the right; while the pointer is on the line a Run button takes the tags' place. Double-click inserts it and the hover shows its about and SQL, as before; its right-click menu has Insert and Run, and for mine and shared ones Edit… and Delete… (asking first). *Changes R-SQL-25 (Run, Edit and Delete were buttons on every line, tags a line of their own).* |
| R-SQL-101 | **Open a finished download** (2026-11-16, asked by the owner). When a download or backup has written its file, the Messages tab shows it in a card with **Open** (the file in the program the system uses for it: Excel for `.xlsx` and `.csv`, an editor for `.sql`), **Show in folder** (the folder it is in), and a close button; a file that cannot be opened says why in the messages. The card goes when closed or when the next download starts. Opening is the launcher's (`opener`, no platform code in the UI crates). |
| R-SQL-102 | **System naming by default** (2026-11-17; asked by the owner after a new install showed a library's tables and views only once switched to System, D52. *Changes R-SQL-97:* System is the default, not SQL.) A new connection's "Naming" starts at System (`LIBRARY/FILE` and the library list), and so do connections saved before Phase 25 and ones opened without a saved connection; a connection saved with SQL keeps it. |
| R-SQL-103 | **No console windows on Windows** (2026-11-17; asked by the owner: Parley SQL opened two extra windows, one per `java.exe`). The SQL helper (the SQL job and the catalog job) and the `java -version` check start without a console window (Win32 `CREATE_NO_WINDOW`); their pipes work as before. Checked on Windows: starting Parley SQL and signing on shows no window but its own. |
| R-SQL-104 | **Drag a library group into place** (2026-11-17; asked by the owner. *Adds to R-SQL-83*, whose Move up and Move down stay.) In the Explorer, a group is dragged by its menu button, right of its name: while dragged it takes the place of the group under the pointer, the others keeping their order, and the pointer shows a grabbing hand; a click on the button still opens the menu. A library open in a group stays open when the group moves. |

## Verification

- **R-SQL-86** (2026-11-15): no Help menu in Parley SQL. **Automated** (`make test`): the header test finds the
  name, File and the gear and no Help; the gear opens Settings with About. **Checked in the running app** by a
  screenshot, Linux under WSL (2026-11-15).

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

### Checked 2026-11-16 — R-SQL-87, R-SQL-88, R-SQL-93..96, Phase 25 (R-SQL-97..100), R-SQL-101

- **R-SQL-101** (2026-11-16): Open and Show in folder for a finished download. **Automated** (`make test`): a
  finished tab download keeps its file (`æøå.csv`); the Messages tab's card opens it and its folder through the
  launcher and goes when closed. **Checked in the running app** by the owner on Windows, macOS, Linux and Linux under WSL (2026-11-16).

- **R-SQL-87, R-SQL-88** (2026-11-15): Explorer and Current library; Snippets as a tree. Built; **automated**
  (`make test`): the side panel's tests fold and open "Explorer", the top bar's test clicks "Current library MSGSUDTA",
  a snippet's Run shows only while the pointer is on its line, and a starter's right-click menu inserts it and has no
  Edit or Delete. Not yet checked in the running app. **Checked in the running app** by the owner on Windows, macOS, Linux and Linux under WSL (2026-11-16).

### Checked 2026-11-17 — R-SQL-102..104

- **R-SQL-102** (2026-11-17): System naming by default. **Automated** (`make test`): the default options send
  `naming system`; a connection saved with SQL naming keeps it; the connection dialog saves SQL when chosen. **Checked in the running app** by the owner, who marked it done (2026-11-17; the operating system was not named).

- **R-SQL-103** (2026-11-17): no console windows on Windows. The SQL helper and the `java -version` check start
  with `CREATE_NO_WINDOW`; the Windows branch is not compiled on Linux, so it has no automated test. **Checked in the running app** by the owner, who marked it done (2026-11-17; the operating system was not named).

- **R-SQL-104** (2026-11-17): a library group is dragged into place. **Automated** (`make test`): a group moves to
  any place with the others in order; dragging "Udvikling æøå"'s menu button onto "Test" moves it, opens no menu,
  and a click still opens it (the test failed while the drag handle was keyed by place). **Checked in the running app** by the owner, who marked it done (2026-11-17; the operating system was not named).
