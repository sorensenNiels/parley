# Phase 11 — Parley SQL

> Finished phase of Parley SQL. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

From IDEA-28 (the owner, 2026-11-14): the SQL window offered on its own as **Parley SQL**, for people who need SQL
on IBM i but no 5250 session (R-SQL-45), still in this repository and still Parley's SQL add-on. Its main window has
a header with a gear for Settings at the right and a status bar, in Parley's look; saved connections replace saved
sessions (R-SQL-46); Settings shows only the pages that apply (R-SQL-47). The choices are D26.

**Status:** Done, verified (2026-11-14, Linux under WSL): the SQL window as a program of its own, `parley-sql` (R-SQL-45), with saved connections (R-SQL-46) and its own settings pages (R-SQL-47); D26. Parley's SQL add-on is unchanged.

## Requirements

| ID | Requirement |
|---|---|
| R-SQL-45 | **Parley SQL, the SQL window on its own** (2026-11-14; D26). A second program, `parley-sql`, built from this repository beside `parley` and shipped in the same installers, opens the SQL window of R-SQL-6 in its main window (title "Parley SQL", or "*connection* – Parley SQL"), with no 5250 session. Above it a header in Parley's look (R-UI-15): the icon and "Parley SQL", File (New connection…, Edit connections…, Connect to ▸, Disconnect, Exit) and Help (Check for updates…, the version); on the right a gear that opens Settings, and while a window is open a pill with the connection's name, host, a lock for TLS and a dot for where it is (not signed on, signing on, ready), whose menu edits it or disconnects. Below, a status bar: Offline and why the last window closed; otherwise not signed on, signing on, Ready or Running…, the last error, and on the right the job (user and release on hover), Autocommit off or Uncommitted changes, TLS or Not encrypted, and the CCSID. The window's Close is Disconnect and leads back to the start screen; closing the program with changes not committed asks first (R-SQL-5). Everything else is the add-on's window: the same helper, history, scripts, snippets (the same folder as Parley's), schema browser and assistant; but not Run on… (R-SQL-43): Parley SQL works on one connection at a time (D26). Without the helper or Java it says why, as R-SQL-12. `parley-sql CONNECTION` opens that connection's sign-on. |
| R-SQL-46 | **Saved connections in Parley SQL** (2026-11-14; D26). The start screen lists the saved connections as cards (name; host, TLS, user) with Edit… and Connect, and New connection…. The connection dialog has Name, Host, a TLS switch, the CA certificate (with TLS), the Code page and the User; Delete and Help on the left, Connect (filled), Save and Cancel on the right; a name must be given and unique, and a host given. A new connection has TLS on (R-P-3); without TLS the dialog warns that statements, results and the password travel as plain text. No password is kept (D16): the user signed on with last is kept and filled in. Renaming a connection keeps its history, scripts and pinned libraries. The connections are kept in Parley SQL's own settings, apart from Parley's saved sessions. |
| R-SQL-47 | **Parley SQL's settings** (2026-11-14; D26). The gear opens Parley's settings dialog with the pages that apply: Appearance (theme, text size, density), SQL (the SQL assistant switch, R-ADD-3, which stays off until turned on; Java, rows at a time, query timeout, shared snippets, and what was found), and About (version, updates, JTOpen's notice). SQL itself is always on; there is no Terminal, Keyboard or Add-ons page. Restore defaults works per page, Cancel puts the old settings back. |

## Verification

- **R-SQL-45, R-SQL-46, R-SQL-47** (2026-11-14): Parley SQL, built (`make run-sql`). **Automated** (`make test`): in
  kittest, the header has the name, File and Help and a gear at the right end, and no Tools menu; the status bar says
  Offline; the gear opens Settings with Appearance, SQL and About only, and the SQL page keeps the dialog in place;
  a new connection is saved from the start screen with TLS on; the connection dialog keeps its place; Connect without
  the helper says why; an open window fills the main window under the header, says Not signed on, and Disconnect goes
  back to the start screen; a signed-on window has no Run on… although other connections are saved (D26); a connection needs a unique name and a host, keeps no password, and a rename keeps its
  history and pins. **Seen** in `--screenshot`s of the start screen and of a connection's sign-on in the main window
  (WSL2, dark). **Checked in the running app** by the owner, Linux under WSL (2026-11-14).
