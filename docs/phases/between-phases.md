# Between phases

> Requirements of Parley 5250 built and verified outside any phase: none of the roadmap's phases names them. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

Asked for on their own, between phases; see each requirement's date.

## Requirements

| ID | Requirement |
|---|---|
| R-UI-18 | **Settings from a gear, no Help menu** (2026-11-14, asked by the owner; as Parley SQL's header, R-SQL-45. *Changes R-UI-15 and R-UI-17:* the Help menu goes.) Parley's header has no Help menu: at its right end, before the session pill, a gear (named "Settings" for screen readers, with that tooltip) opens Settings, and Tools no longer has Settings…. What Help held is elsewhere: Keyboard shortcuts in View (R-UI-17) and on its key, the version and Check now on Settings ▸ About (R-UI-11). Tools shows only while it has something (macros, R-ADD-2, or SQL…, R-SQL-6). |
| R-UI-19 | **A larger gear** (2026-11-15, asked by the owner; adds to R-UI-18 and R-SQL-45.) The Settings gear at the right end of Parley's and Parley SQL's headers is drawn about half as large again as the menus' text, and grows with the text size setting. |
| R-UI-29 | **Japanese, Chinese and Korean text** (2026-11-17; asked by the owner: a Japanese name in Parley SQL's results showed as boxes). The bundled fonts have no CJK glyphs, so the system's own fallback fonts for Han as Japanese, Han as Chinese and Hangul (fontique's fallback lookup: Yu Gothic, Microsoft YaHei and Malgun Gothic on Windows, Hiragino and the like on macOS, Noto CJK through fontconfig on Linux) are added last to the UI, heading, monospace and terminal families of all three programs, each font once; they are looked for once per run. With none installed, nothing changes. Checked on Windows: a Japanese name from a UTF-8 or UTF-16 column shows in the result grid. |
| R-UI-35 | **A colour for each session** (2026-11-17, D54). The session dialog's "Colour": none (default) or one of eight named colours (red, orange, yellow, green, teal, blue, purple, pink), each with a light and a dark variant in the theme. The session in front shows its colour as a block at the left end of the status bar and, while it is the only tab, a 3 px line under the header; each tab has a 3 px stripe in its session's colour along its top (R-UI-33). Never as a background; nowhere else (*cut back by the owner the same day: no pill outline, screen frame or start screen stripe*). Shared with Parley SSH (R-SSH-61). |
| R-UI-36 | **Select with Shift and the arrow keys** (2026-11-18, asked by the owner; adds to R-UI-22). Four new bindable functions `select-up`, `select-down`, `select-left`, `select-right` (Shift+↑ ↓ ← → in the Default map, listed under Parley 5250 in Settings and the shortcut sheet) grow the selection one position that way, within the screen: without a selection one starts at the cursor; after a drag the end the pointer let go of moves. The selection is the same rectangle a drag makes, so Copy and Copy as table copy it, and it goes as R-UI-22 says. The cursor stays where it is and nothing goes to the host. Saved maps other than Default get the new functions unbound. |
| R-NF-3 | **Choose the programs when installing on Windows** (2026-11-20, asked by the owner; D55). The `.msi` has a page to choose which of Parley 5250, Parley SQL and Parley SSH to install, all three ticked; at least one must be chosen. Each one's program, Start menu entry (and Parley 5250's desktop shortcut) comes only with it; the install folder, PATH and licences always. Settings ▸ Apps ▸ Modify (or the `.msi` again) adds or removes one later. The product stays "Parley 5250" in Apps and its folder (the owner, 2026-11-20). Parley 5250 leaves out Tools ▸ SQL… and Tools ▸ SSH…, and their switches in Settings ▸ Add-ons, when that program is not beside it. Other platforms keep installing all three. |
| R-NF-4 | **Three Linux packages** (2026-11-20, asked by the owner; D56). In place of one `.deb` with all three: `parley-5250` (`parley`), `parley-sql` (`parley-sql` and the SQL helper) and `parley-ssh` (`parley-ssh`), each with its own entry and icon in the application menu, all in `/usr/bin`; any of them can be installed alone, and `sudo apt install ./parley*_amd64.deb` installs all three. Each takes over the files of an earlier all-in-one `parley-5250` or `parley` package (Replaces and Breaks), so installing the new ones over an old one works. The AppImage stays one file with all three. |
| R-NF-5 | **Three Mac apps** (2026-11-20, asked by the owner; D56). The `.dmg` holds Parley 5250.app, Parley SQL.app and Parley SSH.app beside a link to Applications; the user drags in the ones wanted, and each has its own name in the Dock and Launchpad. Parley 5250 finds Parley SQL and Parley SSH beside its own app, in /Applications or in ~/Applications; one not found is left out of its menus (R-NF-3). |

## Verification

- **R-UI-18** (2026-11-14): Settings from a gear, no Help menu, built. **Automated** (`make test`): the menu bar
  starts with the app and ends with the gear, with no Help menu; Settings opens from the gear and not from Tools;
  Tools is not there while macros and SQL are both off; Check for updates runs from Settings ▸ About. **Checked in
  the running app** by the owner, Linux under WSL (2026-11-14).

- **R-UI-19** (2026-11-15): a larger gear. **Automated** (`make test`): the menu bar test checks the gear is wider
  than it was at the menus' text size. **Checked in the running app** by a screenshot of Parley and Parley SQL, Linux
  under WSL (2026-11-15).

- **R-UI-29** (2026-11-17): Japanese, Chinese and Korean text. **Automated** (`make test`): the system's fallback
  fonts are added last to every text family and the terminal's, once each, and looked for once per run. **Checked in the running app** by the owner, who marked it done (2026-11-17; the operating system was not named).

- **R-UI-35** (2026-11-17): a colour for each session, cut back the same day to a tab stripe, the line under the
  header with one tab, and the status bar's block. **Automated** (`make test`): eight colours, each with a light and
  a dark variant, and none for No colour; a swatch click chooses a colour; the session dialog saves it, and old
  sessions read with none. **Checked in the running app** by the owner, who marked it done (2026-11-17; the operating system was not named).

- **R-UI-36** (2026-11-18): select with Shift and the arrow keys. **Automated** (`make test`): Shift+→ →, ↓ from
  the cursor selects a rectangle that Copy copies, and the cursor stays; the selection stops at the screen's edge and
  shrinks back; Shift+→ grows a dragged selection; an arrow without Shift drops it; the four `select-…` functions
  read back from a map. **Checked in the running app** by the owner on Windows, macOS and Linux (2026-11-18).

- **R-NF-3** (2026-11-20): choose the programs when installing on Windows (D55). **Automated** (`make test`): Parley 5250
  without Parley SQL or Parley SSH beside it has no Tools ▸ SQL… or SSH…, no key for them and no switch in Settings ▸
  Add-ons (`sql_and_ssh_not_installed_are_left_out_of_tools_and_add_ons_r_nf_3`). Built: the WiX
  template `crates/parley-app/windows/main.wxs` (WiX runs only on Windows); its check is in docs/RELEASING.md.
  **Checked in the running app** by the owner on Windows, macOS, Linux and WSL (2026-11-20).

- **R-NF-4, R-NF-5** (2026-11-20): one package per program on Linux and macOS (D56). **Automated** (`make test`): Parley
  5250 looks for Parley SQL and Parley SSH beside it, and on macOS in their own apps beside its app, in
  /Applications and ~/Applications (`siblings.rs`); Parley SQL finds its helper in `/usr/lib/parley-sql`.
  **Checked on Linux (WSL)**: `scripts/package-split.sh deb` makes `parley-5250`, `parley-sql` (with the helper) and
  `parley-ssh`, each with its own menu entry and icon; in a throwaway dpkg database, the three install over an
  all-in-one `parley-5250` 0.14.0 without file conflicts, and removing `parley-5250` leaves the other two.
  **Checked in the running app** by the owner on Windows, macOS, Linux and WSL (2026-11-20).
