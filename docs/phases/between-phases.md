# Between phases

> Requirements of Parley 5250 built and verified outside any phase: none of the roadmap's phases names them. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

Asked for on their own, between phases; see each requirement's date.

## Requirements

| ID | Requirement |
|---|---|
| R-UI-18 | **Settings from a gear, no Help menu** (2026-11-14, asked by the owner; as Parley SQL's header, R-SQL-45. *Changes R-UI-15 and R-UI-17:* the Help menu goes.) Parley's header has no Help menu: at its right end, before the session pill, a gear (named "Settings" for screen readers, with that tooltip) opens Settings, and Tools no longer has Settings…. What Help held is elsewhere: Keyboard shortcuts in View (R-UI-17) and on its key, the version and Check now on Settings ▸ About (R-UI-11). Tools shows only while it has something (macros, R-ADD-2, or SQL…, R-SQL-6). |
| R-UI-19 | **A larger gear** (2026-11-15, asked by the owner; adds to R-UI-18 and R-SQL-45.) The Settings gear at the right end of Parley's and Parley SQL's headers is drawn about half as large again as the menus' text, and grows with the text size setting. |

## Verification

- **R-UI-18** (2026-11-14): Settings from a gear, no Help menu, built. **Automated** (`make test`): the menu bar
  starts with the app and ends with the gear, with no Help menu; Settings opens from the gear and not from Tools;
  Tools is not there while macros and SQL are both off; Check for updates runs from Settings ▸ About. **Checked in
  the running app** by the owner, Linux under WSL (2026-11-14).

- **R-UI-19** (2026-11-15): a larger gear. **Automated** (`make test`): the menu bar test checks the gear is wider
  than it was at the menus' text size. **Checked in the running app** by a screenshot of Parley and Parley SQL, Linux
  under WSL (2026-11-15).
