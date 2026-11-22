# Phase 41 — Many saved sessions

> Finished phase of Parley SSH. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

Hundreds of saved sessions on the start screen (the owner, 2026-11-20): favourites marked with a star and listed
first, the recent connections under them, and a search at the top that forgives a typo and is driven by the
keyboard; long lists fold and quick connect stays in view. R-SSH-63..65. Folders or tags may follow if this is not
enough.

**Status:** Done (2026-11-20), checked by the owner on Windows, macOS, Linux and WSL: R-SSH-63, R-SSH-64, R-SSH-65.

## Requirements

| ID | Requirement |
|---|---|
| R-SSH-63 | **Favourite sessions** (2026-11-20, asked by the owner). A star on each saved session's row on the start screen marks it as a favourite or not, kept with the session in the settings file; the favourites are listed first, under "Favourites", and also under "All sessions", which lists every saved session (the owner, 2026-11-20). The command palette (R-SSH-48) lists favourite sessions before the other saved sessions. A session made from the defaults or a config host is not a favourite. |
| R-SSH-64 | **Recent connections on the start screen** (2026-11-20, asked by the owner). Under the favourites, "Recent" lists the last five connections made (saved sessions, `~/.ssh/config` hosts and quick connects: R-SSH-48's list), newest first. A saved session (found by its name, or by its address for a connection kept without one) is shown by its name, with its star and Edit…, and connects with its settings as they are now (also from the palette); one since renamed or deleted, and any other connection, is shown by its address with "Save as session…". Not shown while searching. |
| R-SSH-65 | **A start screen for many sessions** (2026-11-20, asked by the owner). The search field (from four saved sessions and config hosts, R-SSH-11) is at the top and has the keyboard when a start screen opens in the pane in front. It looks in names, hosts, user names and jump hosts and forgives a typo as the palette does; the matches, saved sessions and config hosts, are one list, the best fits first; Up and Down choose one, Enter connects it, Escape empties the search. Each saved session or host is one line: star, name, address, Edit… (or Save as session…), Connect. Without a search, "All sessions" and "From ~/.ssh/config" show their count and fold under their heading. "All sessions" is folded until the user opens it, and stays as left, also after a restart (the owner, 2026-11-20); *changed 2026-11-22 (D64): open while nothing is listed above it;* "From ~/.ssh/config" with more than ten hosts starts folded when a section above it shows something. Quick connect stays in view: beside the list when the pane is wide enough, above it when not. |

## Verification

- **R-SSH-63, R-SSH-64, R-SSH-65** (R-SSH-63..65 built 2026-11-20, Linux under WSL). **Automated** (`make test`): `start`'s unit tests (the search over names, hosts, users and jump hosts, a typo forgiven, the best fit then the favourites first, `æøå`; what a click in each kind of row chooses), `palette`'s (a saved session kept recent by name, found by address when kept without one, connected as it is now; one renamed connects as made), and kittest tests in `tests.rs` (the star, the Favourites heading and the palette's order; Recent with "Not saved" and Save as session…; the search having the keyboard, Down and Enter connecting the second match; Escape; All sessions folded at first, opened and folded by a click and kept in the settings). Seen once in a screenshot of the running app with the owner's sessions (Linux under WSL, dark): Recent, All sessions and quick connect beside them.
  - R-SSH-63: `SavedSession::favourite`, the star in `start/list.rs` (since Phase 43 `parley-ui-egui`'s `connect/list.rs`, D64), `start::Choice::Favourite`, favourites first in `palette/items.rs`. Checked: Star and unstar; the favourite kept after a restart; the palette's order.
  - R-SSH-64: `remember_connection` keeps a saved session's name, `recent_saved`/`recent_source`, the Recent section, `Choice::Recent` and `RecentSave`. Checked: A quick connect and a config host in Recent, Save as session… from it; a saved session renamed.
  - R-SSH-65: `start::found` with `palette::score` (since Phase 43 `parley_ui_egui::fuzzy`, D64), the search's keys, `list::section` folding with counts, All sessions' state in `Settings::all_sessions_open`, quick connect beside the list from 1000 points wide and above it below that. Checked: Many sessions; Up, Down, Enter and Escape; All sessions as left after a restart; a narrow split pane; light theme.
  **Checked in the running app** by the owner on Windows, macOS, Linux and WSL (2026-11-20).
