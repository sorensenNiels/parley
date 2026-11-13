# Parley ideas

> Ideas that might become requirements later. Nothing here is planned or promised: the plan is
> [ROADMAP.md](ROADMAP.md). Look an idea up with `grep -n` (`"| IDEA-13 "`) rather than reading the whole file.

## How an idea moves on

- A new idea gets the next free `IDEA-n` (last line of this file). IDs are never reused or renumbered.
- When an idea is picked for a phase, write it as one or more requirements in the roadmap (§5, next free ID of the
  area) and mark the row here *"→ R-XXX-n"*. Remove the row once its requirements are done; the number stays used.
- An idea that would change a decision in the roadmap (§7) says so; the decision is taken before it is built.
- Group ideas by theme; within a theme, newest last.

## Protocol gaps (known since Phase 1)

| ID | Idea |
|---|---|
| IDEA-2 | Operator error codes (0005 and so on) next to the messages, as IBM's emulators show them, once the codes are checked against the manual. |
| IDEA-3 | Mandatory enter, mandatory fill and self-check (modulo 10/11) fields: not enforced yet; the host checks them. |
| IDEA-4 | The Start of Header command-key bits that say whether F1–F24 return field data: all keys return data now. |
| IDEA-5 | Extended attributes (Write Extended Attribute) and the GUI structured fields (windows, menus, scroll bars, WDSF): ignored now; Parley's query reply does not ask for them. |
| IDEA-6 | Sound the alarm (CC2 bit) and blink (blink attributes): both are ignored now. |
| IDEA-7 | Save Partial Screen / Restore Partial Screen and Read Immediate Alternate: not supported yet (the host gets a notice-level error). |
| IDEA-8 | Insert mode accepts trailing blanks as room, not only nulls as on a real 5250: kinder, but check it does not surprise people. |
| IDEA-12 | *Dropped with auto sign-on (R-NET-7, D16): there is no bypass sign-on left to explain.* Startup Response Record (RFC 4777 section 10, `IBMSENDCONFREC`): show why a bypass sign-on failed (user unknown, password expired, and so on) instead of only the sign-on screen. *(Was → R-NET-6, dropped.)* |
| IDEA-14 | Type-ahead: keys pressed while the keyboard is locked (X SYSTEM) are kept and replayed when the host unlocks it, as IBM's emulators can; Reset clears the buffer. Off by default, a setting to turn on. |

## Sessions and connection

| ID | Idea |
|---|---|
| IDEA-9 | Session profiles: several saved connections, each with its own settings and keyboard map. *Saved sessions are R-UI-5 (2026-11-12); the keyboard map per session is R-KEY-2 (2026-11-12).* *→ R-KEY-2 (Phase 3).* |
| IDEA-15 | Several sessions in one window, as tabs (Ctrl+Tab between them), each with its own connection thread; a tab shows the session name and a dot for message waiting or a lost connection. Today it is one session per window (R-UI-5). |
| IDEA-16 | Reconnect: when the connection drops (not when the host ends the job), offer "Reconnect" in the status bar, and optionally reconnect on its own with back-off; the session's Run macro runs again. |
| IDEA-17 | Import sessions from IBM i Access Client Solutions (`.hod`) and Personal Communications (`.ws`) files: host, port, TLS, device name, code page, screen size. Lowers the cost of trying Parley. |
| IDEA-18 | Printer sessions (3812 emulation): receive spooled output on a printer device and save it as PDF or text, or send it to an OS printer. |

## Working with the screen

