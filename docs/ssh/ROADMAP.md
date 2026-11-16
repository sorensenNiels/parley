# Parley SSH roadmap

> The plan for Parley SSH: an SSH client for Windows, Linux and macOS in Parley's look, the program `parley-ssh`, and
> the engine under it. Finished phases, with the text of their requirements and how each was verified, are in
> [phases/](phases/); [REQUIREMENTS.md](REQUIREMENTS.md) is the index of finished IDs. The terminal's plan is
> [../ROADMAP.md](../ROADMAP.md), Parley SQL's [../sql/ROADMAP.md](../sql/ROADMAP.md).
> Look things up with `grep -n` (an ID such as `"R-SSH-7 "`, `"| D41 "` or a heading) rather than reading the whole file.

**Status of this document:** first written on 2026-11-16, when the owner asked for Parley SSH (D41). Phase 26 started the
same day, after the owner answered §6 (D42–D45).

## 1. Vision

Parley SSH is an SSH client for 2026: a shell on remote systems in tabs and split panes, the hosts you already have
in `~/.ssh/config`, keys and agents, a command palette, shell integration, transfers with rsync, and later tunnels, in
the lean, modern look Parley 5250 and Parley SQL share. It is a desktop program of its own, `parley-ssh`, in this
repository and in Parley's releases (D41).

**The yardstick is today's SSH clients and terminals** (Windows Terminal, iTerm2, WezTerm, Termius), not PuTTY (D46,
2026-11-16; Phases 26 and 27 were built with PuTTY in mind). When a requirement is planned in detail, check how they
behave, and keep it lean: good defaults first, and a setting only for what people commonly change. What PuTTY has that
Parley SSH leaves out on purpose is in §4 "Not planned".

## 2. Principles

Parley's principles apply ([../ROADMAP.md](../ROADMAP.md) §2), read for SSH:

- **Keyboard first** (R-P-1): every key reaches the remote program as xterm sends it; Parley SSH's own shortcuts use
  Ctrl+Shift so Ctrl+letter stays the host's.
- **Correct before clever** (R-P-2): the terminal follows xterm's control sequences (ECMA-48, DEC VT100/VT220, xterm's
  ctlseqs) and is tested byte for byte with recorded output of real programs (vim, less, htop, tmux), never a live
  server in `cargo test`.
- **Secure by default** (R-P-3, D16): a host key is always checked and a changed one is never accepted with one click;
  no password or passphrase is kept; modern algorithms only, unless a session asks for an old one.
- **One engine** (R-P-4, R-P-5): the SSH protocol and the terminal emulation live in `parley-ssh-core`, with no UI, and
  the terminal part is sans-I/O as D4's (bytes in, screen out, keys in, bytes out).

## 3. Status

| Phase | State |
|---|---|
| [Phase 26: a first shell](phases/phase-26-a-first-shell.md) | Done (2026-11-16), checked by the owner on Windows, macOS, Linux and WSL: R-SSH-1..10. |
| [Phase 27: saved sessions and keys](phases/phase-27-saved-sessions-and-keys.md) | Done (2026-11-16), checked by the owner on Windows, macOS, Linux and WSL: R-SSH-11..16, R-SSH-59. |
| [Phase 33: tabs and split panes](phases/phase-33-tabs-and-split-panes.md) | Done (2026-11-16), checked by the owner on Windows, macOS, Linux and WSL: R-SSH-39..41, R-SSH-58. |
| Phase 34: hosts from `~/.ssh/config` | In progress: R-SSH-42 done ([phase file](phases/phase-34-hosts-from-ssh-config.md)); R-SSH-43 next. |
| Phase 35: shell integration | Planned. |
| Phase 36: the command palette | Planned. |
| Phase 28: the terminal, lean | In progress: R-SSH-18 done ([phase file](phases/phase-28-the-terminal-lean.md)); cut down by D46. |
| Phase 37: themes and fonts | Planned. |
| Phase 38: snippets and broadcast | Planned. |
| Phase 39: reconnect and restore | Planned. |
| Phase 31: transfers with rsync | Planned (D48; was file transfer with SFTP). |
| Phase 29: the connection, complete | Later; to be trimmed by D46 when planned. |
| Phase 30: tunnels | Later. |
| Phase 32: keys made here | Later. |

