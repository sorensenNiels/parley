# Phase 39 — Reconnect and restore

> Finished phase of Parley SSH. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

A dropped connection comes back by itself after sleep or a network change, and the tabs of last time can open again
on start. R-SSH-51, R-SSH-52.

**Status:** Done (2026-11-20), checked by the owner on Windows, macOS, Linux and WSL: R-SSH-51, R-SSH-52.

## Requirements

| ID | Requirement |
|---|---|
| R-SSH-51 | **Reconnect** (2026-11-16). A connection lost without the user ending it (the network dropped, the computer slept, keepalives unanswered, R-SSH-26) reconnects by itself with the same settings: after 1, 2, 4, 8 … seconds (at most 30 between tries) for 10 minutes (a setting: off, or 1–60 minutes), a line in the terminal for each try and the status bar counting down; Restart now and Stop in the status bar. Signing on again asks only for what it needs (a key or the agent needs nothing; a password is asked again, D16). On success the screen keeps its text and a line says it reconnected; the shell is a new one. Not when the shell exited or the server refused the sign-on. |
| R-SSH-52 | **Restore** (2026-11-16; *details 2026-11-16, D50*). Settings: "Open the tabs of last time" (off by default). When on, Parley SSH saves its tabs, each tab's split panes and their sizes, and each pane's session (saved session by name, else host, port, user and options; never a password) when it closes, and on the next start without a host on the command line opens them again; each pane connects when its tab is first shown. Screens start empty (D50). A saved session since deleted opens with its host as it was. |

## Verification

- **R-SSH-51, R-SSH-52** (built 2026-11-16, Linux under WSL; D50). **Automated** (`make test`): `parley-ssh-core` `tests/net.rs` (a refused host key ends as `Ending::Refused`, which never reconnects); `parley-ssh-ui`'s `reconnect.rs` (only a connection lost while its shell ran comes back; waits of 1, 2, 4, 8, 16 … seconds, at most 30, until the minutes run out), `session.rs` (lost, a failed try with its wait said on the screen, the event log kept across tries, Stop; nothing when off or when the shell never ran), `tests.rs` (two tabs, a split with two sessions, kept and opened again with both panes connecting when shown; restore off; a host on the command line winning; the Terminal page's Reconnect and Restore settings).
  - R-SSH-51: Settings ▸ Terminal ▸ Connections: "Reconnect a lost connection" (on) for 1–60 minutes (10). A connection that fails after its shell ran (`Ending::Failed`; refusals are `Ending::Refused`) is tried again on the same screen after 1, 2, 4 … 30 s; faint lines on the screen and in the event log say each step; the status bar counts down with Restart now and Stop; a key or the agent signs on again by itself, a password is asked (D16); "Reconnected: this is a new shell." A session in a tab not in front reconnects when its tab is shown. Checked: Pull the network cable (or `sudo ss -K` the connection) during `top`; sleep the laptop; a password session asking again; Stop; Restart now; the limit running out.
  - R-SSH-52: Settings ▸ Terminal ▸ Connections: "Open the tabs of last time" (off). `settings_to_save` keeps each tab's layout with its shares and each pane's session (`SavedTab`, never a password) whenever Parley SSH saves its settings; on a start without a host they open again, panes renumbered, a saved session as it is now (one since deleted as it was), connecting when the tab is first shown; screens start empty (D50). Checked: Quit with three tabs (one split), start again; with restore off; `parley-ssh host` with restore on; a deleted saved session.
  **Checked in the running app** by the owner on Windows, macOS, Linux and WSL (2026-11-20).
