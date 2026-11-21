# Parked: not now

> What has been decided **not to build now**, for all three programs (Parley 5250, Parley SQL, Parley SSH), so the
> roadmaps and the ideas files hold only what is planned or still open (D62, the final push, 2026-11-21). Nothing here
> is planned. Look an ID up with `grep -n` (`"| IDEA-17 "`, `"| R-SSH-30 "`).
>
> **Coming back:** when a real user needs one, its row moves back to the ideas file of its theme (an idea) or to its
> roadmap's §5 (a requirement), with the same ID and a note of who asked; a dropped one needs a new decision. IDs are
> never reused or renumbered.

## Waiting for a user who needs it

Decided: worth building, but only when someone asks.

| ID | What | Why parked |
|---|---|---|
| IDEA-17 | Import sessions from IBM i Access Client Solutions (`.hod`) and Personal Communications (`.ws`) files: host, port, TLS, device name, code page, screen size. Lowers the cost of trying Parley. | D60 |
| IDEA-24 | Screen reader support for the 5250 screen beyond today's whole-screen text: the current field's label and contents, and the error line, announced as they change. | D60 |
| Phase 22, more | GUI elements beyond R-DS-10..12 (enhanced user interface level 1): Phase 22 ends with the owner's checks and the GUI stays a beta switch. | D60 |
| Phase 30 | **Tunnels** (Parley SSH): local, remote and dynamic (SOCKS) port forwarding, and changing the forwardings while connected. R-SSH-31's engine part exists (built for Phase 31, R-SSH-54). The requirements below. | D59 |

Phase 30's requirements, as written:

| ID | Requirement |
|---|---|
| R-SSH-30 | **Local port forwarding** (`-L`, RFC 4254 §7.2): listen on a local port and forward each connection to a host and port seen from the server; bound to loopback unless "accept connections from other hosts" is on. |
| R-SSH-31 | **Remote port forwarding** (`-R`, RFC 4254 §7.1): ask the server to listen and forward its connections to a local host and port. |
| R-SSH-32 | **Dynamic forwarding** (`-D`): a local SOCKS 4 and 5 server whose connections go out through the server. |
| R-SSH-34 | **Tunnels while connected**: a Tunnels dialog adds and removes forwardings on an open connection; the status bar shows how many forwardings and open tunnel connections there are; a port that cannot be opened is said in the Event Log and the status bar, not in a pop-up. |

## Not now

Decided for the final push (D62): left out, not rejected.

| ID | What | Product |
|---|---|---|
| IDEA-10 | A trace window (the host's records, decoded) for support, built on `PARLEY_TRACE`. | Parley 5250 |
| IDEA-34, rest | Folding and several cursors in the SQL editor (the rest of IDEA-34 was built in Phases 14 and 16, D28). | Parley SQL |
| IDEA-36 | **Library groups that hold scripts** (Claude's suggestion, 2026-11-15): a group of R-SQL-83 growing into a project, with saved `.sql` scripts beside its libraries. Would change D33 (groups only organise libraries). | Parley SQL |

## Dropped

Decided against; the text is kept so the IDs stay explained.

| ID | What | Why |
|---|---|---|
| IDEA-12 | Startup Response Record (RFC 4777 section 10, `IBMSENDCONFREC`): show why a bypass sign-on failed instead of only the sign-on screen. (Was → R-NET-6.) | No auto sign-on, so no bypass sign-on to explain (R-NET-7, D16). |
| IDEA-42 | Edit a remote file locally: open a file from the SFTP panel (R-SSH-35) in the desktop's editor and upload it again when it is saved. | Built on the SFTP panel that rsync replaced (D48, D60). |

Dropped requirements, as written:

| ID | Requirement |
|---|---|
| R-SSH-33 | ~~**X11 forwarding** (RFC 4254 §6.3): off by default; the X display and its MIT-MAGIC-COOKIE-1 from the Xauthority file, with a fake cookie towards the server as OpenSSH does.~~ Dropped (D59). |
| R-SSH-35 | ~~**An SFTP panel** (draft-ietf-secsh-filexfer-02, version 3, as OpenSSH's server): beside the terminal, on the same connection: browse the server's folders starting in the home folder, download and upload files and folders (also by dragging from and to the desktop), rename, delete (asked first), make a folder, change permissions; transfers with progress and Cancel in the status bar; overwriting asks.~~ *Replaced 2026-11-16 by rsync (D48, R-SSH-53..57).* |
| R-SSH-36 | ~~**SCP where there is no SFTP**: downloads and uploads through `scp -f`/`scp -t` when the server has no SFTP subsystem, in the same panel with fewer actions.~~ *Replaced 2026-11-16 by rsync (D48, R-SSH-53..57).* |
| R-SSH-37 | ~~**Make a key pair** (PuTTY's PuTTYgen): Ed25519 by default, ECDSA P-256/384/521 or RSA 3072/4096, with a comment and a passphrase (asked twice); save the private key as OpenSSH or `.ppk` v3 and the public key; show the public key line for `authorized_keys` with a Copy button and its SHA-256 fingerprint.~~ *Replaced 2026-11-21 by R-SSH-67, R-SSH-68 (D60).* |
| R-SSH-38 | ~~**Change a key**: load a private key file (both formats), change or remove its passphrase, and save it in the other format.~~ *Replaced 2026-11-21 by R-SSH-67, R-SSH-68 (D60).* |
