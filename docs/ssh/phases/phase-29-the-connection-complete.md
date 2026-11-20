# Phase 29 — The connection, complete

> Finished phase of Parley SSH. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

Made lean by D51: keepalives and the IP version, rekeying and compression, a remote command with environment
variables and the terminal type, and proxies. No older algorithms. R-SSH-26..29.

**Status:** Done (2026-11-20), checked by the owner on Windows, macOS, Linux and WSL: R-SSH-26, R-SSH-27, R-SSH-28, R-SSH-29.

## Requirements

| ID | Requirement |
|---|---|
| R-SSH-26 | **Keeping the connection** (*made lean 2026-11-16, D51*): a session setting "Keepalive every N seconds" (`keepalive@openssh.com`; default 30, 0 for none); after three unanswered the connection counts as lost (R-SSH-51). TCP keepalives on and Nagle off always. IPv4, IPv6 or either (default either), also from `AddressFamily` in `~/.ssh/config`. A lost connection says so in the status bar, with Restart. |
| R-SSH-27 | **Rekeying and compression** (*made lean 2026-11-16, D51*): a session setting to rekey after N minutes (default 60) and after N MB (default 1024); `zlib@openssh.com` compression as a switch, off by default (also `Compression yes` in `~/.ssh/config`). Algorithm lists and the older algorithms are not offered (D51). |
| R-SSH-28 | **What runs on the server** (*made lean 2026-11-16, D51*): a session setting for a remote command run instead of the shell, still with a pseudo-terminal (e.g. `tmux new -A -s main`; `RemoteCommand` from `~/.ssh/config`, and a command after the host on the command line as `ssh host cmd`; *2026-11-16, while building: `RequestTTY` is not read, the terminal always gets a pseudo-terminal*); environment variables sent as `env` requests, `NAME=value` one per line (`SetEnv` from the config; the server may refuse them, which the event log says); the terminal type string (default `xterm-256color`). |
| R-SSH-29 | **Proxies** (PuTTY's Proxy page; *made lean 2026-11-16, D51; jump hosts moved to R-SSH-43*): a session setting for a proxy: none, SOCKS 5 (RFC 1928, with user name and password, RFC 1929, the host name resolved by the proxy) or HTTP CONNECT (RFC 9110 §9.3.6, with Basic authentication), host and port; a proxy password is asked when the proxy wants one, never kept; or a local proxy command whose standard input and output carry the connection (`%h`, `%p`, `%r` replaced; `ProxyCommand` from `~/.ssh/config`). A proxy that refuses says why in plain words. A proxy and jump hosts together: the proxy reaches the first jump host. |

## Verification

- **R-SSH-26, R-SSH-27, R-SSH-28, R-SSH-29** (built 2026-11-16, Linux under WSL; D51). **Automated** (`make test`): `parley-ssh-core`'s `tests/net.rs` against the in-process server: a command run instead of the shell with `LANG` taken, another variable refused and the terminal type `xterm`; a SOCKS 5 proxy asking its password and carrying the connection; an HTTP proxy answering 407, the password asked and the connection made again with Basic; an HTTP proxy without a user saying what is missing; a proxy command (a Python relay, Unix) carrying the connection; IPv6 only to an IPv4 address said. `net/client.rs` (keepalive, three unanswered, rekey limits capped at 1 GiB, compression first only when on), `net/transport.rs` (proxy command tokens, HTTP status and bracketed IPv6), `openssh_config.rs` (`RemoteCommand`, `SetEnv`, `AddressFamily`, `Compression`, `ProxyCommand`, `none`); `parley-ssh-ui`'s `sessions/connection.rs` (transport and shell from a session, environment lines, a proxy's host or command needed), `sessions.rs` (the editor's three pages with Save staying put, a missing proxy host said, saving proxy and environment), `openssh.rs` (the config's keywords into a session); `parley-app`'s `ssh_start.rs` (what follows the host is the command).
  - R-SSH-26: Session ▸ Connection: keepalive every N s (30; 0 none), three unanswered end the connection (then R-SSH-51 reconnects); IP version Either, IPv4 or IPv6 (`AddressFamily`); TCP keepalives on and Nagle off always (`socket2`). Checked: A server that goes away silently (firewall drop) is noticed within about 90 s; IPv6 to a dual-stack host.
  - R-SSH-27: New keys after N minutes (60) and N MB (1024, at most 1 GiB); compression switch (off; `Compression yes`). Checked: A long session past the rekey time; compression on a slow link; the event log's algorithms.
  - R-SSH-28: Session ▸ On the server: a command instead of the shell, still with a pseudo-terminal (`RemoteCommand`; `parley-ssh host cmd…`); environment `NAME=value` lines (`SetEnv`), each answered by the server and a refusal said in the event log; the terminal type. Checked: `tmux new -A -s main`; `LANG` with OpenSSH's default `AcceptEnv`; a variable it refuses; `TERM=xterm` in vim.
  - R-SSH-29: Session ▸ Connection ▸ Proxy: None, SOCKS 5, HTTP (host, port, user; the password asked when the proxy wants one, never kept) or a command (`%h`, `%p`, `%r`; `ProxyCommand`); a proxy reaches the first jump host; its refusals in plain words. Checked: A real SOCKS 5 proxy (`ssh -D` on another machine), an HTTP proxy such as Squid with and without a password, `ProxyCommand nc …`, a proxy with jump hosts; Windows' `cmd /C` for a proxy command.
  **Checked in the running app** by the owner on Windows, macOS, Linux and WSL (2026-11-20).
