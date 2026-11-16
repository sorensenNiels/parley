# Phase 28 — The terminal, lean

> Phase of Parley SSH still in progress: its finished requirements. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

Cut down by D46 to what modern use needs: the mouse for vim, tmux and htop (R-SSH-18), find in the scrollback
(R-SSH-24), a simple bell (R-SSH-19) and session logs (R-SSH-25). Appearance moves to Phase 37 (R-SSH-20). Dropped:
PuTTY's keyboard styles (R-SSH-17), selection options (R-SSH-21), character sets other than UTF-8 (R-SSH-22) and its
terminal options (R-SSH-23).

**Status:** In progress. Done (2026-11-16), checked by the owner on Windows, macOS, Linux and WSL: R-SSH-18. Still open in [../ROADMAP.md](../ROADMAP.md): R-SSH-19, R-SSH-24, R-SSH-25.

## Requirements

| ID | Requirement |
|---|---|
| R-SSH-18 | **The mouse for remote programs** (xterm's mouse tracking): modes 9, 1000, 1002 and 1003 with the default, UTF-8 (1005) and SGR (1006) encodings; a session setting turns it off; Shift always selects locally. |

## Verification

- **R-SSH-18** (2026-11-16). **Automated** (`make test`): `parley-ssh-core`'s `mouse.rs` (what each tracking mode reports, the three encodings and their
limits, modifiers, resetting the modes), `screen.rs` (kittest): a drag and the wheel going to a host in modes 1002 and
1006, Shift and the session setting keeping the mouse for selecting.
  - R-SSH-18: Modes 9, 1000, 1002, 1003 with the default, UTF-8 (1005) and SGR (1006) encodings: presses, releases, drags, moves and the wheel go to the host; Shift selects locally; the session editor's Terminal switch turns it off.
  **Checked in the running app** by the owner on Windows, macOS, Linux and WSL (2026-11-16).
