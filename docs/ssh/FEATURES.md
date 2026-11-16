# Parley SSH features

> Checks of Parley SSH requirements still in progress, by requirement ID. The requirements are in
> [ROADMAP.md](ROADMAP.md). Add an entry only when the requirement is fully met, has automated tests where practical,
> and has been checked in the running app on at least one OS; note which OSes were checked. Then move the
> requirement's row from the roadmap, and this entry, to its phase's file in [phases/](phases/), with a row in
> [REQUIREMENTS.md](REQUIREMENTS.md).

## In progress

### Phase 26: a first shell (built 2026-11-16, Linux under WSL)

Automated (all pass): `cargo test -p parley-ssh-core -p parley-ssh-ui`: the terminal byte for byte
(`term/tests.rs`), keys and paste (`keys.rs`, `screen/input.rs`), known hosts (`known_hosts.rs`), sign-on order
(`net/sign_on.rs`), every dialog's buttons staying put (`dialogs.rs`), the start screen and header (`lib.rs`), and a
whole session against an in-process russh server (`tests/net.rs`): host key asked and saved, a wrong password asked
again, pty `xterm-256color` 80×24 with UTF-8, output in the terminal and its DA reply to the server, resize, typing,
exit status 3; a saved key not asked again; a changed key refused on Cancel; Cancel on the password; a refused port.
Seen: the start screen in a `--screenshot` run (dark theme).

