# Between phases

> Requirements of Parley SSH built and verified outside any phase: none of the roadmap's phases names them. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

Asked for on their own, between phases; see each requirement's date.

## Requirements

| ID | Requirement |
|---|---|
| R-SSH-60 | **No console window for a proxy command on Windows** (2026-11-17, found with R-SQL-103). A connection through a proxy command starts `cmd /C` without a console window (Win32 `CREATE_NO_WINDOW`); its standard input and output are the connection as before. |
| R-SSH-61 | **A colour for each session** (2026-11-17, D54, as Parley 5250's R-UI-35). The session editor's "Colour" on the Session page: none or one of the eight named colours. The session in front shows it as the status bar's block and, while there is one tab, the line under the header; every tab has a stripe in the colour of its pane in front. Never as a background; nowhere else (*cut back by the owner the same day: no pill outline, pane frames or start screen stripe*). |
| R-SSH-62 | **The user name asked when the session has none** (2026-11-19, asked by the owner). A session with no user name (a saved session, a restored tab, Restart) connects, checks the host key, then asks "User name" in the sign-on dialog, shown as typed and asked again while empty; Cancel ends the connection quietly. It signs on as that user, which the tab, the status bar, Restart and reconnecting (R-SSH-51) keep without asking again; the saved session is not changed. A jump host with no user anywhere asks for its own, naming its hop. The session editor's User name says "empty: asked when connecting". Quick connect and R-SSH-59 still ask for the user in the start screen. |

## Verification

- **R-SSH-60** (2026-11-17): no console window for a proxy command on Windows. `cmd /C` starts with
  `CREATE_NO_WINDOW`; the Windows branch is not compiled on Linux, so it has no automated test. **Checked in the running app** by the owner, who marked it done (2026-11-17; the operating system was not named).

- **R-SSH-61** (2026-11-17): a colour for each session, cut back the same day to a tab stripe, the line under the
  header with one tab, and the status bar's block. **Automated** (`make test`): the editor saves a chosen colour and
  old sessions read with none; a click on a tab's name brings it to the front with or without a colour (the name was
  a selectable label that took the click, R-SSH-39). **Checked in the running app** by the owner, who marked it done (2026-11-17; the operating system was not named).

- **R-SSH-62** (R-SSH-62 built 2026-11-19, Linux under WSL). **Automated** (`make test`): `parley-ssh-core`'s `tests/net.rs` against an in-process russh server (no user: the host key, then "User name" shown as typed, blank asked again, `Event::User` with the trimmed name, the password prompt and the server's sign-on as that user, `æøå`; Cancel on the user name ends quietly).
  - R-SSH-62: `sign_on::ask_user` before signing on when `Target::user` is empty, `Event::User` taken into the session's target and source, `Target::label` without `user@` while unknown, the editor's hint. Checked: A saved session without a user: the dialog, the tab title after signing on, Restart not asking again.
  **Checked in the running app** by the owner on Windows, macOS, Linux and WSL (2026-11-20).
