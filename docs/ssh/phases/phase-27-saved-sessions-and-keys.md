# Phase 27 — Saved sessions and keys

> Finished phase of Parley SSH. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

What makes it a daily tool: saved sessions with default settings, sign-on with a key file or an agent instead of a
password, a command line as PuTTY's and OpenSSH's, the Event Log, and starting another session. R-SSH-11..16.

**Status:** Done (2026-11-16), checked by the owner on Windows, macOS, Linux and WSL: R-SSH-11, R-SSH-12, R-SSH-13, R-SSH-14, R-SSH-15, R-SSH-16, R-SSH-59.

## Requirements

| ID | Requirement |
|---|---|
| R-SSH-11 | **Saved sessions** (PuTTY's Session page). A session keeps its name, host, port, user name and every setting of later phases; a "Default Settings" entry holds what a new session starts with. The start screen lists them with search (from four sessions on); Connect, or Enter in the search, opens one (*changed 2026-11-16: no double-click, the cards are Parley's shared ones, whose buttons a card-wide click would cover*); New, Edit, Delete (asked first); a quick connect below the list opens a host that is not saved, with the defaults' keys and agent (*changed 2026-11-16: Duplicate is R-SSH-15's window; Duplicate in the editor was left out*). No passwords or passphrases are saved (D16). Kept in Parley SSH's settings, read back by a test from a saved file. |
| R-SSH-12 | **Sign on with a key file** (RFC 4252 §7). A session may name private key files: OpenSSH format (Ed25519, ECDSA, RSA with `rsa-sha2-256/512`) and PuTTY's `.ppk` versions 2 and 3. Publickey is tried before keyboard-interactive and password. An encrypted key's passphrase is asked each time and never kept; a key file that cannot be read or decrypted is said in plain words. Keys are zeroed from memory when the connection ends. |
| R-SSH-13 | **Sign on through an agent** (draft-miller-ssh-agent). When an agent is running, its keys are tried first: `SSH_AUTH_SOCK` on Linux and macOS; on Windows OpenSSH's agent (`\\.\pipe\openssh-ssh-agent`) and Pageant. A session may turn the agent off, or name the one key to use. Agent forwarding (`auth-agent-req@openssh.com`) is a session setting, off by default, with a hint that the server's admin can use the forwarded keys. |
| R-SSH-14 | **A command line** as PuTTY's and OpenSSH's: `parley-ssh [user@]host[:port]`, `-p port` (also PuTTY's `-P`), `-l user`, `-i keyfile`, `-load "saved session"`, `-A`/`-a` (agent forwarding on or off); later phases add `-L`, `-R`, `-D`, `-J`. Wrong arguments print a `parley-ssh:` message and the usage, and the program ends with a non-zero status. |
| R-SSH-15 | **More sessions.** Session ▸ New session (Ctrl+Shift+N) and Duplicate session (Ctrl+Shift+D) open another `parley-ssh` window, a new program as PuTTY does; Duplicate connects to the same host with the same settings and asks for its own password. |
| R-SSH-16 | **The Event Log** (PuTTY's). A dialog listing what happened while connecting and after, with times: the address and port, the server's version string, the algorithms chosen, the host key and its fingerprint, how it was checked, each sign-on method tried and its outcome, channels opened and closed, rekeys, the disconnect and its reason. Never a password, passphrase or key. Copy selects all of it. |
| R-SSH-59 | **Parley 5250's SSH… starts Parley SSH** (2026-11-16, asked by the owner; as R-SQL-70 for Parley SQL). An SSH add-on in Parley 5250 (R-ADD-1; off by default) adds Tools ▸ SSH… (and a function a key can be bound to) while a session is connected. It starts the program Parley SSH, installed beside Parley, exactly as when it is started on its own, with the session's host on its command line (`parley-ssh HOST`, R-SSH-14): port 22, signing on as Parley SSH's default user. Parley knows no user and passes no password (D16). When Parley SSH's defaults name no user, the host waits in the start screen's quick connect, which says the user name is missing. Each SSH… starts another Parley SSH. When Parley SSH is not beside Parley or cannot start, Parley says so in a notice. Parley's Add-ons page says that SSH… starts Parley SSH, whose settings are there. |

## Verification

- **R-SSH-11, R-SSH-12, R-SSH-13, R-SSH-14, R-SSH-15, R-SSH-16, R-SSH-59** (2026-11-16). **Automated** (`make test`): `tests/net.rs` against the in-process russh server: an OpenSSH key file signs on with no
prompt; an encrypted key asks its passphrase, again when wrong; a PuTTY `.ppk` v3 key signs on; a key file that
cannot be read is said (`Event::Problem`) and the password still works; russh's own agent on a Unix socket signs on and,
forwarded, answers the server's request for its keys; an agent key not named is not used and forwarding stays off.
`sessions.rs`, `start.rs` and `tests.rs` (kittest): a session saved from the editor and listed, edit and delete (asked
twice), the defaults, search, the command line's request, New and Duplicate asking for windows; `ssh_start.rs` the
arguments; `tests/stored_settings.rs` settings saved before sessions still load and sessions read back.
  - R-SSH-11: Saved sessions in the settings, "Default Settings…", the editor (Name, Host, Port, User name, Key files, agent switches), search, quick connect.
  - R-SSH-12: Key files tried after the agent's keys: OpenSSH, PKCS #8, `.ppk` v2/v3; passphrase each time, three tries; keys wiped when dropped (ssh-key).
  - R-SSH-13: `SSH_AUTH_SOCK`; on Windows OpenSSH's agent pipe, then Pageant; one key by fingerprint; forwarding off by default, with a warning when turned on.
  - R-SSH-14: `parley-ssh [user@]host[:port] -p -l -i -load -A -a`, wrong arguments with usage and exit 2.
  - R-SSH-15: Session ▸ New session (Ctrl+Shift+N) and Duplicate session (Ctrl+Shift+D) start `parley-ssh` again: `-load NAME` for a saved session, else its host and options.
  - R-SSH-16: Session ▸ Event log: every line with its time, Copy, never a password or key.
  - R-SSH-59: Parley 5250's SSH add-on (`AddOn::Ssh`), Tools ▸ SSH… and `Function::Ssh`, the notice; `ssh_start::start` runs `parley-ssh HOST`; a host without a user waits in quick connect. Tests: `parley-ui-egui` `lib.rs` (`*_r_ssh_59`: menu only with the add-on, starts on the session's host, the key, the notice, nothing without a connection), `ssh_start.rs` (the arguments read back), `parley-ssh-ui` `tests.rs` (quick connect filled in).
  **Checked in the running app** by the owner on Windows, macOS, Linux and WSL (2026-11-16).
