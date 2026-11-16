# Parley SSH features

> Checks of Parley SSH requirements still in progress, by requirement ID. The requirements are in
> [ROADMAP.md](ROADMAP.md). Add an entry only when the requirement is fully met, has automated tests where practical,
> and has been checked in the running app on at least one OS; note which OSes were checked. Then move the
> requirement's row from the roadmap, and this entry, to its phase's file in [phases/](phases/), with a row in
> [REQUIREMENTS.md](REQUIREMENTS.md).

## In progress

### Phase 34: hosts from `~/.ssh/config` (R-SSH-43 built 2026-11-16, Linux under WSL)

Automated (all pass): `parley-ssh-core`'s `net/jump.rs` (OpenSSH's syntax with `ssh://`, IPv6 and `none`, what
cannot be read, a config host's settings and its own `ProxyJump`, a loop said), `tests/net.rs` against in-process
russh servers with `direct-tcpip` (two jump hosts: each hop's host key and password asked and named, only the target's
`Secured`, each hop forwarding to the next; a jump host that cannot reach the target; a connection that never
started); `parley-ssh-ui`'s `sessions.rs` (the session's user and key files for jumps, a bad field said by the editor
and on connecting, `-J` for another window), `dialogs.rs` (a named prompt says its host), `openssh.rs` (a config
host's `ProxyJump`); `parley-app`'s `ssh_start.rs` (`-J`). Not yet seen in the running app.

| ID | Built | Still to check in the running app |
|---|---|---|
| R-SSH-43 | `Options::jumps`, `net::jump::resolve`/`check`, each hop connected through the one before with its own host-key check and sign-on, the Jump hosts field in the session editor, `-J`, a config host's `ProxyJump`, prompts naming their hop. | A session through a real bastion (`ProxyJump` in `~/.ssh/config` and typed in the editor); two jump hosts; a jump host that cannot reach the target; the editor's new field in light and dark. |
