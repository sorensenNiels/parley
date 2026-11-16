# Phase 18 — The work area, rearranged

> Finished phase of Parley SQL. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

Asked by the owner (2026-11-15), built before Phase 17: the side panel moves to the left and becomes resizable
(R-SQL-82); its Database tab becomes Libraries, where the operator sorts libraries into groups per connection, such as
Development and Testing (R-SQL-83); Snippets sits below Libraries (R-SQL-84); History becomes a tab beside Results and
Messages (R-SQL-85). The right side panel goes (D33).

**Status:** Done, verified (2026-11-15, Linux under WSL), built before Phase 17 at the owner's wish: the Libraries panel on the left (R-SQL-82), library groups (R-SQL-83), Snippets under Libraries (R-SQL-84), History beside Results and Messages (R-SQL-85); D33.

## Requirements

| ID | Requirement |
|---|---|
| R-SQL-82 | **The side panel on the left** (2026-11-15; the owner, D33). The side panel moves from the right to the left edge of the window and holds Libraries (R-SQL-83) above Snippets (R-SQL-84); History moves under the editor (R-SQL-85). Dragging its right edge changes its width (at least 200 points, at most half the window); double-clicking the edge sets the default 280 again. A button at its top right, "Hide the side panel", folds it to a narrow strip at the window's left edge with one button, "Show Libraries and Snippets", which opens it again at the width it had. Ctrl+B does both and is listed, and can be changed, on the Shortcuts page (R-SQL-60). The width and whether the panel is folded are kept in the settings between runs. *Changes R-SQL-51 (right edge; folded only while the window is open), R-SQL-25 and R-SQL-34 (tabs beside History).* |
| R-SQL-83 | **Library groups** (2026-11-15; the owner, D33). The Database tab becomes the Libraries section, a tree whose root is the IBM i: its relational database name (`CURRENT SERVER`, by default its system name, read with the library list) or, until that is read, the host; the hover gives the host and release. A right-click on the root opens the IBM i's menu (New group…, Read the catalog again), where actions on the system itself go later (the owner, 2026-11-15). Under the root, per saved connection, comes first the group "Library list" (the SQL job's library list as R-SQL-32 reads it; it cannot be changed), then the operator's groups (e.g. Development, Testing) in their order. Each group folds open and closed, and this is kept. Each library in a group opens to its objects as in R-SQL-34. "New group…" at the top of Libraries asks for a name, which must be given and be unique within the connection. A group's menu (right-click, or its menu button) has "Add library…" (a library by its system name, checked to be a valid name as pinning did; not looked up on the host), Rename…, Move up, Move down and Delete (asks first; removes the group only, never anything on the host). A library's menu, in any group, has "Add to group ▸" with the groups it is not yet in, and "Remove from group" in the operator's groups. A library may be in several groups. A group only organises: it never changes the job's library list or current schema. The search field filters across every group. Groups are kept in the settings per saved connection and follow a renamed connection (R-SQL-46). The first time, the libraries pinned before become a group "Pinned", which can be renamed or deleted. Completion and hover (R-SQL-36, R-SQL-37) know the libraries of every group, as they knew the pinned ones. *Replaces R-SQL-34's pinned libraries.* |
| R-SQL-84 | **Snippets under Libraries** (2026-11-15; the owner, D33). Below Libraries in the side panel, a Snippets section shows the Snippets panel of R-SQL-25, otherwise unchanged. Each section has a header with its name that folds it; a folded section leaves only its header and gives the other the room. While both are open, the divider between them can be dragged (each keeps at least 120 points). The split and which sections are folded are kept in the settings. |
| R-SQL-85 | **History beside Results and Messages** (2026-11-15; the owner, D33). Below the editor (or beside it, R-SQL-49) the tabs are Results, Messages and History. History lists the statements run, kept as R-SQL-7 says, newest first; one click puts a statement back in the editor, as before. Running a statement shows Results or Messages as before; History shows only when chosen. *Changes R-SQL-7 (the history listed beside the editor).* *Changed 2026-11-15 by R-SQL-91 (a click selects; double-click or Enter adds the statement).* |

## Verification

- **R-SQL-82..85** (2026-11-15): the work area, rearranged (Phase 18). **Automated** (`make test`): the panel
  on the left folds and opens by its buttons and Ctrl+B and keeps its edge with a long name open (R-SQL-82); groups
  named once ignoring case, renamed, moved, libraries in several groups, `æøå` and bad names refused, pinned libraries
  becoming "Pinned" in the settings and following a rename, the IBM i as the tree's root (R-SQL-83); the sections'
  heights within their limits, folding on their own (R-SQL-84); History as a tab putting a statement back (R-SQL-85).
  **Checked in the running app** by the owner, Linux under WSL
  (2026-11-15).
