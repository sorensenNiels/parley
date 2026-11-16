# Ideas: Beyond PuTTY

> One theme of [../IDEAS.md](../IDEAS.md); the next free number is on the last line of [../../IDEAS.md](../../IDEAS.md).

| ID | Idea |
|---|---|
| IDEA-37 | **Tabs and split panes** (2026-11-16): several sessions in one window, as tabs and side by side, with keys to move between them. Changes R-SSH-10 and R-SSH-15 (one window per session, as PuTTY). |
| IDEA-38 | **OpenSSH's config** (2026-11-16): read `~/.ssh/config` (`Host`, `HostName`, `User`, `Port`, `IdentityFile`, `ProxyJump`) and offer its hosts on the start screen beside the saved sessions, or import them as sessions. |
| IDEA-39 | **Type into several sessions at once** (2026-11-16): broadcast keys to the sessions chosen, with a clear mark on each. Needs IDEA-37. |
| IDEA-40 | **Snippets and macros** (2026-11-16): commands kept by name and sent with a click or a key, and recorded macros, as Parley 5250's (D14) and Parley SQL's snippets, with a `password` step that asks each time (D16). |
| IDEA-41 | **Copy my key to the server** (2026-11-16): `ssh-copy-id` from the window: add the session's public key to the server's `authorized_keys` after a password sign-on. |
| IDEA-42 | **Edit a remote file locally** (2026-11-16): open a file from the SFTP panel (R-SSH-35) in the desktop's editor and upload it again when it is saved. |
