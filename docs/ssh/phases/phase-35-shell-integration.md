# Phase 35 — Shell integration

> Finished phase of Parley SSH. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

Links you can click, the shell's prompts as marks to jump between commands and copy a command's output, a
notification when a long command ends, and the clipboard from the server. R-SSH-44..47.

**Status:** Done (2026-11-20), checked by the owner on Windows, macOS, Linux and WSL: R-SSH-44, R-SSH-45, R-SSH-46, R-SSH-47.

## Requirements

| ID | Requirement |
|---|---|
| R-SSH-44 | **Links** (2026-11-16). OSC 8 hyperlinks and URLs in the text are underlined under the pointer; Ctrl+click (Cmd+click) opens them in the browser; the right-click menu copies one. |
| R-SSH-45 | **Commands as marks** (2026-11-16). With a shell that sends OSC 133 (bash, zsh, fish with their integration; PowerShell), prompts are marked in the scroll bar; Ctrl+Shift+Up and Down jump between commands; the right-click menu selects or copies a command's output; a failed command's mark is red. |
| R-SSH-46 | **Notifications** (2026-11-16). OSC 9 and OSC 777 notifications from the host, and a command that ran longer than 10 seconds ending (R-SSH-45), show as a desktop notification while the window is not focused. |
| R-SSH-47 | **The clipboard from the server** (2026-11-16). OSC 52: the host may set the clipboard (on by default, a setting turns it off); it may never read it. |

## Verification

- **R-SSH-44, R-SSH-45, R-SSH-46, R-SSH-47** (built 2026-11-16, Linux under WSL). **Automated** (`make test`): `parley-ssh-core`'s `term/integration.rs` (an OSC 8 link over its text and ended by an empty URI; written addresses without the punctuation after them, a balanced bracket kept; OSC 133 prompts, output and exit status; marks of dropped lines gone and none on the alternate screen; OSC 9 and 777 notifications but not ConEmu's numbered OSC 9; OSC 52 set, a query never answered), `term/find.rs` (a line put at the top); `parley-ssh-ui`'s `screen.rs` (Ctrl+click on a link opens it and sends nothing to the host), `screen/scrollbar.rs` (Ctrl+Shift+Up and Down from command to command; a command's output from any of its lines; a failed command's mark red), `session.rs` (a command over 10 s and the host's notifications become notes, a short one not; the clipboard set only when the setting allows).
  While building: the screen had no scroll bar although R-SSH-8 names one; a slim one at the right edge now scrolls by drag or click and carries R-SSH-45's marks. And eframe's `links` feature was off, so egui's links opened nothing on the desktop in all three programs (Parley 5250's addresses on the screen, the newer-version link): it is on now.
  - R-SSH-44: OSC 8 links and written `http(s)://`, `ftp://`, `file://` addresses: underlined under the pointer, Ctrl+click (Cmd+click) opens in the browser (logged), also while a program has the mouse; right-click ▸ Copy link. Checked: `ls --hyperlink=auto`, `gcc` errors with links, a URL in `git log`; Cmd+click on macOS; Windows' default browser.
  - R-SSH-45: OSC 133 A/C/D marks; each prompt marked on the scroll bar, red when its command failed; Ctrl+Shift+Up/Down (Cmd+Up/Down) to the previous and next command; right-click on a command ▸ Select command output, Copy command output. Checked: bash with `bash-preexec` or VS Code's/WezTerm's integration script, zsh, fish 4 (built in), PowerShell with its prompt marks; long output; the scroll bar in each theme.
  - R-SSH-46: OSC 9 and OSC 777;notify from the host, and a command (OSC 133) that ran 10 s or more ending, as a desktop notification (`notify-rust` in the launcher, on a thread) and the taskbar asking for attention, only while the window is not focused; from tabs not in front too. Checked: `sleep 12` in another window; `printf '\e]9;Build done\a'`; notifications on Windows, macOS and Linux (GNOME, KDE).
  - R-SSH-47: OSC 52 sets the clipboard (Settings ▸ Terminal: "Programs on the server may set the clipboard", on); never read; each time in the event log. Checked: tmux with `set -g set-clipboard on`, vim's OSC 52 yank, over a jump host; the setting off.
  **Checked in the running app** by the owner on Windows, macOS, Linux and WSL (2026-11-20).
