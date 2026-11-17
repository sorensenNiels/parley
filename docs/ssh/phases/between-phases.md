# Between phases

> Requirements of Parley SSH built and verified outside any phase: none of the roadmap's phases names them. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

Asked for on their own, between phases; see each requirement's date.

## Requirements

| ID | Requirement |
|---|---|
| R-SSH-60 | **No console window for a proxy command on Windows** (2026-11-17, found with R-SQL-103). A connection through a proxy command starts `cmd /C` without a console window (Win32 `CREATE_NO_WINDOW`); its standard input and output are the connection as before. |
| R-SSH-61 | **A colour for each session** (2026-11-17, D54, as Parley 5250's R-UI-35). The session editor's "Colour" on the Session page: none or one of the eight named colours. The session in front shows it as the status bar's block and, while there is one tab, the line under the header; every tab has a stripe in the colour of its pane in front. Never as a background; nowhere else (*cut back by the owner the same day: no pill outline, pane frames or start screen stripe*). |

## Verification

- **R-SSH-60** (2026-11-17): no console window for a proxy command on Windows. `cmd /C` starts with
  `CREATE_NO_WINDOW`; the Windows branch is not compiled on Linux, so it has no automated test. **Checked in the running app** by the owner, who marked it done (2026-11-17; the operating system was not named).

- **R-SSH-61** (2026-11-17): a colour for each session, cut back the same day to a tab stripe, the line under the
  header with one tab, and the status bar's block. **Automated** (`make test`): the editor saves a chosen colour and
  old sessions read with none; a click on a tab's name brings it to the front with or without a colour (the name was
  a selectable label that took the click, R-SSH-39). **Checked in the running app** by the owner, who marked it done (2026-11-17; the operating system was not named).
