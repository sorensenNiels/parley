# Phase 34 — Hosts from `~/.ssh/config`

> Phase of Parley SSH still in progress: its finished requirements. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

From IDEA-38: the hosts in OpenSSH's config on the start screen and in the palette, as OpenSSH reads them, and jump
hosts (`ProxyJump`). R-SSH-42, R-SSH-43.

**Status:** In progress. Done (2026-11-16), checked by the owner on Windows, macOS, Linux and WSL: R-SSH-42. Still open in [../ROADMAP.md](../ROADMAP.md): R-SSH-43.

## Requirements

| ID | Requirement |
|---|---|
| R-SSH-42 | **Hosts from `~/.ssh/config`** (2026-11-16, IDEA-38; *details 2026-11-16, the owner*). OpenSSH's config, read-only: `~/.ssh/config` (`%USERPROFILE%\.ssh\config` on Windows) and its `Include`s (relative to `~/.ssh`, `~` and wildcards in the file name, 16 levels deep as OpenSSH), not the system's `ssh_config`. Each name on a `Host` line without `*`, `?` or `!` is listed in a group "From ~/.ssh/config" on the start screen below the saved sessions, found by the same search; patterns are not listed but apply. A host's settings are OpenSSH's: for each keyword the first value of the blocks that match, in file order; `IdentityFile`s add up. Used: `HostName` (`%h`), `User`, `Port`, `IdentityFile` (`~`, `%d`, `%u`, `%h`, `%r`, `%%`), `ForwardAgent` (yes or no) and `ProxyJump` (R-SSH-43); what the config leaves out comes from the defaults (R-SSH-11), on the start screen the user name last from the local one. `Match` blocks are skipped. A card connects, or Save as session… opens the session editor with it, to keep it as a saved session. `parley-ssh NAME` on the command line (R-SSH-14) reads the config for NAME as `ssh NAME` does; `user@`, `-l`, `-p`, `-i`, `-A`/`-a` win over it; with no user anywhere, the host waits in quick connect (R-SSH-59). The files are checked for changes at most once a second while a start screen shows, and read again when one changed. A line that cannot be read (and a `Match` block) is skipped and said in a hint under the group; the hosts go into the palette with Phase 36. |

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