The table is in the order the phases are built (the owner, 2026-11-16); numbers are kept, so they are not in order.

## 4. Phases

Phase numbers continue the shared sequence; the next free one is on the last line of [../ROADMAP.md](../ROADMAP.md).

### Phase 28 — the terminal, lean

Cut down by D46 to what modern use needs: the mouse for vim, tmux and htop (R-SSH-18), find in the scrollback
(R-SSH-24), a simple bell (R-SSH-19) and session logs (R-SSH-25). Appearance moves to Phase 37 (R-SSH-20). Dropped:
PuTTY's keyboard styles (R-SSH-17), selection options (R-SSH-21), character sets other than UTF-8 (R-SSH-22) and its
terminal options (R-SSH-23).

### Phase 34 — hosts from `~/.ssh/config`

From IDEA-38: the hosts in OpenSSH's config on the start screen and in the palette, as OpenSSH reads them, and jump
hosts (`ProxyJump`). R-SSH-42, R-SSH-43.

### Phase 35 — shell integration

Links you can click, the shell's prompts as marks to jump between commands and copy a command's output, a
notification when a long command ends, and the clipboard from the server. R-SSH-44..47.

### Phase 36 — the command palette

One box (Ctrl+Shift+P) for every host, session, tab and action. R-SSH-48.

### Phase 37 — themes and fonts

Themes people know by look, following the system's light and dark, fonts with Nerd Font symbols and emoji. R-SSH-20
(rewritten).

### Phase 38 — snippets and broadcast

From IDEA-39 and IDEA-40: commands kept by name and sent with a key, and typing into several panes at once.
R-SSH-49, R-SSH-50.

### Phase 39 — reconnect and restore

A dropped connection comes back by itself after sleep or a network change, and the tabs of last time can open again
on start. R-SSH-51, R-SSH-52.

### Phase 29 — the connection, complete

PuTTY's Connection, Data, Proxy and SSH pages: keepalives, IPv4/IPv6, the algorithm lists, rekeying, compression, a
remote command instead of a shell, environment variables, terminal modes, and proxies and jump hosts. R-SSH-26..29.

### Phase 30 — tunnels

PuTTY's Tunnels and X11 pages: local, remote and dynamic (SOCKS) port forwarding, X11 forwarding, and changing the
forwardings while connected. R-SSH-30..34.

### Phase 31 — transfers with rsync

From IDEA-43 (D48): files and folders copied with rsync, straight from server to server where they can reach each
other, through Parley SSH where they cannot, and between the desktop and a server; a Transfer panel with a dry run
first, progress, Cancel and resume, and transfers saved by name. Replaces SFTP and SCP (R-SSH-35, R-SSH-36).
R-SSH-53..57.

### Phase 32 — keys made here

What PuTTY does with PuTTYgen: make a key pair, change its passphrase, convert between OpenSSH and PuTTY formats, and
copy the public key for `authorized_keys`. R-SSH-37, R-SSH-38.

### Later (unordered, to be planned)

From PuTTY, after Phase 32 if still wanted: an agent of Parley SSH's own (PuTTY's Pageant; Phase 27 uses the agent
that is already running), GSSAPI/Kerberos sign-on and key exchange, OpenSSH certificates, sharing one connection
between sessions, and Telnet, Rlogin, raw TCP and serial connections. New features beyond PuTTY: see [IDEAS.md](IDEAS.md).

### Not planned

- **SSH-1.** Broken since the 1990s; OpenSSH dropped it in 2017.
- **PuTTY's command-line tools** (Plink, PSCP, PSFTP) as separate programs: OpenSSH's `ssh`, `scp` and `sftp` ship
  with every desktop OS Parley supports (Windows 10 and later included). Parley SSH's command line (R-SSH-14) only
  opens a window.
