# Parley ideas

> Ideas that might become requirements later. Nothing here is planned or promised: the plan is
> [ROADMAP.md](ROADMAP.md). Look an idea up with `grep -n` (`"| IDEA-13 "`) rather than reading the whole file.
> Ideas for SQL (the SQL window, Parley SQL) are in [sql/IDEAS.md](sql/IDEAS.md); both files share one sequence of
> numbers, kept on the last line of this file.

## How an idea moves on

- A new idea gets the next free `IDEA-n` (last line of this file). IDs are never reused or renumbered.
- When an idea is picked for a phase, write it as one or more requirements in the roadmap (§5, next free ID of the
  area) and mark the row here *"→ R-XXX-n"*. Remove the row once its requirements are done; the number stays used.
- An idea that would change a decision in the roadmap (§7) says so; the decision is taken before it is built.
- Group ideas by theme; within a theme, newest last.

## Protocol gaps (known since Phase 1)

| ID | Idea |
|---|---|
| IDEA-5 | Extended attributes (Write Extended Attribute) and the GUI structured fields (windows, menus, scroll bars, WDSF): ignored now; Parley's query reply does not ask for them. |
| IDEA-8 | Insert mode accepts trailing blanks as room, not only nulls as on a real 5250: kinder, but check it does not surprise people. |
| IDEA-12 | *Dropped with auto sign-on (R-NET-7, D16): there is no bypass sign-on left to explain.* Startup Response Record (RFC 4777 section 10, `IBMSENDCONFREC`): show why a bypass sign-on failed (user unknown, password expired, and so on) instead of only the sign-on screen. *(Was → R-NET-6, dropped.)* |
| IDEA-14 | Type-ahead: keys pressed while the keyboard is locked (X SYSTEM) are kept and replayed when the host unlocks it, as IBM's emulators can; Reset clears the buffer. Off by default, a setting to turn on. |

## Sessions and connection

| ID | Idea |
|---|---|
| IDEA-15 | Several sessions in one window, as tabs (Ctrl+Tab between them), each with its own connection thread; a tab shows the session name and a dot for message waiting or a lost connection. Today it is one session per window (R-UI-5). |
| IDEA-17 | Import sessions from IBM i Access Client Solutions (`.hod`) and Personal Communications (`.ws`) files: host, port, TLS, device name, code page, screen size. Lowers the cost of trying Parley. |
| IDEA-18 | Printer sessions (3812 emulation): receive spooled output on a printer device and save it as PDF or text, or send it to an OS printer. |

## Working with the screen

| ID | Idea |
|---|---|
| IDEA-21 | Screen history: keep the last screens of the session (as D7 keeps saved screens) and let the user page back through them read-only, search them, and copy from them; export a screen as text or HTML. |
| IDEA-24 | Screen reader support for the 5250 screen beyond today's whole-screen text: the current field's label and contents, and the error line, announced as they change. |

## Automation

| ID | Idea |
|---|---|
| IDEA-10 | A trace window (the host's records, decoded) for support, built on `PARLEY_TRACE`. |
| IDEA-27 | A command bar (Ctrl+K): type a CL command, pick from recent ones, and Parley types it on the command line and presses Enter; it refuses when the screen has no command line. |

Next free ID: `IDEA-37`.
