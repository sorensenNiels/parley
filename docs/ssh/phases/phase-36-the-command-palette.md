# Phase 36 — The command palette

> Finished phase of Parley SSH. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

One box (Ctrl+Shift+P) for every host, session, tab and action. R-SSH-48.

**Status:** Done (2026-11-20), checked by the owner on Windows, macOS, Linux and WSL: R-SSH-48.

## Requirements

| ID | Requirement |
|---|---|
| R-SSH-48 | **The command palette** (2026-11-16). Ctrl+Shift+P (Cmd+Shift+P) opens a box over the window that finds, as one types and forgiving typos, saved sessions, `~/.ssh/config` hosts, open tabs, recent connections and every menu action, each with its shortcut; Enter runs the one chosen; recent ones first. |

## Verification

- **R-SSH-48** (built 2026-11-16, Linux under WSL). **Automated** (`make test`): `parley-ssh-ui`'s `palette.rs` (words, letters in order, a swapped pair and two typos in a long query found, none in three letters, æøå; recent first, then the best fit, the detail searched; each recent key once, at most 20), `tests.rs` (Ctrl+Shift+P, "setings" running Settings and listed first next time, snippets and shortcuts listed; a saved session connecting in the start screen, then in a new tab, kept once as a recent connection without its name).
  - R-SSH-48: Ctrl+Shift+P (Cmd+Shift+P): "Go to…" over the window finds open tabs, saved sessions, `~/.ssh/config` hosts, the last ten connections, snippets and every menu action, each with its shortcut; matches whole words, letters in order and a typo (from four letters); arrows and Enter or a click; the 20 chosen last first (`palette_recent`, kept in the settings). A session, host or recent connection opens in the start screen in front, else in a new tab. Checked: Many sessions and config hosts; typing fast; macOS's Cmd+Shift+P; light and dark.
  **Checked in the running app** by the owner on Windows, macOS, Linux and WSL (2026-11-20).
