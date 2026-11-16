# Parley SSH decisions

> Decisions are numbered across all three programs (Parley 5250, Parley SQL and Parley SSH); the next free number is on [../ROADMAP.md](../ROADMAP.md)'s last line.

Decisions D1–D18 and D36–D38 (language, crates, TLS, the look, no saved passwords, two programs apart, the alarm) are
in [../DECISIONS.md](../DECISIONS.md) and bind this work too.

| ID | Question | Decision | Alternatives considered |
|---|---|---|---|
| D41 | Should the Parley family get an SSH client, and how? | **Yes: Parley SSH** (the owner, 2026-11-16), an SSH client that does what PuTTY does, then more, in the look Parley 5250 and Parley SQL share (D12). Built as D36 built Parley SQL: its own engine crate `parley-ssh-core` and UI crate `parley-ssh-ui` on `parley-ui-egui`'s look, a third binary `parley-ssh` in `parley-app`, its own settings and app id `parley-ssh`, in this repository and in Parley's releases. `cargo deny` keeps it apart from the other two programs (R-P-5). Its plan is [ROADMAP.md](ROADMAP.md). | A separate repository (the shared look would drift, D12); an add-on inside Parley 5250 (an SSH user needs no green screen). |
| D42 | Which SSH library (§6 question 1)? | **`russh`** (the owner, 2026-11-16): pure Rust, Apache-2.0, with `russh`'s key and agent support. It needs tokio, which D4 kept out of Parley; tokio runs only inside `parley-ssh-core`'s network thread (a current-thread runtime), so the UI and the terminal engine stay as D4's: no async outside that thread. | `ssh2` (libssh2 and OpenSSL: C code on three OSes and a second TLS stack beside D6's rustls). |
| D43 | How is the terminal emulated (§6 question 2)? | **A screen model of Parley SSH's own over the `vte` parser** (the owner, 2026-11-16), sans-I/O in `parley-ssh-core` (bytes in, screen out, replies out), tested byte for byte as `parley-core` is (R-P-2). | `alacritty_terminal` (large, its API follows Alacritty, its screen does not fit Parley's widgets). |
| D44 | May Parley SSH sign on with keys (§6 question 3)? Reads D16 for SSH. | **Yes: key files and an agent that is already running** (the owner, 2026-11-16): the key and the agent are the user's, as OpenSSH uses them. Parley SSH keeps no password or passphrase, asks for them each time, and has no agent of its own (Later). | Passwords only; an agent of Parley SSH's own from the start. |
| D45 | One window per session, or tabs (§6 question 4)? | **One window per session, as PuTTY** (the owner, 2026-11-16; R-SSH-10, R-SSH-15). Tabs and split panes stay an idea (IDEA-37). | Tabs from the start. |
