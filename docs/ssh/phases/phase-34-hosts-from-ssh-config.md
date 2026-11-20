# Phase 34 — Hosts from `~/.ssh/config`

> Finished phase of Parley SSH. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

From IDEA-38: the hosts in OpenSSH's config on the start screen and in the palette, as OpenSSH reads them, and jump
hosts (`ProxyJump`). R-SSH-42, R-SSH-43.

**Status:** Done (2026-11-20), checked by the owner on Windows, macOS, Linux and WSL: R-SSH-42, R-SSH-43.

## Requirements

| ID | Requirement |
|---|---|
| R-SSH-42 | **Hosts from `~/.ssh/config`** (2026-11-16, IDEA-38; *details 2026-11-16, the owner*). OpenSSH's config, read-only: `~/.ssh/config` (`%USERPROFILE%\.ssh\config` on Windows) and its `Include`s (relative to `~/.ssh`, `~` and wildcards in the file name, 16 levels deep as OpenSSH), not the system's `ssh_config`. Each name on a `Host` line without `*`, `?` or `!` is listed in a group "From ~/.ssh/config" on the start screen below the saved sessions, found by the same search; patterns are not listed but apply. A host's settings are OpenSSH's: for each keyword the first value of the blocks that match, in file order; `IdentityFile`s add up. Used: `HostName` (`%h`), `User`, `Port`, `IdentityFile` (`~`, `%d`, `%u`, `%h`, `%r`, `%%`), `ForwardAgent` (yes or no) and `ProxyJump` (R-SSH-43); what the config leaves out comes from the defaults (R-SSH-11), on the start screen the user name last from the local one. `Match` blocks are skipped. A card connects, or Save as session… opens the session editor with it, to keep it as a saved session. `parley-ssh NAME` on the command line (R-SSH-14) reads the config for NAME as `ssh NAME` does; `user@`, `-l`, `-p`, `-i`, `-A`/`-a` win over it; with no user anywhere, the host waits in quick connect (R-SSH-59). The files are checked for changes at most once a second while a start screen shows, and read again when one changed. A line that cannot be read (and a `Match` block) is skipped and said in a hint under the group; the hosts go into the palette with Phase 36. |
| R-SSH-43 | **Jump hosts** (2026-11-16; *moved here from R-SSH-29*). One or more jump hosts (`ProxyJump`, a session setting, and `-J` on the command line), reached through `direct-tcpip`, each with its own host-key check and sign-on. *Details 2026-11-16, while building:* written as OpenSSH writes them (`[user@]host[:port]` separated by commas, `ssh://` and `[…]` for IPv6 allowed, `none` for none), in the session editor's Jump hosts field and the defaults; a name that `~/.ssh/config` knows takes its `HostName`, `User`, `Port`, `IdentityFile`s and its own `ProxyJump` (followed 8 levels deep, then said as a loop); a config host's `ProxyJump` comes with it (R-SSH-42). A jump host with no user anywhere signs on as the session's user (OpenSSH uses the local one); it tries its `IdentityFile`s, then the session's key files and the agent's keys, and never forwards the agent. The host-key and sign-on dialogs name the hop that asks; only the target's algorithms go to the status bar, every hop's to the event log. A jump host that cannot reach the next says so; jump hosts that cannot be read are said by the editor and by `-J`, and a session whose jump hosts cannot be used fails at once with the reason. |

## Verification

- **R-SSH-42** (2026-11-16). **Automated** (`make test`): `parley-ssh-core`'s `openssh_config.rs` (names and patterns, first value wins, `IdentityFile`s
adding up, `%` tokens and `~`, `!` patterns, `Include` with wildcards and inside a `Host` block, 16 levels, `Match`
and bad lines skipped, quotes and `=`, a file that comes later noticed), `parley-ssh-ui`'s `openssh.rs` (the
defaults under a config host) and `tests.rs` (kittest): the command line reading the config with typed values
winning and R-SSH-59's quick connect kept, the group listed with a skipped line said, Save as session… saving it.
Seen in the running app (`--screenshot`, Linux under WSL, dark): the group with an `Include`d host, `%h`, a port and
the `Match` line skipped.
  - R-SSH-42: `~/.ssh/config` and its `Include`s read as OpenSSH reads them; its hosts in a group on the start screen, searched with the saved sessions; Connect, Save as session…; `parley-ssh NAME` reads the config; read again when a file changes.
  **Checked in the running app** by the owner on Windows, macOS, Linux and WSL (2026-11-16).

- **R-SSH-43** (R-SSH-43 built 2026-11-16, Linux under WSL). **Automated** (`make test`): `parley-ssh-core`'s `net/jump.rs` (OpenSSH's syntax with `ssh://`, IPv6 and `none`, what cannot be read, a config host's settings and its own `ProxyJump`, a loop said), `tests/net.rs` against in-process russh servers with `direct-tcpip` (two jump hosts: each hop's host key and password asked and named, only the target's `Secured`, each hop forwarding to the next; a jump host that cannot reach the target; a connection that never started); `parley-ssh-ui`'s `sessions.rs` (the session's user and key files for jumps, a bad field said by the editor and on connecting, `-J` for another window), `dialogs.rs` (a named prompt says its host), `openssh.rs` (a config host's `ProxyJump`); `parley-app`'s `ssh_start.rs` (`-J`).
  - R-SSH-43: `Options::jumps`, `net::jump::resolve`/`check`, each hop connected through the one before with its own host-key check and sign-on, the Jump hosts field in the session editor, `-J`, a config host's `ProxyJump`, prompts naming their hop. Checked: A session through a real bastion (`ProxyJump` in `~/.ssh/config` and typed in the editor); two jump hosts; a jump host that cannot reach the target; the editor's new field in light and dark.
  **Checked in the running app** by the owner on Windows, macOS, Linux and WSL (2026-11-20).
