# Phase 13 — The editor, handier

> Finished phase of Parley SQL. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

Asked by the owner (2026-11-14): the small things a code editor is expected to do, on egui's text field as it is
(R-SQL-52..59). Bigger editor work (line numbers, a gutter, an editor widget of Parley's own) is IDEA-34. The text
changes are pure functions in `parley_sql_core::edit`, tested without the UI; the editor turns keys into them.

**Status:** Done, verified (2026-11-14, Linux under WSL): asked by the owner: comment lines (R-SQL-52), indent and outdent (R-SQL-53), move and duplicate lines (R-SQL-54), brackets and quotes that close themselves (R-SQL-55), the matching bracket (R-SQL-56), go to a line (R-SQL-57), keyword case (R-SQL-58), zoom the editor (R-SQL-59); then Parley SQL's Shortcuts page (R-SQL-60).

## Requirements

| ID | Requirement |
|---|---|
| R-SQL-52 | **Comment lines** (2026-11-14; asked by the owner). Ctrl+/ (also Ctrl+Shift+7 on keyboards where `/` needs Shift) comments out the lines the selection touches, or the cursor's line: `-- ` at the smallest indentation of the lines that are not blank. When every line that is not blank already starts with `--` after its indentation, it takes the `--` and one space after it away instead. The selection then covers the lines changed. |
| R-SQL-53 | **Indent and outdent** (2026-11-14; asked by the owner). With a selection, Tab indents each line it touches by four spaces (the formatter's indentation, R-SQL-22) and keeps the lines selected, instead of replacing the selection with a tab; Shift+Tab takes up to four spaces (or one tab) off the start of each line, also of the cursor's line without a selection. Without a selection, Tab puts spaces up to the next multiple of four columns; the editor never types a tab character itself. |
| R-SQL-54 | **Move and duplicate lines** (2026-11-14; asked by the owner). Alt+↑ and Alt+↓ move the lines the selection touches (or the cursor's line) one line up or down, keeping them selected; at the first or last line nothing happens. Ctrl+Shift+D puts a copy of those lines below them and moves the selection or cursor onto the copy. |
| R-SQL-55 | **Brackets and quotes close themselves** (2026-11-14; asked by the owner). Typing `(`, `'` or `"` puts in the closing one too, the cursor between, when the next character is a space, the end of the line, `)`, `,` or `;`, the cursor is not inside a string, name or comment, and, for a quote, the character before is not a letter, digit or `_`; with a selection, the pair goes around it. Typing `)` just before a `)`, or a quote just before the quote that closes the string or name, steps over it. Backspace between an empty pair takes both. A switch "Close brackets and quotes" (on by default) in Settings ▸ Appearance ▸ SQL window turns it off. |
| R-SQL-56 | **The matching bracket** (2026-11-14; asked by the owner). With the cursor just after or before `(` or `)` outside strings, names and comments, it and its partner are drawn on a background; one without a partner is drawn in the error colour. |
| R-SQL-57 | **Go to a line** (2026-11-14; asked by the owner). Ctrl+G opens a bar over the editor, as the find bar is: a field "Line" and "of *n*"; Enter puts the cursor at the start of that line (past the end: the last line), shows it and closes the bar; Esc closes it. |
| R-SQL-58 | **Keyword case** (2026-11-14; asked by the owner). Ctrl+Shift+U puts SQL's keywords (those coloured, R-SQL-19) in the selection, or the statement under the cursor, in upper case, Ctrl+Shift+L in lower case; names, strings and comments are left as they are. Also as "Upper case" and "Lower case" beside Format. |
| R-SQL-59 | **Zoom the editor** (2026-11-14; asked by the owner). Ctrl and the mouse wheel over the editor, or Ctrl+Plus and Ctrl+Minus while it has the focus, make the editor's text larger or smaller (from 60 % to 250 %, in steps of 10 %), and Ctrl+0 gives back 100 %; the rest of the window keeps its size. The size lasts while the window is open. |
| R-SQL-60 | **Change the SQL window's shortcuts** (2026-11-14; asked by the owner. *Changes R-SQL-47:* Parley SQL gets a keyboard page.) Parley SQL's Settings has a page "Shortcuts" under General, listing every action of the SQL window that has a shortcut, in groups (Run, Scripts, Find, Editing, Zoom), each with its keys: click a key and press a new one to change it, 🗙 removes it, + adds one, Reset gives back the action's own keys. A key another action has moves only when the user says so ("Use it for …" or "Keep it"). Keys that the text field or the system need are refused with the reason: keys without Ctrl or Alt (except F1–F24 and Esc), Ctrl+C, Ctrl+V, Ctrl+X, Ctrl+A, Ctrl+Z, Ctrl+Y and Ctrl with the arrows, Home, End, Backspace or Delete. A field filters the list by an action's name or key. Below, the keys that cannot change are listed with what they do (Tab and Shift+Tab, Esc on a bar or the completion list, the completion list's keys, undo, redo, copy, paste). The settings keep only the actions whose keys differ from their own; Restore defaults gives every action its own keys. The window's buttons and menus show the keys in use. Parley's SQL add-on keeps the default keys (the owner, 2026-11-14). |

## Verification

- **R-SQL-60** (2026-11-14): Parley SQL's Shortcuts page, built. **Automated** (`make test`): every action has its
  own keys, which read back and are allowed; only changed actions are kept; typing and text-field keys are refused;
  Ctrl+Shift+F is taken as Format, not Find; a free key replaces or joins at once, a taken one moves only when
  confirmed; in kittest the page changes Go to line to Ctrl+Shift+G by click and press, Parley SQL's settings list
  the page and keep Save in place, and a window given F3 for Find opens the find bar on F3 and not on Ctrl+F.
  **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-52..59** (2026-11-14): the editor, handier (Phase 13), built. **Automated** (`make test`): the text
  changes in `parley_core::sql::edit` (comment, indent, outdent, Tab stops, move and duplicate lines, pairs, the
  matching bracket, keyword case, line starts), with non-ASCII text and edges; in kittest Ctrl+/, Tab, Shift+Tab,
  Alt+↓ and Ctrl+Shift+D reach the text, `(` closes itself only with the setting on, Ctrl+G goes to a line,
  Ctrl+Shift+U and the zoom keys work, the bracket pair is marked and a lone one in the error colour. Fixed on the way:
  the editor kept the keyboard when Automatic moved the results (R-SQL-49), and Enter in the find bar no longer types
  a line break over the match (R-SQL-23). **Checked in the running app** by the owner, Linux under WSL (2026-11-14).
