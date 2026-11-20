# Phase 38 — Snippets and broadcast

> Finished phase of Parley SSH. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

From IDEA-39 and IDEA-40: commands kept by name and sent with a key, and typing into several panes at once.
R-SSH-49, R-SSH-50.

**Status:** Done (2026-11-20), checked by the owner on Windows, macOS, Linux and WSL: R-SSH-49, R-SSH-50.

## Requirements

| ID | Requirement |
|---|---|
| R-SSH-49 | **Snippets** (2026-11-16, IDEA-40; *details 2026-11-16, D50*). Commands kept by name in Parley SSH's settings and edited in Settings ▸ Snippets (add, edit, delete asked first, reorder; name, text of one or more lines, send Enter after it or not, a key of its own if wanted, caught as the Keyboard page catches keys); exported to and imported from one JSON file. A Snippets menu in the header lists them; Ctrl+Shift+S opens a picker that finds them as one types (name and text); a snippet's own key sends it at once. A snippet goes to the pane in front, as pasted text (bracketed when the host asked, R-SSH-9). Placeholders `{{name}}` and `{{name:default}}` are asked for in one dialog before sending, each name once; `{{{{` sends a literal `{{`. They go into the palette when Phase 36 is built. |
| R-SSH-50 | **Broadcast** (2026-11-16, IDEA-39). Type into several panes at once: Session ▸ Broadcast to this tab (Ctrl+Shift+B) or to every tab; keys and pastes typed in the pane in front go to every connected pane in scope; each pane that receives has a coloured border and a "Broadcast" badge, and the status bar says how many panes receive. Turned off from the menu again, or with Escape twice quickly. Snippets sent while broadcasting go to every pane too. |

## Verification

- **R-SSH-49, R-SSH-50** (built 2026-11-16, Linux under WSL; D50). **Automated** (`make test`): `parley-ssh-core`'s `snippet.rs` (placeholders once with their defaults, filled with answers then defaults, braces that are no placeholder and `{{{{` sent as written, æøå); `parley-ssh-ui`'s `snippets.rs` (export and import replacing by name, a file that is not one said, the picker's search, a name of its own and text needed, only a snippet with placeholders asks, the placeholder dialog sending its answers with its buttons staying put), `tests.rs` (Ctrl+Shift+B on and Escape twice off, Session ▸ Stop broadcasting, Broadcast to every tab; a snippet's own key without a session saying to connect; Ctrl+Shift+S finding by name and asking for a placeholder; the Snippets menu and Settings ▸ Snippets with a missing name said and the dialog keeping its size), `dialogs.rs` (the longer shortcut sheet scrolls in a small window).
  - R-SSH-49: `Settings::snippets`; Settings ▸ Snippets (add, edit, move, delete asked twice, a key caught as the Keyboard page does, Enter after it); Export… and Import… through the system's file window (`SnippetFileWindow`, one JSON file); the Snippets menu; the picker (Ctrl+Shift+S, Cmd+S on macOS); placeholders asked in one dialog; sent as a paste (bracketed when the host asked) then Enter. Checked: A snippet into bash with and without Enter; a multi-line one; placeholders with defaults; a snippet's own key; export, then import on another OS; the page and dialogs in light and dark.
  - R-SSH-50: Session ▸ Broadcast to this tab (Ctrl+Shift+B, Cmd+B on macOS) and to every tab, Stop broadcasting; keys and pastes typed in the pane in front go to every other session pane in scope, a paste encoded for each pane's own modes; a border and a "Broadcast" badge on each pane in scope; "Broadcasting to N panes" in the status bar; Escape twice within 0.6 s stops; snippets broadcast too. Mouse and focus reports stay with their own pane. Checked: Three panes typing `uptime` at once; a paste into panes where one has bracketed paste on; every tab; Escape twice; the marks in light and dark.
  **Checked in the running app** by the owner on Windows, macOS, Linux and WSL (2026-11-20).
