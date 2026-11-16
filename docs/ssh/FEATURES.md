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
