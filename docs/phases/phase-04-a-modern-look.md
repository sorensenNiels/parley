# Phase 4 — A modern look

> Finished phase of Parley 5250. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

The owner chose these from mockups of three options per area (2026-11-12): an icon font (R-UI-13, D17); a status
bar of pills that shows only what is true and can be clicked (R-UI-14, option S3); one header row with the session
instead of the menu bar plus toolbar (R-UI-15, option T2); the seldom-used 5250 keys in a Keys menu in the status bar (R-UI-16; first a key strip under the screen, changed 2026-11-13); and
a keyboard shortcut sheet over the screen instead of the side panel (R-UI-17, option K1). Built in that order.
Labels from the screen ("F3=Exit") on the function keys stayed IDEA-19, now R-UI-20 in Phase 20.

**Status:** Done (2026-11-15): verified on Windows, macOS, Linux and WSL (FEATURES.md): icons (R-UI-13, D17), an interactive status bar (R-UI-14), one header with the session (R-UI-15), the 5250 keys in a Keys menu in the status bar (R-UI-16), a shortcut sheet (R-UI-17), no error for a key in a protected area (R-KEY-8); both commands after installing (R-NF-2).

## Requirements

| ID | Requirement |
|---|---|
| R-KEY-8 | **No error for a key in a protected area** (2026-11-13, asked by the owner; D18). A character, Delete, Backspace, Erase EOF or Field Exit typed where the cursor is not in an input field does nothing: no operator error, no "Press Reset", the screen unchanged. Tab, Shift+Tab, the arrows and a click still move the cursor to an input field. Other operator errors (a letter in a numeric field, a full field in insert mode, …) are unchanged. *(Changes R-KEY-1 and R-UI-2: the protected-area error is gone.)* |
| R-UI-13 | **Icons** (2026-11-12, the owner; D17). Parley bundles the Phosphor icon font (Regular) beside its other fonts, in a font family of its own, and names each icon it uses in one `icons` module; `bundled_fonts_have_every_symbol_the_ui_uses` checks every one. Menu items, the session pill, the Keys menu and the status bar's chips show an icon before their text where one helps. |
| R-UI-14 | **An interactive status bar** (2026-11-12, the owner; option S3). *Changes R-UI-2, R-UI-4 and R-KEY-6: the boxes go.* A flat bar. On the left a green check icon (Ready) and beside it a processing slot of its own: an amber hourglass while the host works after an AID key (X SYSTEM), also when nothing else would draw a frame, empty otherwise but as wide, so nothing moves; the check stays while the host works (the owner, 2026-11-13); the operator error with "Press Reset" as a red pill; Connecting…, or Offline with why the last connection ended and, when a saved session was in use, a "Connect again" button; a notice from the host follows the pill in muted text. While a macro is recorded, a red "REC m:ss" pill and a Stop button that does what Tools ▸ Stop recording does (R-KEY-6); while one runs, a pill with its name. On the right, each only while it applies: message waiting (amber pill), INS, TLS (hover: how the host's certificate is checked), the keyboard map (click: a menu of the maps; the one chosen is saved to the session, R-KEY-2), the code page (CCSID; hover: its name), the size of the screen in use (24×80, 27×132), the response time (from an AID key locking the keyboard to the host unlocking it, measured by the UI; under 1 s in ms), and the cursor's row, column. When the window is too narrow, response time, then size, code page and keyboard map hide first. The bar keeps one height. |
| R-UI-15 | **One header** (2026-11-12, the owner; option T2). *Changes R-UI-4: the toolbar row goes.* The menu bar keeps the brand, File, Edit, View, Tools, Help and the update link (R-UI-10, R-UI-11), and gains a session pill on the right while connected: a status dot, the session's name, host:port and a lock when TLS is on. It opens a menu with Connect to ▸, Edit session… and Disconnect. Menu items have icons (R-UI-13); those with a key show the key of the map in use. |
| R-UI-16 | **The 5250 keys in a Keys menu** (2026-11-12, the owner; option T2. *Changed 2026-11-13 by the owner: the key strip under the screen was too busy for keys seldom used, so they moved into the status bar.*) While connected, a "Keys" button after the state pill in the status bar (R-UI-14) opens a menu: Attention, System Request, Reset, Clear, Help, Print, Roll Down and Roll Up, each with the keys of the map in use that press it, then F1–F24 in a grid whose buttons show their keys on hover as key caps. A click sends the key. |
| R-UI-17 | **A keyboard shortcut sheet** (2026-11-12, the owner; option K1). *Changes R-UI-2's list of shortcuts and R-UI-7: the side panel goes.* Help ▸ Keyboard shortcuts, View ▸ Keyboard shortcuts and a new bindable function `shortcuts` (Ctrl+/ in the Default map, R-KEY-2) open a dialog over the screen: the keys of the map in use under the same headings as Settings ▸ Keyboard, as key caps, with a filter field and "Change keys…" (Settings on the Keyboard page). Esc, the same key, 🗙 or Close closes it. |
| R-NF-2 | **Both commands after installing** (2026-11-15, asked by the owner; D26, adds to R-NF-1). One release, whose installers hold both programs; after installing, `parley` and `parley-sql` work in a terminal and both are in the menu where the OS allows it. Windows `.msi`: the install folder on the system PATH (removed on uninstall), Parley SQL in the Start menu. `.deb`: both in `/usr/bin`, Parley SQL in the application menu. AppImage: `parley --sql [ARGS]`, and the AppImage started through a link named `parley-sql`, start Parley SQL; the release notes say how to make the links. macOS: the release notes give the two links into `/usr/local/bin`. `parley --sql` works on every OS. The release notes start with these install steps. |

## Verification

### Phase 3 (the rest) and Phase 4: a modern look (2026-11-15) — R-KEY-5..8, R-UI-13..17, R-NF-2

Verified by the owner in the running app on Windows, macOS, Linux and Linux under WSL (2026-11-15), including
R-NF-2's `.msi` (PATH and Start menu) and the macOS links.

- **R-NF-2** (2026-11-15): both commands after installing. Built; **automated** (`make test`): `--sql` first and a
  start by the name `parley-sql` hand over to Parley SQL, anything else is Parley. **Checked on Linux under WSL:**
  the `.deb` holds `/usr/bin/parley`, `/usr/bin/parley-sql` and both menu entries; the AppImage answers
  `--version`, `--sql --version` and, through a link named `parley-sql`, `--version` as Parley SQL.

- **R-KEY-8** (2026-11-13): a key typed in a protected area is ignored, with no error to Reset (D18).
  **Automated** (`make test`): in parley-core, a character (also `æ`), Delete, Backspace, Erase EOF and Field Exit at a
  protected position leave the keyboard unlocked and the screen unchanged, and Tab then goes to the next input field.

- **R-UI-13** (2026-11-12): icons from the bundled Phosphor font (D17). **Automated** (`make test`): every
  icon in `icons::ALL` is in the icon font, which is the only font in its family and stays out of the text families
  (Inter has glyphs of its own at some of the same codepoints). **Seen** in `--screenshot`s on PUB400's sign-on screen
  (WSL2, light and dark).

- **R-UI-14** (2026-11-12): the interactive status bar. **Automated** (`make test`): the state, TLS, keyboard
  map, code page, size and cursor show, INS only in insert mode; a narrow bar drops size and code page but keeps TLS
  and the cursor; the clock times the host from an AID key to the unlocked keyboard; the lock is noticed in the frame Enter
  goes down, the hourglass has its own slot beside the check, which stays, and Keys does not move; the REC pill and Stop keep the
  state pill in place (R-KEY-6); choosing a map in the chip saves it to the session. **Seen** in `--screenshot`s (WSL2,
  light and dark: Ready, 125 ms, 24×80, CCSID 37, Default, 5, 25).

- **R-UI-15** (2026-11-12): one header with the session pill. **Automated** (`make test`): menus and Help are
  there before connecting; the pill shows host:port and its menu disconnects; Connect to another session while
  connected closes the open one first. **Seen** in `--screenshot`s (WSL2, light and dark).

- **R-UI-16** (2026-11-12, changed 2026-11-13): the 5250 keys in a Keys menu in the status bar. **Automated**
  (`make test`): Attention and F15 are sent from the menu, the menu lists System Request and F24 while connected, and
  there is no Keys button before connecting. **Seen** in a `--screenshot` (WSL2, light): Ready, Keys, then the chips.

- **R-UI-17** (2026-11-12): the keyboard shortcut sheet. **Automated** (`make test`): the Default map's keys
  under Settings' headings, Ctrl+/ opens it from the screen and it stays open the next frame, Esc closes it (not on
  the frame it opens), both close buttons, Change keys… opens Settings ▸ Keyboard, the filter, and the dialog keeps
  its place.
