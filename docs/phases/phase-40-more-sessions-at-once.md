# Phase 40 — More sessions at once

> Finished phase of Parley 5250. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

**Status:** Done (2026-11-17), checked by the owner: R-UI-30..34.

From the owner, 2026-11-17 (D53), with IDEA-15: duplicate the session in use and start a named group of saved sessions
at once. Where the extra sessions open is the user's setting: a window each, as today, or tabs in one window. Built in
steps: tabs and the setting (R-UI-33, R-UI-34), then duplicate (R-UI-30) and groups (R-UI-31, R-UI-32), which open
the way the setting says.

## Requirements

| ID | Requirement |
|---|---|
| R-UI-30 | **Duplicate the session** (2026-11-17, D53). File ▸ Duplicate session opens the session in use again, saved or not, with the same host, port, TLS, code page, size, keyboard map and macro, but no device name, so the IBM i gives it one of its own; it opens where R-UI-34 says. Disabled on the start screen. |
| R-UI-31 | **Session groups** (2026-11-17, D53). A group is a name and a list of saved sessions, in order; a session may be in several groups. File ▸ Session groups… (and Edit… on a group's card on the start screen) opens a dialog that makes, renames, orders and deletes groups and switches their sessions on and off. A group's sessions start in the saved sessions' order. Groups are saved with the settings; renaming a session keeps it in its groups, deleting it takes it out. |
| R-UI-32 | **Starting a group** (2026-11-17, D53). The start screen lists the groups under the saved sessions, each with Start and Edit…; File ▸ Start group ▸ lists them too. Starting opens every session of the group where R-UI-34 says; from the start screen the first takes the window it is started from. A group can be marked "start when Parley starts", as a session can (R-UI-5): at most one session or group is marked, and marking one unmarks the other. |
| R-UI-33 | **Sessions as tabs** (2026-11-17, IDEA-15, D53). One window holds several sessions as tabs, each with its own connection, screen, find bar, macros and status; a tab shows the session's name and a dot for message waiting or a lost connection. Ctrl+Tab and Ctrl+Shift+Tab move between tabs; a tab closes from its × (asking first while connected, as closing the window does); closing the window asks once for all connected tabs. With one session there is no tab bar. |
| R-UI-34 | **Where more sessions open** (2026-11-17, D53). Settings ▸ Terminal ▸ Sessions: "Open more sessions in" a new window (default, as before) or a new tab. A duplicate (R-UI-30) and a group's sessions (R-UI-32) open there; a new window is another Parley started with `--session NAME` (or the host, for a session not saved). |

## Verification

- **Phase 40** (2026-11-17), built; `make verify` on Linux under WSL. **Automated** (`make test`):
  - **R-UI-30** a duplicate opens in a tab without the device name, or by default in another window as a saved
    session with `--no-device-name`; a window that cannot open says why.
  - **R-UI-31** groups follow a renamed session and forget a deleted one; every group needs a name of its own; the
    groups dialog keeps its buttons in place and saves a new group in the saved sessions' order.
  - **R-UI-32** a group starts here first, then in windows, leaving out sessions no longer saved; from File ▸ Start
    group it starts in tabs; one group at most starts with Parley, and a session marked to start unmarks them.
  - **R-UI-33** Ctrl+Tab and Ctrl+Shift+Tab move between tabs; × closes one, asking first while connected; a tab
    behind whose host ends it closes and the window stays; closing the window asks once for every connected tab.
  - **R-UI-34** a saved session opens in another window by its name, one not saved by its host, port and TLS
    (`parley --session NAME`, read back by Parley's own command line).
- **Checked in the running app** by the owner, who marked it done (2026-11-17; the operating system was not named).
