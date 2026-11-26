# Ideas: Snippets

> One theme of [../IDEAS.md](../IDEAS.md); the next free number is on the last line of [../../IDEAS.md](../../IDEAS.md).
> Claude's suggestions, 2026-11-25, after the owner found adding snippets not intuitive and asked for running a snippet
> with values put into its SQL. Builds on snippets (R-SQL-24), the panel (R-SQL-25, R-SQL-84, R-SQL-88), placeholders
> (R-SQL-26) and the completion list (R-SQL-64).
>
> Where it stands: a snippet is saved only from the bookmark button in the run bar ("Save as snippet…", what Run would
> run) or from a History row's menu; the Snippets section's header has no "New". Placeholders (`${name}`,
> `${name:default}`) already work: a form asks for each when a snippet is inserted or run; the only mention of them is a
> hint line in the save dialog, and the values are remembered only until Parley closes.

## Make adding a snippet easy to find

| ID | Idea |
|---|---|
| IDEA-61 | **A "New snippet" button in the Snippets header** (the `+` the Explorer has for groups), opening the snippet dialog with the editor's selection, or the statement under the cursor, or empty; and when there are no snippets of one's own, a line in the empty "Mine" group: "Save a statement as a snippet: select it and press Ctrl+Shift+S, or click +". *→ R-SQL-106* |
| IDEA-62 | **"Save as snippet…" everywhere a statement is:** the editor's right-click menu, a result tab's menu (the SQL that made it), the schema browser's generated SQL, and a shortcut on the Shortcuts page (R-SQL-60). *→ R-SQL-107 (editor and shortcut; a result tab's menu waits, as a tab does not keep its SQL)* |
| IDEA-63 | **Drag to save:** selected text dropped on the Snippets section opens the save dialog with it; a snippet dragged into the editor inserts it where it is dropped. *→ R-SQL-118 (a snippet into the editor; text dragged out of the editor is not possible with egui's text field)* |
| IDEA-64 | **A title guessed for you:** the save dialog suggests a title and tags from the statement (`SELECT … FROM QSYS2.ACTIVE_JOB_INFO` → "Active job info", tag "qsys2"), so Save is one click; the assistant (D25) may write the about line when it is on. *→ R-SQL-119 (title and tags; the assistant's about line not yet)* |
| IDEA-65 | **Turn literals into placeholders while saving:** the save dialog lists the statement's literals (`'QSYSOPR'`, `100`, `CURRENT DATE - 7 DAYS`) with a switch each; switched on, the literal becomes `${name:literal}` with a name taken from the column it is compared with (`WHERE USER_NAME = 'QSYSOPR'` → `${user_name:QSYSOPR}`). This is the step from "a query I ran" to "a query I reuse" without knowing the `${…}` syntax. *→ R-SQL-111* |
| IDEA-66 | **Make a placeholder from the editor:** select a value in the editor, "Make placeholder…" asks a name and replaces it with `${name:value}`; placeholders are coloured in the editor so they stand out. |
| IDEA-67 | **The save dialog shows the placeholders it found** as a live list under the SQL ("Asks for: user_name (QSYSOPR), days (7)"), so a typo in `${…}` is seen before saving. *→ R-SQL-108* |

## Run a snippet with values

| ID | Idea |
|---|---|
| IDEA-68 | **Placeholders in any statement, not only snippets:** running SQL from the editor that holds `${…}` asks for the values with the same form (R-SQL-26), so a script can be tried before it is saved. Needs care beside parameter markers (`?`, R-SQL-40): `${…}` is put into the text, `?` is bound. *→ R-SQL-120 (with a setting, on at first, and Run as written)* |
| IDEA-69 | **Typed placeholders that write the quotes:** `${user:text}`, `${days:number}`, `${from:date}`, `${lib:name}`; a text value is quoted and its `'` doubled, a number checked, a date picked from a calendar, a name upper-cased; untyped ones go in as typed, as today. Removes the most common mistake (forgetting quotes) and closes the injection-by-accident hole a pasted `O'Brien` opens. *→ R-SQL-110 (written `${name as text}`; lists and choices not yet)* |
| IDEA-70 | **Values picked from the host:** `${lib:library}`, `${file:table}`, `${user:user}`, `${job:job}` offer the catalog's names with the completion list (R-SQL-64) as one types, and `${x:choice(A,B,C)}` a fixed list. *→ R-SQL-115 (libraries and tables; users, jobs and choices not yet)* |
| IDEA-71 | **Lists:** `${users:list}` takes several values (one per line or comma-separated) and writes `'A', 'B', 'C'` for an `IN (…)`. *→ R-SQL-114* |
| IDEA-72 | **The values form shows the SQL as it will run** below the fields, filled as one types, and has "Run" and "Insert" both, whichever opened it. *→ R-SQL-109* |
| IDEA-73 | **Values remembered across restarts, per snippet and per connection:** the last few values of each placeholder are offered in a drop-down (today only the last value, and only until Parley closes); a placeholder can be marked to always ask empty. *→ R-SQL-113 (per name and connection; marking a placeholder to ask empty not yet)* |
| IDEA-74 | **Run a snippet from the keyboard:** a "Run snippet…" palette (fuzzy search, as the start screen finds, D64) opens the values form and runs, without leaving the editor; and typing a snippet's title in the completion list (R-SQL-64) with Tab inserts it with its placeholders as fields to Tab through, as code editors do, instead of the form. *→ R-SQL-116 (Run snippet; tabbing through placeholders in the editor not taken: the values form checks types, offers names and remembers values, which typing in the text would skip)* |
| IDEA-75 | **A snippet as a saved report:** a snippet can be marked to open its result in a new tab named after it (with the values in the title: "Active jobs — QSYSOPR"), and run again from the tab with other values. *→ R-SQL-117* |
