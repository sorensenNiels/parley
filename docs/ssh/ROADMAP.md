# Parley SSH roadmap

> The plan for Parley SSH: an SSH client for Windows, Linux and macOS in Parley's look, the program `parley-ssh`, and
> the engine under it. Finished phases, with the text of their requirements and how each was verified, will be in
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
| Phase 26: a first shell | In progress (2026-11-16): R-SSH-1..10 built and tested ([FEATURES.md](FEATURES.md)); to be checked in the running app against real servers. |
| Phase 27: saved sessions and keys | In progress (2026-11-16): R-SSH-11..16 built and tested ([FEATURES.md](FEATURES.md)); to be checked in the running app. |
| Phase 33: tabs and split panes | In progress (2026-11-16): R-SSH-39..41 built and tested ([FEATURES.md](FEATURES.md)); to be checked in the running app. |
| Phase 34: hosts from `~/.ssh/config` | Planned. |
| Phase 35: shell integration | Planned. |
| Phase 36: the command palette | Planned. |
| Phase 28: the terminal, lean | Planned; cut down by D46. |
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

### Phase 26 — a first shell

The smallest Parley SSH that is useful: open `parley-ssh`, type a host, user name and password, check the host key,
and work in a shell with full-screen programs (vim, less, htop, tmux) in colour. R-SSH-1..10.

### Phase 27 — saved sessions and keys

What makes it a daily tool: saved sessions with default settings, sign-on with a key file or an agent instead of a
password, a command line as PuTTY's and OpenSSH's, the Event Log, and starting another session. R-SSH-11..16.

### Phase 28 — the terminal, lean

Cut down by D46 to what modern use needs: the mouse for vim, tmux and htop (R-SSH-18), find in the scrollback
(R-SSH-24), a simple bell (R-SSH-19) and session logs (R-SSH-25). Appearance moves to Phase 37 (R-SSH-20). Dropped:
PuTTY's keyboard styles (R-SSH-17), selection options (R-SSH-21), character sets other than UTF-8 (R-SSH-22) and its
terminal options (R-SSH-23).

### Phase 33 — tabs and split panes

