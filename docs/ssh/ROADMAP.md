# Parley SSH roadmap

> The plan for Parley SSH: an SSH client for Windows, Linux and macOS in Parley's look, the program `parley-ssh`, and
> the engine under it. Finished phases, with the text of their requirements and how each was verified, will be in
> [phases/](phases/); [REQUIREMENTS.md](REQUIREMENTS.md) is the index of finished IDs. The terminal's plan is
> [../ROADMAP.md](../ROADMAP.md), Parley SQL's [../sql/ROADMAP.md](../sql/ROADMAP.md).
> Look things up with `grep -n` (an ID such as `"R-SSH-7 "`, `"| D41 "` or a heading) rather than reading the whole file.

**Status of this document:** first written on 2026-11-16, when the owner asked for Parley SSH (D41). Phase 26 started the
same day, after the owner answered §6 (D42–D45).

## 1. Vision

Parley SSH is an SSH client that does what people use PuTTY for (a shell on a remote system, saved sessions, keys
and agents, tunnels, file transfer, logging) in the look Parley 5250 and Parley SQL share, and then goes further
than PuTTY, one idea at a time ([IDEAS.md](IDEAS.md)). It is a desktop program of its own, `parley-ssh`, in this
repository and in Parley's releases (D41).

PuTTY's feature set is the yardstick for Phases 26–32: when a requirement is planned in detail, check how PuTTY (and,
where PuTTY is silent, OpenSSH and xterm) behaves, and do the same unless a requirement says otherwise. What PuTTY
has that Parley SSH leaves out on purpose is said in §4 "Not planned".

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
| Phase 26: a first shell | In progress (2026-11-16): R-SSH-1..10 built and tested ([FEATURES.md](FEATURES.md)); to be checked in the running app against real servers. |
| Phase 27: saved sessions and keys | Planned. |
| Phase 28: the terminal, complete | Planned. |
| Phase 29: the connection, complete | Planned. |
| Phase 30: tunnels | Planned. |
| Phase 31: file transfer | Planned. |
| Phase 32: keys made here | Planned. |

## 4. Phases

Phase numbers continue the shared sequence; the next free one is on the last line of [../ROADMAP.md](../ROADMAP.md).

### Phase 26 — a first shell

The smallest Parley SSH that is useful: open `parley-ssh`, type a host, user name and password, check the host key,
and work in a shell with full-screen programs (vim, less, htop, tmux) in colour. R-SSH-1..10.

### Phase 27 — saved sessions and keys

What makes it a daily tool: saved sessions with default settings, sign-on with a key file or an agent instead of a
password, a command line as PuTTY's and OpenSSH's, the Event Log, and starting another session. R-SSH-11..16.

### Phase 28 — the terminal, complete

PuTTY's Terminal, Keyboard, Bell, Window, Appearance, Translation, Selection and Colours pages, as settings per
session with defaults: keyboard modes, the mouse for remote programs, bell, colours and fonts, character sets,
selection, find in the scrollback, and session logs. R-SSH-17..25.

### Phase 29 — the connection, complete

PuTTY's Connection, Data, Proxy and SSH pages: keepalives, IPv4/IPv6, the algorithm lists, rekeying, compression, a
remote command instead of a shell, environment variables, terminal modes, and proxies and jump hosts. R-SSH-26..29.

### Phase 30 — tunnels

PuTTY's Tunnels and X11 pages: local, remote and dynamic (SOCKS) port forwarding, X11 forwarding, and changing the
forwardings while connected. R-SSH-30..34.

### Phase 31 — file transfer