- **Settings in the Windows registry.** Parley SSH keeps its settings as Parley does (eframe's storage), on every OS.
- **A way to skip host-key checks** (R-P-3).

## 5. Requirements

Open requirements, in the order they are planned, as `| ID | Requirement | Phase |`. The done ones go to
[REQUIREMENTS.md](REQUIREMENTS.md); new ones come from [IDEAS.md](IDEAS.md). Area: `SSH`, Parley SSH and its engine.
Rows for Phases 28–32 name what is in scope; their details are written from PuTTY's behaviour when the phase is
planned, as Phase 22's rows were.

| ID | Requirement | Phase |
|---|---|---|
| R-SSH-17 | ~~**Keyboard settings** (PuTTY's Keyboard and Features pages): Backspace sends `DEL` or `^H`; Home/End as standard or rxvt; function keys as xterm, ESC[n~, Linux, Xterm R6, VT400, VT100+ or SCO; the initial state of the cursor keys and keypad; a switch to refuse application cursor/keypad mode; Alt as `ESC` prefix or as Meta; AltGr as AltGr or as Ctrl+Alt.~~ *Dropped 2026-11-16 (D46): legacy options; UTF-8 and xterm's keys and behaviour only.* | 28 |
| R-SSH-19 | **The bell** (*made lean 2026-11-16, D46*): none, a sound (as D37's beep, played by the launcher) or a flash of the pane's border (default: flash); while the window is not focused, its taskbar entry asks for attention. | 28 |
| R-SSH-20 | **Themes and fonts** (*rewritten 2026-11-16, D46; was PuTTY's Window, Appearance and Colours pages*). Settings ▸ Appearance: a theme from a gallery with previews (Parley's own, and well-known permissively licensed schemes such as Solarized, Nord, Dracula, Gruvbox, Catppuccin and Tokyo Night), a light and a dark one that follow the system's theme; the terminal font, bundled or installed, Nerd Font symbols and emoji drawn from fallback fonts; the font size with Ctrl+= / Ctrl+- / Ctrl+0; the cursor (block, bar or underline, blinking or not); padding around the screen. Details when Phase 37 is planned. | 37 |
| R-SSH-21 | ~~**Selection settings** (PuTTY's Selection page): copy on select with a right or middle click pasting (PuTTY's way) or the right-click menu (R-SSH-9); rectangular selection with Alt+drag; which characters make a word for a double click; copy with colours (HTML and RTF beside the text).~~ *Dropped 2026-11-16 (D46): legacy options; UTF-8 and xterm's keys and behaviour only.* | 28 |
| R-SSH-22 | ~~**Character sets** (PuTTY's Translation page): the remote character set, UTF-8 by default, or ISO 8859-1..16, CP437, CP850, CP1252 or KOI8-R; ambiguous-width characters narrow or wide; line drawing as Unicode or as the font's.~~ *Dropped 2026-11-16 (D46): legacy options; UTF-8 and xterm's keys and behaviour only.* | 28 |
| R-SSH-23 | ~~**Terminal options** (PuTTY's Terminal page): auto-wrap on at start, implicit CR in every LF and LF in every CR, the answerback to `^E`, whether the host may set the window title, and whether it may resize the window.~~ *Dropped 2026-11-16 (D46): legacy options; UTF-8 and xterm's keys and behaviour only.* | 28 |
| R-SSH-24 | **Find in the scrollback** (Ctrl+Shift+F): the find bar of Parley 5250 (`terminal/find`), over the screen and the scrollback, with match case and next/previous. | 28 |
| R-SSH-25 | **Session logs** (*made lean 2026-11-16, D46*): per session, off by default: the text as shown (without escape sequences) or the raw output, to a file named from a pattern with `&H` host, `&Y&M&D` date, `&T` time, appended to; Session ▸ Start/Stop log for the session open. | 28 |
| R-SSH-26 | **Keeping the connection** (PuTTY's Connection page): keepalives every N seconds (`keepalive@openssh.com`), TCP keepalives, Nagle off, IPv4, IPv6 or either; a lost connection is shown as such, with Restart. | 29 |
| R-SSH-27 | **Algorithms and rekeying** (PuTTY's SSH, Kex, Host keys and Cipher pages): the order of key exchanges, host-key types and ciphers, with a "warn below here" line as PuTTY's, and older ones (`diffie-hellman-group1-sha1`, `ssh-rsa` with SHA-1, `aes-cbc`, `3des-cbc`) offered only when moved above it; rekey after N minutes or N bytes; `zlib@openssh.com` compression. | 29 |
| R-SSH-28 | **What runs on the server** (PuTTY's SSH and Data pages): a remote command instead of the shell, or no shell at all (for tunnels only); the terminal type string; environment variables (`env` requests); terminal modes; no pseudo-terminal; a user name taken from the session or asked. | 29 |
| R-SSH-29 | **Proxies and jump hosts** (PuTTY's Proxy page, OpenSSH's `ProxyJump`): SOCKS 4, SOCKS 5 and HTTP CONNECT proxies, a local proxy command, and one or more jump hosts reached through `direct-tcpip`, each with its own host-key check and sign-on; proxy passwords asked, not kept; which hosts go direct. | 29 |
| R-SSH-30 | **Local port forwarding** (`-L`, RFC 4254 §7.2): listen on a local port and forward each connection to a host and port seen from the server; bound to loopback unless "accept connections from other hosts" is on. | 30 |
| R-SSH-31 | **Remote port forwarding** (`-R`, RFC 4254 §7.1): ask the server to listen and forward its connections to a local host and port. | 30 |
| R-SSH-32 | **Dynamic forwarding** (`-D`): a local SOCKS 4 and 5 server whose connections go out through the server. | 30 |
| R-SSH-33 | **X11 forwarding** (RFC 4254 §6.3): off by default; the X display and its MIT-MAGIC-COOKIE-1 from the Xauthority file, with a fake cookie towards the server as OpenSSH does. | 30 |
| R-SSH-34 | **Tunnels while connected**: a Tunnels dialog adds and removes forwardings on an open connection; the status bar shows how many forwardings and open tunnel connections there are; a port that cannot be opened is said in the Event Log and the status bar, not in a pop-up. | 30 |
| R-SSH-35 | ~~**An SFTP panel** (draft-ietf-secsh-filexfer-02, version 3, as OpenSSH's server): beside the terminal, on the same connection: browse the server's folders starting in the home folder, download and upload files and folders (also by dragging from and to the desktop), rename, delete (asked first), make a folder, change permissions; transfers with progress and Cancel in the status bar; overwriting asks.~~ *Replaced 2026-11-16 by rsync (D48, R-SSH-53..57).* | 31 |
| R-SSH-36 | ~~**SCP where there is no SFTP**: downloads and uploads through `scp -f`/`scp -t` when the server has no SFTP subsystem, in the same panel with fewer actions.~~ *Replaced 2026-11-16 by rsync (D48, R-SSH-53..57).* | 31 |
| R-SSH-37 | **Make a key pair** (PuTTY's PuTTYgen): Ed25519 by default, ECDSA P-256/384/521 or RSA 3072/4096, with a comment and a passphrase (asked twice); save the private key as OpenSSH or `.ppk` v3 and the public key; show the public key line for `authorized_keys` with a Copy button and its SHA-256 fingerprint. | 32 |
| R-SSH-38 | **Change a key**: load a private key file (both formats), change or remove its passphrase, and save it in the other format. | 32 |
| R-SSH-43 | **Jump hosts** (2026-11-16; *moved here from R-SSH-29*). One or more jump hosts (`ProxyJump`, a session setting, and `-J` on the command line), reached through `direct-tcpip`, each with its own host-key check and sign-on. | 34 |
| R-SSH-44 | **Links** (2026-11-16). OSC 8 hyperlinks and URLs in the text are underlined under the pointer; Ctrl+click (Cmd+click) opens them in the browser; the right-click menu copies one. | 35 |
| R-SSH-45 | **Commands as marks** (2026-11-16). With a shell that sends OSC 133 (bash, zsh, fish with their integration; PowerShell), prompts are marked in the scroll bar; Ctrl+Shift+Up and Down jump between commands; the right-click menu selects or copies a command's output; a failed command's mark is red. | 35 |
| R-SSH-46 | **Notifications** (2026-11-16). OSC 9 and OSC 777 notifications from the host, and a command that ran longer than 10 seconds ending (R-SSH-45), show as a desktop notification while the window is not focused. | 35 |
| R-SSH-47 | **The clipboard from the server** (2026-11-16). OSC 52: the host may set the clipboard (on by default, a setting turns it off); it may never read it. | 35 |
| R-SSH-48 | **The command palette** (2026-11-16). Ctrl+Shift+P (Cmd+Shift+P) opens a box over the window that finds, as one types and forgiving typos, saved sessions, `~/.ssh/config` hosts, open tabs, recent connections and every menu action, each with its shortcut; Enter runs the one chosen; recent ones first. | 36 |
| R-SSH-49 | **Snippets** (2026-11-16, IDEA-40). Commands kept by name (Settings ▸ Snippets, and in the palette), sent to the pane in front with Enter or not, with a key of their own if wanted; placeholders asked for when sent. Details when Phase 38 is planned. | 38 |
| R-SSH-50 | **Broadcast** (2026-11-16, IDEA-39). Type into several panes at once: a switch per tab or for every tab, each pane that receives marked clearly, and turned off with Escape twice. | 38 |
| R-SSH-51 | **Reconnect** (2026-11-16). A connection lost to sleep or a network change reconnects by itself (with backoff, for 10 minutes; a setting), asking again only for what sign-on needs; the screen keeps its text and says it reconnected. | 39 |
| R-SSH-52 | **Restore** (2026-11-16). On start, the tabs and panes open when Parley SSH last closed can open again (a setting, off by default), each connecting as it is shown. | 39 |
| R-SSH-53 | **Server to server, direct** (2026-11-16, IDEA-43, D48). From a session on server A, rsync on A copies a path to server B (`rsync -az`), signing on to B with the user's agent through agent forwarding (R-SSH-13) for that transfer only: no key is copied to A and the data does not pass through the desktop. | 31 |
| R-SSH-54 | **Server to server, through Parley SSH** (2026-11-16, IDEA-43, D48). When A cannot reach B, A's rsync goes to B through a port Parley SSH forwards back from A (remote forwarding, R-SSH-31, built with this). | 31 |
| R-SSH-55 | **Desktop and server** (2026-11-16, IDEA-43, D48). With the desktop's rsync, Parley SSH as its transport; where the desktop has no rsync (Windows by default) this is said with how to get it, and what then is decided when Phase 31 is planned. | 31 |
| R-SSH-56 | **The Transfer panel** (2026-11-16, IDEA-43, D48). A source and a target, each an open session (or the desktop) and a path; a dry run first (`--dry-run --itemize-changes`) listed as what will be added, changed and deleted; then progress, speed and time left (`--info=progress2`), Cancel, and resume (`--partial`); switches for deleting extra files, keeping permissions and excludes; transfers saved by name to run again, also from the palette (R-SSH-48). | 31 |
| R-SSH-57 | **rsync is checked first** (2026-11-16, IDEA-43, D48). Both ends need rsync 3.1 or later (`rsync --version`); when one has none or an older one, this is said in plain words before anything is copied. | 31 |

## 6. Open questions

None. The four asked on 2026-11-16 were answered by the owner the same day: D42 (`russh`), D43 (`vte` and a screen of
Parley SSH's own), D44 (key files and a running agent, nothing kept), D45 (one window per session).

## 7. Decisions

The decisions, with the question, the choice and the alternatives, are in [DECISIONS.md](DECISIONS.md); D1–D18 and
D36–D38 in [../DECISIONS.md](../DECISIONS.md) bind this work too.

## 8. How to change this roadmap

- New SSH work gets a requirement in §5 with the next free `R-SSH` ID (the last line); IDs are never reused or renumbered.
- A requirement in another area, a decision (`D`) or a new phase takes the next free number from the last line of
  [../ROADMAP.md](../ROADMAP.md), which is then moved on, so a number never means two things.
- While a requirement is built, how it is checked goes under "In progress" in [FEATURES.md](FEATURES.md).
- A requirement that is implemented, tested and checked in the running app moves, with its full row and how it
  was verified, to its phase's file in [phases/](phases/), and gets an index row (ID, title, phase) in
  [REQUIREMENTS.md](REQUIREMENTS.md).
- When a phase is finished, its story (its §4 section and §3 state) moves to a new file
  `phases/phase-NN-<slug>.md`, and its §3 row links to it.
- An idea is not a plan: it goes in [IDEAS.md](IDEAS.md) (by theme in [ideas/](ideas/)) until it is picked, then becomes requirements here.

Next free ID: `R-SSH-60` (other areas, decisions and phases: last line of [../ROADMAP.md](../ROADMAP.md); ideas: last line of [../IDEAS.md](../IDEAS.md)).