| ID | Idea |
|---|---|
| IDEA-19 | Hotspots: click "F3=Exit" (or any `Fnn=` text) to press that key, click an option number in a list to type it, click a URL to open it. Ctrl+click so a plain click still moves the cursor (R-UI-1). |
| IDEA-20 | Better copy: drag a rectangle to copy part of the screen; copy a list screen (subfile) as tab-separated text so it pastes into a spreadsheet as columns. |
| IDEA-21 | Screen history: keep the last screens of the session (as D7 keeps saved screens) and let the user page back through them read-only, search them, and copy from them; export a screen as text or HTML. |
| IDEA-22 | Find on screen (Ctrl+F): highlight matches on the current screen. |
| IDEA-23 | 5250 colour schemes: classic green-on-black (today), a light scheme, and user-chosen colours for the seven 5250 colours; per session, so production can look different from test. |
| IDEA-24 | Screen reader support for the 5250 screen beyond today's whole-screen text: the current field's label and contents, and the error line, announced as they change. |

## Data on the host

| ID | Idea |
|---|---|
| IDEA-13 | **SQL against the open session's host.** A "SQL…" window (Tools menu, a shortcut) for the connected session: type one or more statements, run them, see result sets in a grid (sortable, copy cells/rows, export CSV), messages and SQLSTATE for errors, row counts for updates, and a history of statements per session. It reuses the session's host, TLS setting and CA file; the user and password are asked for (Parley keeps none, D16). *How*: a second connection to the IBM i database host server (`QZDASOINIT`, port 8471, or 9471 with TLS; signed on through the sign-on server, 8476/9476), the protocol JTOpen and the ODBC driver use; built sans-I/O in `parley-core` (or its own crate) as the 5250 protocol is (D4), tested with recorded traffic. *Open questions*: the host-server protocol is documented mainly by JTOpen's source, so the first step is to check what can be implemented from public documentation; fall back to the installed IBM i Access ODBC driver (needs the user to install it); commit control (autocommit on by default, Commit/Rollback buttons); a guard for statements that change data (confirm `DELETE`/`UPDATE` without `WHERE`); a row limit with "fetch more". It would be the second kind of connection to a host, so it needs a decision next to D4/D6. *Probed 2026-11-13 without signing on (R-NET-8): the owner's IBM i (7.3) offers the host servers and DRDA (446) without TLS only (9471, 9476 refused; 447 not reachable), password level 0, no Mapepire (8076) and no SSH; PUB400 (7.5) offers the host servers with and without TLS (certificate trusted), DRDA on 446, password level 3, a Jetty server on 8076 with a self-signed certificate (likely Mapepire), and SSH on 2222. So the host servers and DRDA reach both; Mapepire only PUB400. Route chosen: the native host-server client (D19).* *→ R-SQL-1..13, R-NET-9 (Phase 5). Running SQL changed to a Java helper with JTOpen (D20, 2026-11-13).* |
| IDEA-25 | IBM i Services views built on IDEA-13: the job log of the session's own job, the spooled files of the user (open one as text or PDF), and the message queue behind the "message waiting" indicator, from the `QSYS2` SQL services. |
| IDEA-26 | Data transfer: download a table or a query result to CSV or Excel, and upload a CSV into a table, as Access Client Solutions' Data Transfer does; built on IDEA-13. |

## Automation

| ID | Idea |
|---|---|
| IDEA-10 | A trace window (the host's records, decoded) for support, built on `PARLEY_TRACE`. |
| IDEA-11 | Macros: record and play keystrokes, and run one after sign-on ("Run macro" in the session dialog, disabled until then). *Recording*: Tools ▸ Record macro starts it, the status bar shows "REC" until Stop, then Parley asks for a name; after each AID key (Enter, F-keys…) the macro waits for the host to unlock the keyboard before going on, so it does not race the host. Text typed into non-display fields (passwords) is never recorded: the macro stores a "password" step that uses the session's keychain password (D11) or asks for it. *Running*: Tools ▸ Run macro ▸ (name), a key the user assigns, or after sign-on; Esc or Reset stops a running macro. Macros are saved as readable text the user can edit, global or per session. *→ R-KEY-3, R-KEY-4 (Phase 3).* |
| IDEA-27 | A command bar (Ctrl+K): type a CL command, pick from recent ones, and Parley types it on the command line and presses Enter; it refuses when the screen has no command line. |

Next free ID: `IDEA-28`.
