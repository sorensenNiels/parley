# Phase 28 — The terminal, lean

> Finished phase of Parley SSH. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

Cut down by D46 to what modern use needs: the mouse for vim, tmux and htop (R-SSH-18), find in the scrollback
(R-SSH-24), a simple bell (R-SSH-19) and session logs (R-SSH-25). Appearance moves to Phase 37 (R-SSH-20). Dropped:
PuTTY's keyboard styles (R-SSH-17), selection options (R-SSH-21), character sets other than UTF-8 (R-SSH-22) and its
terminal options (R-SSH-23).

**Status:** Done (2026-11-20), checked by the owner on Windows, macOS, Linux and WSL: R-SSH-18, R-SSH-19, R-SSH-24, R-SSH-25. Dropped (D46): R-SSH-17, R-SSH-21, R-SSH-22, R-SSH-23.

## Requirements

| ID | Requirement |
|---|---|
| R-SSH-18 | **The mouse for remote programs** (xterm's mouse tracking): modes 9, 1000, 1002 and 1003 with the default, UTF-8 (1005) and SGR (1006) encodings; a session setting turns it off; Shift always selects locally. |
| R-SSH-19 | **The bell** (*made lean 2026-11-16, D46*): none, a sound (as D37's beep, played by the launcher) or a flash of the pane's border (default: flash); while the window is not focused, its taskbar entry asks for attention. |
| R-SSH-24 | **Find in the scrollback** (Ctrl+Shift+F): the find bar of Parley 5250 (`terminal/find`), over the screen and the scrollback, with match case and next/previous. |
| R-SSH-25 | **Session logs** (*made lean 2026-11-16, D46*): per session, off by default: the text as shown (without escape sequences) or the raw output, to a file named from a pattern with `&H` host, `&Y&M&D` date, `&T` time, appended to; Session ▸ Start/Stop log for the session open. |
| R-SSH-17 | ~~**Keyboard settings** (PuTTY's Keyboard and Features pages): Backspace sends `DEL` or `^H`; Home/End as standard or rxvt; function keys as xterm, ESC[n~, Linux, Xterm R6, VT400, VT100+ or SCO; the initial state of the cursor keys and keypad; a switch to refuse application cursor/keypad mode; Alt as `ESC` prefix or as Meta; AltGr as AltGr or as Ctrl+Alt.~~ *Dropped 2026-11-16 (D46): legacy options; UTF-8 and xterm's keys and behaviour only.* |
| R-SSH-21 | ~~**Selection settings** (PuTTY's Selection page): copy on select with a right or middle click pasting (PuTTY's way) or the right-click menu (R-SSH-9); rectangular selection with Alt+drag; which characters make a word for a double click; copy with colours (HTML and RTF beside the text).~~ *Dropped 2026-11-16 (D46): legacy options; UTF-8 and xterm's keys and behaviour only.* |
| R-SSH-22 | ~~**Character sets** (PuTTY's Translation page): the remote character set, UTF-8 by default, or ISO 8859-1..16, CP437, CP850, CP1252 or KOI8-R; ambiguous-width characters narrow or wide; line drawing as Unicode or as the font's.~~ *Dropped 2026-11-16 (D46): legacy options; UTF-8 and xterm's keys and behaviour only.* |
| R-SSH-23 | ~~**Terminal options** (PuTTY's Terminal page): auto-wrap on at start, implicit CR in every LF and LF in every CR, the answerback to `^E`, whether the host may set the window title, and whether it may resize the window.~~ *Dropped 2026-11-16 (D46): legacy options; UTF-8 and xterm's keys and behaviour only.* |

## Verification

- **R-SSH-18** (2026-11-16). **Automated** (`make test`): `parley-ssh-core`'s `mouse.rs` (what each tracking mode reports, the three encodings and their
limits, modifiers, resetting the modes), `screen.rs` (kittest): a drag and the wheel going to a host in modes 1002 and
1006, Shift and the session setting keeping the mouse for selecting.
  - R-SSH-18: Modes 9, 1000, 1002, 1003 with the default, UTF-8 (1005) and SGR (1006) encodings: presses, releases, drags, moves and the wheel go to the host; Shift selects locally; the session editor's Terminal switch turns it off.
  **Checked in the running app** by the owner on Windows, macOS, Linux and WSL (2026-11-16).

- **R-SSH-19, R-SSH-24, R-SSH-25** (R-SSH-24 built 2026-11-16, R-SSH-19 and R-SSH-25 2026-11-17, Linux under WSL). **Automated** (`make test`): `parley-ssh-core`'s `term/find.rs` (matches in the scrollback and on the screen with and without case, not overlapping, wide characters and æøå as cells; a match scrolled into view; the change counter), `parley-ssh-ui`'s `find.rs` (the current match scrolled to, only those in view given, Match case), `session.rs` (the bar over a pane: "1 of 2", Match case to "1 of 1", Close clearing the highlights), `window_keys.rs` (Ctrl+Shift+F, Ctrl+F still the host's). R-SSH-19 and R-SSH-25: `parley-ssh-core`'s `term/plain.rs` (escape sequences, carriage returns and strings left out, also cut in two by the network, a UTF-8 character too; recording only while asked), `parley-ssh-ui`'s `log.rs` (a file named from its pattern with `~`; a text log and a raw one appended to the same file), `session.rs` (a log keeps only what came while it was on; the bell noticed).
  While building: the snippet picker's "No snippets yet" hint used `▸`, which the bundled fonts lack; opening the picker without snippets would have crashed (Phase 38). Fixed.
  - R-SSH-19: Settings ▸ Terminal ▸ Bell: Flash (default: the pane's border, 0.2 s), Sound (the launcher's beep, D37) or None; while the window is not focused the taskbar asks for attention; bells from tabs not in front too. Checked: `printf '\a'`; tab completion with nothing to complete in bash; each setting; Windows' and macOS's taskbar or dock.
  - R-SSH-25: Session editor ▸ On the server ▸ Session log: Off, Text (without escape sequences) or Raw output, to a file from a pattern (`~/parley-ssh-logs/&H-&Y&M&D-&T.log`), appended to with a line saying when it started; started when the shell starts; Session ▸ Start log / Stop log for the session in front (text when the session has none); an error said in the status bar and the log stopped; kept across reconnects. Checked: A text log of vim and htop (readable?), a raw log replayed with `cat`; a folder that cannot be written; Windows paths.
  - R-SSH-24: Edit ▸ Find… and Ctrl+Shift+F (Cmd+F on macOS) open Parley 5250's find bar over the pane, with Match case; Enter and Shift+Enter or the arrows go to the next and previous match, scrolled into view; matches in view are highlighted in the theme's yellow, the current one solid; Escape or 🗙 closes. Searched again only when the text, Match case or the output changed. Checked: `dmesg` or a long `journalctl` with 10,000 lines of scrollback; matches while output keeps coming; each theme's highlight; Escape giving the keyboard back to the shell.
  **Checked in the running app** by the owner on Windows, macOS, Linux and WSL (2026-11-20).