What PuTTY does with PSCP and PSFTP, in the window: an SFTP panel on the same connection, and SCP where a server has no
SFTP. R-SSH-35, R-SSH-36.

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
| R-SSH-1 | **Parley SSH, a program of its own** (D41). Two new crates: `parley-ssh-core` (the SSH client and the terminal emulation; no UI) and `parley-ssh-ui` (the window, on `parley-ui-egui`'s look: theme, dialogs, icons, header, status bar, the settings frame with Appearance and About). A third binary `parley-ssh` in `parley-app`, with app id `parley-ssh` and settings of its own. `cargo deny` keeps the programs apart (R-P-5): Parley 5250's and Parley SQL's crates never use the SSH crates, and the SSH crates use neither program's engine nor Parley SQL's UI. `make run-ssh` starts it; the installers on all three OSes install it beside `parley` and `parley-sql`, with its own menu entry and icon (Start menu shortcut on Windows, `.desktop` entry in the `.deb`), and `parley-ssh` works in a new terminal as R-NF-2 makes `parley-sql` work; `parley --ssh [ARGS]`, or a link named `parley-ssh` to the AppImage, starts Parley SSH (2026-11-16, asked by the owner). | 26 |
| R-SSH-2 | **Connect** (RFC 4253). A start screen as Parley SQL's, with host, port (22) and user name; SSH-2 only. Offered by default, in this order: key exchange `curve25519-sha256`, `ecdh-sha2-nistp256/384/521`, `diffie-hellman-group16-sha512`, `diffie-hellman-group14-sha256`; host keys `ssh-ed25519`, `ecdsa-sha2-nistp256/384/521`, `rsa-sha2-512`, `rsa-sha2-256`; ciphers `chacha20-poly1305@openssh.com`, `aes256-gcm@openssh.com`, `aes128-gcm@openssh.com`, `aes256-ctr`, `aes128-ctr`; MACs `hmac-sha2-256-etm@openssh.com`, `hmac-sha2-512-etm@openssh.com`, `hmac-sha2-256`, `hmac-sha2-512`; strict key exchange (`kex-strict-c-v00@openssh.com`, the Terrapin fix) when the server offers it. A server with nothing in common, a refused connection, a timeout or a disconnect is told in plain words with the server's reason. | 26 |
| R-SSH-3 | **Host keys are checked** (RFC 4251 §4.1). Known keys are read from OpenSSH's `~/.ssh/known_hosts` (hashed entries and `[host]:port` too) and from Parley SSH's own `known_hosts` beside its settings, where it writes new ones (OpenSSH's file is never changed). A key not seen before opens a dialog with the host, the key type and its SHA-256 fingerprint: Accept and save, Connect once, or Cancel. A key that differs from the saved one opens a warning with both fingerprints and only Cancel as the main action; to go on the user removes the saved key in that dialog by a separate, explicit step and then sees the new-key dialog. No setting turns the check off. | 26 |
| R-SSH-4 | **Sign on with a password or keyboard-interactive** (RFC 4252, RFC 4256). The methods the server lists are tried in the order keyboard-interactive, password; each prompt is shown as the server sends it (name, instruction, prompts, echo or hidden), so one-time codes work. The password is asked each time and never kept (D16), held in `parley_ssh_core::Secret` (hidden from `Debug`, wiped when dropped; *changed 2026-11-16: the SSH crates do not use Parley 5250's engine, R-SSH-1*), and never logged. A server's banner (`SSH_MSG_USERAUTH_BANNER`) is shown before the prompts. A server that asks for a new password (`SSH_MSG_USERAUTH_PASSWD_CHANGEREQ`) gets a change-password dialog (*not yet: russh 0.65 cannot receive that message; servers that change passwords through PAM ask through keyboard-interactive, which works*). | 26 |
| R-SSH-5 | **A shell** (RFC 4254 §6). A session channel with a pseudo-terminal (`TERM=xterm-256color`, the window's columns and rows, UTF-8 terminal modes) and a shell. A resized window sends `window-change`. When the shell ends, the status bar says so with its exit status or signal and the screen stays readable; Restart session (Ctrl+Shift+R) connects again with the same settings, asking for the password again. | 26 |
| R-SSH-6 | **xterm emulation** (ECMA-48, DEC STD 070, xterm's ctlseqs). Sans-I/O in `parley-ssh-core`: bytes in, a screen of cells out, replies out. C0 controls; cursor movement and saving (`CUP`, `CUU`…, `DECSC`/`DECRC`); erase and insert/delete of characters and lines; scrolling regions (`DECSTBM`) and index; tab stops; SGR: bold, faint, italic, underline (single, double, curly), blink, inverse, hidden, strike-through, 16, 256 and 24-bit colours; DEC modes: cursor keys (`DECCKM`), auto-wrap (`DECAWM`), origin (`DECOM`), cursor shown (`DECTCEM`), the alternate screen (47, 1047, 1049), bracketed paste (2004), focus events (1004); DEC Special Graphics line drawing (`ESC ( 0`); the window title (OSC 0 and 2); device status and attribute replies (`DSR`, `DA1`, `DA2`) as xterm's; `RIS` and `DECSTR`. UTF-8 with wide characters (East Asian Wide, emoji) in two cells and combining marks in their base cell. Unknown sequences are skipped, never shown as text, and never panic. Tested with recorded output of vim, less, htop, tmux and `vttest`'s first screens, and with truncated and malformed sequences. | 26 |
| R-SSH-7 | **Keys go to the host as xterm sends them.** Printable characters as UTF-8 (AltGr characters too); Enter `CR`; Backspace `DEL` (0x7F); Tab, Shift+Tab (`CSI Z`); Escape; Ctrl+letter and Ctrl+@, [, \, ], ^, _; Alt+key as `ESC` then the key; the arrows, Home and End in normal and application mode; Insert, Delete, Page Up and Page Down (`CSI 2~` …); F1–F12 with Shift, Ctrl and Alt as xterm's modifier parameter. Parley SSH's own shortcuts use Ctrl+Shift (copy, paste, find, new session, restart), so Ctrl+C, Ctrl+V and the rest reach the host. A shortcut sheet lists them, as Parley's (R-UI-17). | 26 |
| R-SSH-8 | **The screen and its scrollback.** The terminal font of Parley 5250 (bundled JetBrains Mono or an installed font, `parley-ui-egui`'s `fonts`), with bold (drawn heavier and, for the eight normal colours, bright; no bold face is bundled) and italic (*not yet: no italic face is bundled; with R-SSH-20's fonts*); a block cursor that blinks; Parley's dark and light terminal colours (the 16 ANSI colours of each). Lines that scroll off the top are kept (10,000 by default; not for the alternate screen) and read with the mouse wheel, the scroll bar and Shift+Page Up/Down; a key that goes to the host scrolls back to the bottom. The window's columns and rows follow its size; the status bar shows them while it is resized. | 26 |
| R-SSH-9 | **Copy and paste.** Drag selects (across the scrollback too), a double click a word, a triple click a line; Ctrl+Shift+C or the right-click menu copies, as text without the trailing blanks of each line and with wrapped lines joined. Ctrl+Shift+V, Shift+Insert or the menu pastes; inside bracketed paste mode the text is wrapped in `CSI 200~`/`CSI 201~`, otherwise line ends are sent as `CR`. Pasting more than one line outside bracketed paste asks first (it may run commands), with "don't ask again" per session. While a remote program reads the mouse (R-SSH-18), Shift+drag still selects. | 26 |
| R-SSH-10 | **The window around the screen.** The header and status bar of Parley SQL: the session pill (`user@host`, connected or not), menus Session (New, Restart, Disconnect, Close), Edit (Copy, Paste, Copy all, Clear scrollback, Reset terminal), View and Help; the status bar shows the state, the size and, on hover, the algorithms in use and the host key's fingerprint. Settings with Parley's Appearance (theme, text size) and About (version, the update check, D13) pages. Closing the window while connected asks first (setting). Works in light and dark. | 26 |
| R-SSH-11 | **Saved sessions** (PuTTY's Session page). A session keeps its name, host, port, user name and every setting of later phases; a "Default Settings" entry holds what a new session starts with. The start screen lists them with search; double-click or Enter opens one; New, Edit, Duplicate, Delete (asked first). No passwords or passphrases are saved (D16). Kept in Parley SSH's settings, read back by a test from a saved file. | 27 |
| R-SSH-12 | **Sign on with a key file** (RFC 4252 §7). A session may name private key files: OpenSSH format (Ed25519, ECDSA, RSA with `rsa-sha2-256/512`) and PuTTY's `.ppk` versions 2 and 3. Publickey is tried before keyboard-interactive and password. An encrypted key's passphrase is asked each time and never kept; a key file that cannot be read or decrypted is said in plain words. Keys are zeroed from memory when the connection ends. | 27 |
| R-SSH-13 | **Sign on through an agent** (draft-miller-ssh-agent). When an agent is running, its keys are tried first: `SSH_AUTH_SOCK` on Linux and macOS; on Windows OpenSSH's agent (`\\.\pipe\openssh-ssh-agent`) and Pageant. A session may turn the agent off, or name the one key to use. Agent forwarding (`auth-agent-req@openssh.com`) is a session setting, off by default, with a hint that the server's admin can use the forwarded keys. | 27 |
| R-SSH-14 | **A command line** as PuTTY's and OpenSSH's: `parley-ssh [user@]host[:port]`, `-p port`, `-l user`, `-i keyfile`, `-load "saved session"`, `-A`/`-a` (agent forwarding on or off); later phases add `-L`, `-R`, `-D`, `-J`. Wrong arguments print a `parley-ssh:` message and the usage, and the program ends with a non-zero status. | 27 |
| R-SSH-15 | **More sessions.** Session ▸ New session (Ctrl+Shift+N) and Duplicate session (Ctrl+Shift+D) open another `parley-ssh` window, a new program as PuTTY does; Duplicate connects to the same host with the same settings and asks for its own password. | 27 |
| R-SSH-16 | **The Event Log** (PuTTY's). A dialog listing what happened while connecting and after, with times: the address and port, the server's version string, the algorithms chosen, the host key and its fingerprint, how it was checked, each sign-on method tried and its outcome, channels opened and closed, rekeys, the disconnect and its reason. Never a password, passphrase or key. Copy selects all of it. | 27 |
| R-SSH-17 | **Keyboard settings** (PuTTY's Keyboard and Features pages): Backspace sends `DEL` or `^H`; Home/End as standard or rxvt; function keys as xterm, ESC[n~, Linux, Xterm R6, VT400, VT100+ or SCO; the initial state of the cursor keys and keypad; a switch to refuse application cursor/keypad mode; Alt as `ESC` prefix or as Meta; AltGr as AltGr or as Ctrl+Alt. | 28 |
| R-SSH-18 | **The mouse for remote programs** (xterm's mouse tracking): modes 9, 1000, 1002 and 1003 with the default, UTF-8 (1005) and SGR (1006) encodings; a session setting turns it off; Shift always selects locally. | 28 |
| R-SSH-19 | **The bell** (PuTTY's Bell page): none, a sound (as D37's beep, played by the launcher) or a flash of the screen; flash the taskbar entry while the window is not focused; a limit on bells per second. | 28 |
| R-SSH-20 | **Appearance per session** (PuTTY's Window, Appearance and Colours pages): font and size, cursor block, underline or bar and whether it blinks, bold as a font or as a bright colour, colour schemes (Parley's, PuTTY's default, Solarized light and dark, and the 16 ANSI colours, foreground, background and cursor edited by hand), columns and rows at start, what a resize does (change rows and columns, or the font), scrollback length, scroll on key or on output. | 28 |
| R-SSH-21 | **Selection settings** (PuTTY's Selection page): copy on select with a right or middle click pasting (PuTTY's way) or the right-click menu (R-SSH-9); rectangular selection with Alt+drag; which characters make a word for a double click; copy with colours (HTML and RTF beside the text). | 28 |
| R-SSH-22 | **Character sets** (PuTTY's Translation page): the remote character set, UTF-8 by default, or ISO 8859-1..16, CP437, CP850, CP1252 or KOI8-R; ambiguous-width characters narrow or wide; line drawing as Unicode or as the font's. | 28 |
| R-SSH-23 | **Terminal options** (PuTTY's Terminal page): auto-wrap on at start, implicit CR in every LF and LF in every CR, the answerback to `^E`, whether the host may set the window title, and whether it may resize the window. | 28 |
| R-SSH-24 | **Find in the scrollback** (Ctrl+Shift+F): the find bar of Parley 5250 (`terminal/find`), over the screen and the scrollback, with match case and next/previous. | 28 |
| R-SSH-25 | **Session logs** (PuTTY's Logging page): none, printable output, all output, or SSH packets (sign-on data and passwords blanked); to a file named from a pattern with `&H` host, `&Y&M&D` date, `&T` time; if the file exists, ask, overwrite or append; flush often. | 28 |
| R-SSH-26 | **Keeping the connection** (PuTTY's Connection page): keepalives every N seconds (`keepalive@openssh.com`), TCP keepalives, Nagle off, IPv4, IPv6 or either; a lost connection is shown as such, with Restart. | 29 |
| R-SSH-27 | **Algorithms and rekeying** (PuTTY's SSH, Kex, Host keys and Cipher pages): the order of key exchanges, host-key types and ciphers, with a "warn below here" line as PuTTY's, and older ones (`diffie-hellman-group1-sha1`, `ssh-rsa` with SHA-1, `aes-cbc`, `3des-cbc`) offered only when moved above it; rekey after N minutes or N bytes; `zlib@openssh.com` compression. | 29 |
| R-SSH-28 | **What runs on the server** (PuTTY's SSH and Data pages): a remote command instead of the shell, or no shell at all (for tunnels only); the terminal type string; environment variables (`env` requests); terminal modes; no pseudo-terminal; a user name taken from the session or asked. | 29 |
| R-SSH-29 | **Proxies and jump hosts** (PuTTY's Proxy page, OpenSSH's `ProxyJump`): SOCKS 4, SOCKS 5 and HTTP CONNECT proxies, a local proxy command, and one or more jump hosts reached through `direct-tcpip`, each with its own host-key check and sign-on; proxy passwords asked, not kept; which hosts go direct. | 29 |
| R-SSH-30 | **Local port forwarding** (`-L`, RFC 4254 §7.2): listen on a local port and forward each connection to a host and port seen from the server; bound to loopback unless "accept connections from other hosts" is on. | 30 |
| R-SSH-31 | **Remote port forwarding** (`-R`, RFC 4254 §7.1): ask the server to listen and forward its connections to a local host and port. | 30 |
| R-SSH-32 | **Dynamic forwarding** (`-D`): a local SOCKS 4 and 5 server whose connections go out through the server. | 30 |
| R-SSH-33 | **X11 forwarding** (RFC 4254 §6.3): off by default; the X display and its MIT-MAGIC-COOKIE-1 from the Xauthority file, with a fake cookie towards the server as OpenSSH does. | 30 |
| R-SSH-34 | **Tunnels while connected**: a Tunnels dialog adds and removes forwardings on an open connection; the status bar shows how many forwardings and open tunnel connections there are; a port that cannot be opened is said in the Event Log and the status bar, not in a pop-up. | 30 |
| R-SSH-35 | **An SFTP panel** (draft-ietf-secsh-filexfer-02, version 3, as OpenSSH's server): beside the terminal, on the same connection: browse the server's folders starting in the home folder, download and upload files and folders (also by dragging from and to the desktop), rename, delete (asked first), make a folder, change permissions; transfers with progress and Cancel in the status bar; overwriting asks. | 31 |
| R-SSH-36 | **SCP where there is no SFTP**: downloads and uploads through `scp -f`/`scp -t` when the server has no SFTP subsystem, in the same panel with fewer actions. | 31 |
| R-SSH-37 | **Make a key pair** (PuTTY's PuTTYgen): Ed25519 by default, ECDSA P-256/384/521 or RSA 3072/4096, with a comment and a passphrase (asked twice); save the private key as OpenSSH or `.ppk` v3 and the public key; show the public key line for `authorized_keys` with a Copy button and its SHA-256 fingerprint. | 32 |
| R-SSH-38 | **Change a key**: load a private key file (both formats), change or remove its passphrase, and save it in the other format. | 32 |

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

Next free ID: `R-SSH-39` (other areas, decisions and phases: last line of [../ROADMAP.md](../ROADMAP.md); ideas: last line of [../IDEAS.md](../IDEAS.md)).