From IDEA-37 (D47): several sessions in one window, as tabs and as panes side by side or one over the other, with the
keys people know from Windows Terminal and iTerm2. R-SSH-39..41.

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
| R-SSH-11 | **Saved sessions** (PuTTY's Session page). A session keeps its name, host, port, user name and every setting of later phases; a "Default Settings" entry holds what a new session starts with. The start screen lists them with search (from four sessions on); Connect, or Enter in the search, opens one (*changed 2026-11-16: no double-click, the cards are Parley's shared ones, whose buttons a card-wide click would cover*); New, Edit, Delete (asked first); a quick connect below the list opens a host that is not saved, with the defaults' keys and agent (*changed 2026-11-16: Duplicate is R-SSH-15's window; Duplicate in the editor was left out*). No passwords or passphrases are saved (D16). Kept in Parley SSH's settings, read back by a test from a saved file. | 27 |
| R-SSH-12 | **Sign on with a key file** (RFC 4252 §7). A session may name private key files: OpenSSH format (Ed25519, ECDSA, RSA with `rsa-sha2-256/512`) and PuTTY's `.ppk` versions 2 and 3. Publickey is tried before keyboard-interactive and password. An encrypted key's passphrase is asked each time and never kept; a key file that cannot be read or decrypted is said in plain words. Keys are zeroed from memory when the connection ends. | 27 |
| R-SSH-13 | **Sign on through an agent** (draft-miller-ssh-agent). When an agent is running, its keys are tried first: `SSH_AUTH_SOCK` on Linux and macOS; on Windows OpenSSH's agent (`\\.\pipe\openssh-ssh-agent`) and Pageant. A session may turn the agent off, or name the one key to use. Agent forwarding (`auth-agent-req@openssh.com`) is a session setting, off by default, with a hint that the server's admin can use the forwarded keys. | 27 |
| R-SSH-14 | **A command line** as PuTTY's and OpenSSH's: `parley-ssh [user@]host[:port]`, `-p port` (also PuTTY's `-P`), `-l user`, `-i keyfile`, `-load "saved session"`, `-A`/`-a` (agent forwarding on or off); later phases add `-L`, `-R`, `-D`, `-J`. Wrong arguments print a `parley-ssh:` message and the usage, and the program ends with a non-zero status. | 27 |
| R-SSH-15 | **More sessions.** Session ▸ New session (Ctrl+Shift+N) and Duplicate session (Ctrl+Shift+D) open another `parley-ssh` window, a new program as PuTTY does; Duplicate connects to the same host with the same settings and asks for its own password. | 27 |
| R-SSH-16 | **The Event Log** (PuTTY's). A dialog listing what happened while connecting and after, with times: the address and port, the server's version string, the algorithms chosen, the host key and its fingerprint, how it was checked, each sign-on method tried and its outcome, channels opened and closed, rekeys, the disconnect and its reason. Never a password, passphrase or key. Copy selects all of it. | 27 |
| R-SSH-17 | ~~**Keyboard settings** (PuTTY's Keyboard and Features pages): Backspace sends `DEL` or `^H`; Home/End as standard or rxvt; function keys as xterm, ESC[n~, Linux, Xterm R6, VT400, VT100+ or SCO; the initial state of the cursor keys and keypad; a switch to refuse application cursor/keypad mode; Alt as `ESC` prefix or as Meta; AltGr as AltGr or as Ctrl+Alt.~~ *Dropped 2026-11-16 (D46): legacy options; UTF-8 and xterm's keys and behaviour only.* | 28 |
| R-SSH-18 | **The mouse for remote programs** (xterm's mouse tracking): modes 9, 1000, 1002 and 1003 with the default, UTF-8 (1005) and SGR (1006) encodings; a session setting turns it off; Shift always selects locally. | 28 |
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
| R-SSH-39 | **Tabs** (2026-11-16, IDEA-37, D47). Each session is a tab under the header, with its title (what the host set, else `user@host`), a state dot, and a close button; the tab bar shows once there are two tabs. New tab (Ctrl+Shift+T) opens the start screen in a tab; Close tab (Ctrl+Shift+W) asks first while connected (R-SSH-10's setting); Ctrl+Tab and Ctrl+Shift+Tab go to the next and previous tab, Ctrl+Alt+1…9 to a tab by number (Cmd+1…9 on macOS; *changed 2026-11-16 while building: Ctrl+2…8 are control characters for the host, so Windows Terminal's Ctrl+Alt*); a middle click closes a tab; tabs are reordered by dragging. The header's pill, the status bar, the event log and the Edit menu follow the tab in front. Closing the last tab shows the start screen. | 33 |
| R-SSH-40 | **Split panes** (2026-11-16, IDEA-37, D47). A tab holds one or more panes: Split right (Alt+Shift+=) and Split down (Alt+Shift+-), as Windows Terminal, open the same session's host in a new pane beside or below the one in front; Alt+arrow moves to the pane in that direction; the divider is dragged to resize; Close pane (Ctrl+Shift+W in a split tab) closes the pane in front. Each pane has its own connection, terminal and size; the one in front has a thin accent border. | 33 |
| R-SSH-41 | **New, Duplicate and windows with tabs** (2026-11-16, D47; *changes R-SSH-15*). New session (Ctrl+Shift+N) opens a new tab; Duplicate session (Ctrl+Shift+D) opens the session in front again in a new tab; New window (Ctrl+Shift+Alt+N) starts another `parley-ssh` as R-SSH-15 did. A session opened from the command line (R-SSH-14) is the first tab. | 33 |
| R-SSH-42 | **Hosts from `~/.ssh/config`** (2026-11-16, IDEA-38). The `Host` entries of OpenSSH's config (`~/.ssh/config` and its `Include`s; wildcard patterns are not listed but apply), read-only, in a group of their own on the start screen and in the palette, with `HostName`, `User`, `Port`, `IdentityFile` and `ProxyJump` used as OpenSSH uses them (first value wins); re-read when the file changes. Details when Phase 34 is planned. | 34 |
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

Next free ID: `R-SSH-58` (other areas, decisions and phases: last line of [../ROADMAP.md](../ROADMAP.md); ideas: last line of [../IDEAS.md](../IDEAS.md)).
