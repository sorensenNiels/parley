# Phase 14 — The editor, deeper

> Finished phase of Parley SQL. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

From IDEA-34 (the owner, 2026-11-14), in this order: the gutter (R-SQL-61, R-SQL-62), completion (R-SQL-63,
R-SQL-64), a routine's parameters (R-SQL-65), the word under the cursor and Ctrl+D (R-SQL-66, R-SQL-67), trailing
blanks (R-SQL-68). The editor stays egui's text field; the gutter is drawn beside it from the rows it laid out
(D28). Folding and several cursors, which need an editor of Parley's own, stay in IDEA-34.

**Status:** Done, verified (2026-11-14, Linux under WSL): from IDEA-34 (the owner): on egui's text field with a gutter of Parley's own (D28): line numbers (R-SQL-61), a ▶ and the last outcome per statement (R-SQL-62), completion on demand and with snippets (R-SQL-63, R-SQL-64), a routine's parameters while it is called (R-SQL-65), the word under the cursor marked (R-SQL-66), Ctrl+D (R-SQL-67), trailing blanks trimmed on save (R-SQL-68).

## Requirements

| ID | Requirement |
|---|---|
| R-SQL-61 | **Line numbers** (2026-11-14; D28). A gutter left of the editor's text numbers its lines; a line that wraps has its number on its first row only. The cursor's line has its number in the text colour, the others muted. The gutter is as wide as the largest number needs, grows with the text size (R-SQL-59), and scrolls with the text. |
| R-SQL-62 | **Run a statement from the gutter, and its outcome** (2026-11-14; D28). In the gutter, the first line of each statement has a ▶ (faint, in the text colour on the cursor's statement, in the accent colour under the pointer, named "Run the statement on line *n*" for screen readers); clicking it runs that statement as Run does (R-SQL-7, the guard of R-SQL-9 included). After a statement ran, its first line shows the outcome while its text is unchanged: a tick for rows read or changed, a red dot for an error; hovering it says the rows (and "more to fetch"), the rows changed or "done", the time it took, or the error. A statement that failed also has its line number in the error colour. The outcomes last while the window is open. |
| R-SQL-63 | **Completion when asked** (2026-11-14; *adds to R-SQL-36*). Ctrl+Space (a shortcut of R-SQL-60) opens the completion list at the cursor at any time, also with nothing or one letter typed, and after a `.`. The list puts what the statement's tables have first, then objects and libraries, then snippets, then keywords; keywords are always offered (up to ten), however many names come first. |
| R-SQL-64 | **Snippets in the completion list** (2026-11-14). The list offers the snippets (R-SQL-24) whose title or a tag has a word starting with what is typed, with the detail "snippet"; choosing one replaces what was typed with the snippet, asking for its placeholders first when it has any (R-SQL-26). |
| R-SQL-65 | **A routine's parameters while it is called** (2026-11-14). While the cursor is inside the brackets after `CALL name(` or `name(` of a procedure or function in the catalog (qualified, or in the current schema), a small box above the cursor's line shows its parameters from `QSYS2.SYSPARMS`: mode (IN, OUT, INOUT), name and type, the one the cursor is at (counted by commas outside brackets and strings) in bold; a routine with several signatures shows each (up to three) with the count of parameters. It is read on the catalog connection (R-SQL-33) once per routine and window; while it is read, or for a name not in the catalog, nothing is shown. Esc hides the box until the cursor leaves the brackets. |
| R-SQL-66 | **The word under the cursor** (2026-11-14). Without a selection, when the cursor is on a name (not in a string or comment) that appears again in the script, every place of it is drawn on a faint background; unquoted names match in any case, quoted ones exactly, and only whole names. Not for scripts too long to colour (R-SQL-19). |
| R-SQL-67 | **Select the next one** (2026-11-14). Ctrl+D (a shortcut of R-SQL-60) selects the name under the cursor when nothing is selected; with a selection, it selects the next place of the same text after it (from the top again after the last), ignoring case, and shows it. |
| R-SQL-68 | **Trailing blanks trimmed on save** (2026-11-14). Save and Save as… take spaces and tabs off the ends of lines before writing, in the script too; the cursor stays on its line. Text inside a string that spans lines is left as it is. |

## Verification

- **R-SQL-61..68** (2026-11-14): the editor, deeper (Phase 14, D28), built. **Automated** (`make test`): in
  `parley-core` the word under the cursor and its places (quoted, unquoted, æøå, not in strings), Ctrl+D's next place
  with wrapping, trailing blanks trimmed but not inside a string, the routine call and argument at the cursor
  (nested, quoted, keywords and strings left out), the `SYSPARMS` query and its signatures; in the UI the outcomes kept
  by statement text and said on hover, a statement's first line, the ▶ that selects and runs its statement, the
  outcome of a run and of an error, Ctrl+Space and a list that stays open while typing, a snippet chosen from the list,
  keywords kept in a long list, Ctrl+D, Save trimming the file and the script, the word's places on a background, the
  call found and Esc hiding its box. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).
