# Phase 33 — Tabs and split panes

> Finished phase of Parley SSH. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

From IDEA-37 (D47): several sessions in one window, as tabs and as panes side by side or one over the other, with the
keys people know from Windows Terminal and iTerm2. R-SSH-39..41.

**Status:** Done (2026-11-16), checked by the owner on Windows, macOS, Linux and WSL: R-SSH-39, R-SSH-40, R-SSH-41, R-SSH-58.

## Requirements

| ID | Requirement |
|---|---|
| R-SSH-39 | **Tabs** (2026-11-16, IDEA-37, D47). Each session is a tab under the header, with its title (what the host set, else `user@host`), a state dot, and a close button; the tab bar shows once there are two tabs. New tab (Ctrl+Shift+T) opens the start screen in a tab; Close tab (Ctrl+Shift+W) asks first while connected (R-SSH-10's setting); Ctrl+Tab and Ctrl+Shift+Tab go to the next and previous tab, Ctrl+Alt+1…9 to a tab by number (Cmd+1…9 on macOS; *changed 2026-11-16 while building: Ctrl+2…8 are control characters for the host, so Windows Terminal's Ctrl+Alt*); a middle click closes a tab; tabs are reordered by dragging. The header's pill, the status bar, the event log and the Edit menu follow the tab in front. Closing the last tab shows the start screen. |
| R-SSH-40 | **Split panes** (2026-11-16, IDEA-37, D47). A tab holds one or more panes: Split right (Alt+Shift+=) and Split down (Alt+Shift+-), as Windows Terminal, open the same session's host in a new pane beside or below the one in front; Alt+arrow moves to the pane in that direction; the divider is dragged to resize; Close pane (Ctrl+Shift+W in a split tab) closes the pane in front. Each pane has its own connection, terminal and size; the one in front has a thin accent border. |
| R-SSH-41 | **New, Duplicate and windows with tabs** (2026-11-16, D47; *changes R-SSH-15*). New session (Ctrl+Shift+N) opens a new tab; Duplicate session (Ctrl+Shift+D) opens the session in front again in a new tab; New window (Ctrl+Shift+Alt+N) starts another `parley-ssh` as R-SSH-15 did. A session opened from the command line (R-SSH-14) is the first tab. |
| R-SSH-58 | **A split pane that fails, and closing a pane** (2026-11-16; *adds to R-SSH-40*). A pane opened by a split, or connected from a start screen in a split tab, is taken out again when its session ends before the shell starts (it cannot connect, the host key is refused, signing on fails or is cancelled); the pane it was split from comes back to the front and the status bar says why. A tab's only pane stays, to show why. Each pane of a split tab has a close button (🗙) in its top right corner, which closes it as Close pane does, asking first while connected. |

## Verification

- **R-SSH-39, R-SSH-40, R-SSH-41, R-SSH-58** (2026-11-16). **Automated** (`make test`): `panes.rs` the layout (split, close, rectangles, Alt+arrow neighbours, dragging a divider),
`window_keys.rs` the window's shortcuts (taken before the terminal; Ctrl+digits and Ctrl+letters stay the host's;
Alt+arrows only in a split tab; Cmd on macOS), `tabs.rs` a tab's panes, `tests.rs` (kittest): new tab and the bar
from two tabs, Ctrl+Tab and Ctrl+Alt+2, close without asking for a start screen, the last tab leaving a start screen,
splitting right and down, Alt+Left, closing the pane in front, a connected tab asking before it closes, Duplicate in a
tab and New window through the launcher, a split pane that cannot connect taken out and one still connecting kept,
the pane close buttons (R-SSH-58).
  - R-SSH-39: Tabs with title, state dot, close button, middle click, drag to move, + for a new tab; Ctrl+Shift+T/W, Ctrl+Tab, Ctrl+Alt+1…9; the header, status bar, event log and Edit menu follow the tab in front.
  - R-SSH-40: Panes split right and down (Alt+Shift+= and -, Session ▸ Split), each its own connection and size, Alt+arrows, dividers dragged, an accent border on the pane in front; a pane's questions wait until it is in front.
  - R-SSH-41: New session and New tab (Ctrl+Shift+N/T) open a tab; Duplicate (Ctrl+Shift+D) the session in a tab; New window (Ctrl+Shift+Alt+N) another `parley-ssh`; the command line's session is the first tab.
  - R-SSH-58: A split pane whose session ends before the shell starts is taken out, the pane it came from back in front, the reason in the status bar; a close button on each pane of a split tab.
  **Checked in the running app** by the owner on Windows, macOS, Linux and WSL (2026-11-16).