| ID | Built | Still to check in the running app |
|---|---|---|
| R-SSH-1 | Crates, `parley-ssh` binary, `make run-ssh`/`test-ssh`, `cargo deny` wrappers, packager entries (binary, `.desktop`, Start menu shortcut, PATH), `parley --ssh` and a `parley-ssh` link to the AppImage (`siblings.rs`), the release workflow starting all three binaries. Checked 2026-11-16 on Linux under WSL: the `.deb` holds `/usr/bin/parley-ssh` and `parley-ssh.desktop`; the AppImage answers `--ssh --version` and, through a link named `parley-ssh`, `--version`. | The `.msi` (Start menu shortcut, `parley-ssh` in a new terminal) and the `.dmg` (link into `/usr/local/bin`). `cargo deny check` fails on RUSTSEC-2023-0071 (russh's `rsa 0.10.0-rc`): waiting for the owner's choice. |
| R-SSH-2 | Algorithms as listed; strict kex; plain-words errors; `user@host:port` in the host field. | Connect to OpenSSH (Linux, macOS, Windows) and to IBM i's OpenSSH; a server with nothing in common. |
| R-SSH-3 | Own `known_hosts` beside the settings, OpenSSH's read (hashed too), new/changed dialogs. | An unknown key, Accept and save, connect again; change a key and see the warning. |
| R-SSH-4 | Keyboard-interactive then password, three tries each, banner, `Secret`. Not done: the change-password request (russh cannot receive it). | A server with a banner; a one-time code over keyboard-interactive. |
| R-SSH-5 | pty `xterm-256color`, `window-change`, exit status and signal in the status bar, Restart (Ctrl+Shift+R). | `exit 3`, `kill -9 $$`, Restart. |
| R-SSH-6 | The xterm emulation. | vim, less, htop, tmux, `mc`, colours (`msgcat --color=test`), wide characters and emoji. |
| R-SSH-7 | Keys as xterm; Ctrl+C/V/X to the host; Ctrl+Shift shortcuts; shortcut sheet. | Ctrl+C stops `sleep 100`; F-keys in `mc`; Alt+b in bash; AltGr on a Danish keyboard on Windows. |
| R-SSH-8 | JetBrains Mono, block cursor blinking, dark and light ANSI colours, 10,000-line scrollback, wheel and Shift+Page Up/Down. Not done: italic (no italic face bundled). | Scroll back during output; light theme. |
| R-SSH-9 | Drag, double and triple click, copy with wrapped lines joined, paste with bracketed mode, the multi-line question, the right-click menu. | Paste several lines into bash (bracketed) and into `cat` (asks). |
| R-SSH-10 | Header (Session, Edit, View, Help, pill, gear), status bar with size and the algorithms on hover, Settings (Appearance, Terminal, About), the close question. New session waits for R-SSH-15 (Phase 27). | Light and dark; close while connected. |

### Phase 27: saved sessions and keys (built 2026-11-16, Linux under WSL)

Automated (all pass): `tests/net.rs` against the in-process russh server: an OpenSSH key file signs on with no
prompt; an encrypted key asks its passphrase, again when wrong; a PuTTY `.ppk` v3 key signs on; a key file that
cannot be read is said (`Event::Problem`) and the password still works; russh's own agent on a Unix socket signs on and,
forwarded, answers the server's request for its keys; an agent key not named is not used and forwarding stays off.
`sessions.rs`, `start.rs` and `tests.rs` (kittest): a session saved from the editor and listed, edit and delete (asked
twice), the defaults, search, the command line's request, New and Duplicate asking for windows; `ssh_start.rs` the
arguments; `tests/stored_settings.rs` settings saved before sessions still load and sessions read back. Seen: the start
screen in a `--screenshot` run, with the settings Phase 26 saved.

| ID | Built | Still to check in the running app |
|---|---|---|
| R-SSH-11 | Saved sessions in the settings, "Default Settings…", the editor (Name, Host, Port, User name, Key files, agent switches), search, quick connect. | Save, connect, edit, delete; restart Parley SSH and see them again. |
| R-SSH-12 | Key files tried after the agent's keys: OpenSSH, PKCS #8, `.ppk` v2/v3; passphrase each time, three tries; keys wiped when dropped (ssh-key). | An encrypted OpenSSH key and a PuTTYgen key against a real server. |
| R-SSH-13 | `SSH_AUTH_SOCK`; on Windows OpenSSH's agent pipe, then Pageant; one key by fingerprint; forwarding off by default, with a warning when turned on. | ssh-agent on Linux/macOS, the Windows OpenSSH agent and Pageant on Windows; `ssh-add -l` on the server with forwarding. |
| R-SSH-14 | `parley-ssh [user@]host[:port] -p -l -i -load -A -a`, wrong arguments with usage and exit 2. | `parley-ssh -load NAME` and `parley-ssh me@host -i key` in a terminal. |
| R-SSH-15 | Session ▸ New session (Ctrl+Shift+N) and Duplicate session (Ctrl+Shift+D) start `parley-ssh` again: `-load NAME` for a saved session, else its host and options. | Both, on each OS. |
| R-SSH-16 | Session ▸ Event log: every line with its time, Copy, never a password or key. | Read it after a connection; copy it. |

### Phase 33: tabs and split panes (built 2026-11-16, Linux under WSL)

Automated (all pass): `panes.rs` the layout (split, close, rectangles, Alt+arrow neighbours, dragging a divider),
`window_keys.rs` the window's shortcuts (taken before the terminal; Ctrl+digits and Ctrl+letters stay the host's;
Alt+arrows only in a split tab; Cmd on macOS), `tabs.rs` a tab's panes, `tests.rs` (kittest): new tab and the bar
from two tabs, Ctrl+Tab and Ctrl+Alt+2, close without asking for a start screen, the last tab leaving a start screen,
splitting right and down, Alt+Left, closing the pane in front, a connected tab asking before it closes, Duplicate in a
tab and New window through the launcher. Not yet seen in the running app.

| ID | Built | Still to check in the running app |
|---|---|---|
| R-SSH-39 | Tabs with title, state dot, close button, middle click, drag to move, + for a new tab; Ctrl+Shift+T/W, Ctrl+Tab, Ctrl+Alt+1…9; the header, status bar, event log and Edit menu follow the tab in front. | The tab bar's look in light and dark; dragging tabs; closing a connected tab asks. |
| R-SSH-40 | Panes split right and down (Alt+Shift+= and -, Session ▸ Split), each its own connection and size, Alt+arrows, dividers dragged, an accent border on the pane in front; a pane's questions wait until it is in front. | Two sessions side by side with vim and htop; resize by the divider; host-key and password questions in a pane not in front. |
| R-SSH-41 | New session and New tab (Ctrl+Shift+N/T) open a tab; Duplicate (Ctrl+Shift+D) the session in a tab; New window (Ctrl+Shift+Alt+N) another `parley-ssh`; the command line's session is the first tab. | New window on each OS. |
