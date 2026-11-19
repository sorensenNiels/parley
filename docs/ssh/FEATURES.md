# Parley SSH features

> Checks of Parley SSH requirements still in progress, by requirement ID. The requirements are in
> [ROADMAP.md](ROADMAP.md). Add an entry only when the requirement is fully met, has automated tests where practical,
> and has been checked in the running app on at least one OS; note which OSes were checked. Then move the
> requirement's row from the roadmap, and this entry, to its phase's file in [phases/](phases/), with a row in
> [REQUIREMENTS.md](REQUIREMENTS.md).

## In progress

### Between phases: the user name asked when the session has none (R-SSH-62 built 2026-11-19, Linux under WSL)

Automated (all pass): `parley-ssh-core`'s `tests/net.rs` against an in-process russh server (no user: the host key,
then "User name" shown as typed, blank asked again, `Event::User` with the trimmed name, the password prompt and the
server's sign-on as that user, `æøå`; Cancel on the user name ends quietly). Not yet seen in the running app.

| ID | Built | Still to check in the running app |
|---|---|---|
| R-SSH-62 | `sign_on::ask_user` before signing on when `Target::user` is empty, `Event::User` taken into the session's target and source, `Target::label` without `user@` while unknown, the editor's hint. | A saved session without a user: the dialog, the tab title after signing on, Restart not asking again. |

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

### Phase 37: themes and fonts (built 2026-11-16, Linux under WSL; D49)

Automated (all pass): `parley-ssh-core`'s `term/tests.rs` (the user's cursor until the host chooses, back with
`CSI 0 SP q` and RIS); `parley-ssh-ui`'s `screen/themes.rs` (every theme listed once, dark or light as its colours
say, text and cursor readable on its background, kept by name), `fonts.rs` (the bundled faces and both fallbacks in
every face's family, a missing font falling back with its faces, the fallbacks holding powerline and emoji glyphs, a
face the font lacks faked), `screen.rs` (padding and a bigger font leave fewer cells; the cursor set),
`window_keys.rs` (Ctrl+= / Ctrl+- / Ctrl+0, Cmd on macOS), `tests.rs` (View sizes the screen's text within 8–32;
the Appearance page choosing a dark theme and the cursor with its buttons staying put). Seen: Parley SSH starts with
the new fonts (`--screenshot`, Linux under WSL). Not yet seen: the Appearance page and a session in each theme.

| ID | Built | Still to check in the running app |
|---|---|---|
| R-SSH-20 | 12 themes (`screen/themes.rs`), one for dark and one for light, previewed on Settings ▸ Appearance with a colour strip in each list; the font (bundled or installed, `font_choice` shared with Parley 5250) with real bold, italic and bold italic (JetBrains Mono's bundled faces, an installed font's own), Symbols Nerd Font Mono and Noto Emoji as fallbacks; size 8–32 with Ctrl+= / Ctrl+- / Ctrl+0 and View's three items; cursor block, bar or underline, blinking or not (DECSCUSR 0 gives it back); padding 0–24. The themes' own colours are kept: Solarized Light's text is 4.1:1 and Catppuccin Latte's cursor 2.3:1. View's Larger/Smaller/Actual size now size the screen's text; Appearance's Text size still sizes everything. | Each theme with `ls --color`, vim and htop, in light and dark; a starship or oh-my-posh prompt with Nerd symbols; emoji; bold and italic (`printf '\e[1mbold \e[3mitalic\e[0m'`) with JetBrains Mono and with an installed font; Ctrl+= / Ctrl+- / Ctrl+0; the cursor choices and vim changing the cursor; padding; the Appearance page in light and dark. |

### Phase 38: snippets and broadcast (built 2026-11-16, Linux under WSL; D50)

Automated (all pass): `parley-ssh-core`'s `snippet.rs` (placeholders once with their defaults, filled with answers
then defaults, braces that are no placeholder and `{{{{` sent as written, æøå); `parley-ssh-ui`'s `snippets.rs`
(export and import replacing by name, a file that is not one said, the picker's search, a name of its own and text
needed, only a snippet with placeholders asks, the placeholder dialog sending its answers with its buttons staying
put), `tests.rs` (Ctrl+Shift+B on and Escape twice off, Session ▸ Stop broadcasting, Broadcast to every tab; a
snippet's own key without a session saying to connect; Ctrl+Shift+S finding by name and asking for a placeholder;
the Snippets menu and Settings ▸ Snippets with a missing name said and the dialog keeping its size), `dialogs.rs`
(the longer shortcut sheet scrolls in a small window). Not yet seen in the running app; delivery to other panes'
hosts is not covered by a test (it needs connected sessions).

| ID | Built | Still to check in the running app |
|---|---|---|
| R-SSH-49 | `Settings::snippets`; Settings ▸ Snippets (add, edit, move, delete asked twice, a key caught as the Keyboard page does, Enter after it); Export… and Import… through the system's file window (`SnippetFileWindow`, one JSON file); the Snippets menu; the picker (Ctrl+Shift+S, Cmd+S on macOS); placeholders asked in one dialog; sent as a paste (bracketed when the host asked) then Enter. | A snippet into bash with and without Enter; a multi-line one; placeholders with defaults; a snippet's own key; export, then import on another OS; the page and dialogs in light and dark. |
| R-SSH-50 | Session ▸ Broadcast to this tab (Ctrl+Shift+B, Cmd+B on macOS) and to every tab, Stop broadcasting; keys and pastes typed in the pane in front go to every other session pane in scope, a paste encoded for each pane's own modes; a border and a "Broadcast" badge on each pane in scope; "Broadcasting to N panes" in the status bar; Escape twice within 0.6 s stops; snippets broadcast too. Mouse and focus reports stay with their own pane. | Three panes typing `uptime` at once; a paste into panes where one has bracketed paste on; every tab; Escape twice; the marks in light and dark. |

### Phase 39: reconnect and restore (built 2026-11-16, Linux under WSL; D50)

Automated (all pass): `parley-ssh-core` `tests/net.rs` (a refused host key ends as `Ending::Refused`, which never
reconnects); `parley-ssh-ui`'s `reconnect.rs` (only a connection lost while its shell ran comes back; waits of 1, 2,
4, 8, 16 … seconds, at most 30, until the minutes run out), `session.rs` (lost, a failed try with its wait said on the
screen, the event log kept across tries, Stop; nothing when off or when the shell never ran), `tests.rs` (two tabs, a
split with two sessions, kept and opened again with both panes connecting when shown; restore off; a host on the
command line winning; the Terminal page's Reconnect and Restore settings). Not yet seen in the running app.

| ID | Built | Still to check in the running app |
|---|---|---|
| R-SSH-51 | Settings ▸ Terminal ▸ Connections: "Reconnect a lost connection" (on) for 1–60 minutes (10). A connection that fails after its shell ran (`Ending::Failed`; refusals are `Ending::Refused`) is tried again on the same screen after 1, 2, 4 … 30 s; faint lines on the screen and in the event log say each step; the status bar counts down with Restart now and Stop; a key or the agent signs on again by itself, a password is asked (D16); "Reconnected: this is a new shell." A session in a tab not in front reconnects when its tab is shown. | Pull the network cable (or `sudo ss -K` the connection) during `top`; sleep the laptop; a password session asking again; Stop; Restart now; the limit running out. |
| R-SSH-52 | Settings ▸ Terminal ▸ Connections: "Open the tabs of last time" (off). `settings_to_save` keeps each tab's layout with its shares and each pane's session (`SavedTab`, never a password) whenever Parley SSH saves its settings; on a start without a host they open again, panes renumbered, a saved session as it is now (one since deleted as it was), connecting when the tab is first shown; screens start empty (D50). | Quit with three tabs (one split), start again; with restore off; `parley-ssh host` with restore on; a deleted saved session. |

### Phase 29: the connection, complete (built 2026-11-16, Linux under WSL; D51)

Automated (all pass): `parley-ssh-core`'s `tests/net.rs` against the in-process server: a command run instead of
the shell with `LANG` taken, another variable refused and the terminal type `xterm`; a SOCKS 5 proxy asking its
password and carrying the connection; an HTTP proxy answering 407, the password asked and the connection made again
with Basic; an HTTP proxy without a user saying what is missing; a proxy command (a Python relay, Unix) carrying the
connection; IPv6 only to an IPv4 address said. `net/client.rs` (keepalive, three unanswered, rekey limits capped at
1 GiB, compression first only when on), `net/transport.rs` (proxy command tokens, HTTP status and bracketed IPv6),
`openssh_config.rs` (`RemoteCommand`, `SetEnv`, `AddressFamily`, `Compression`, `ProxyCommand`, `none`);
`parley-ssh-ui`'s `sessions/connection.rs` (transport and shell from a session, environment lines, a proxy's host or
command needed), `sessions.rs` (the editor's three pages with Save staying put, a missing proxy host said, saving
proxy and environment), `openssh.rs` (the config's keywords into a session); `parley-app`'s `ssh_start.rs` (what
follows the host is the command). Not yet seen in the running app.

| ID | Built | Still to check in the running app |
|---|---|---|
| R-SSH-26 | Session ▸ Connection: keepalive every N s (30; 0 none), three unanswered end the connection (then R-SSH-51 reconnects); IP version Either, IPv4 or IPv6 (`AddressFamily`); TCP keepalives on and Nagle off always (`socket2`). | A server that goes away silently (firewall drop) is noticed within about 90 s; IPv6 to a dual-stack host. |
| R-SSH-27 | New keys after N minutes (60) and N MB (1024, at most 1 GiB); compression switch (off; `Compression yes`). | A long session past the rekey time; compression on a slow link; the event log's algorithms. |
| R-SSH-28 | Session ▸ On the server: a command instead of the shell, still with a pseudo-terminal (`RemoteCommand`; `parley-ssh host cmd…`); environment `NAME=value` lines (`SetEnv`), each answered by the server and a refusal said in the event log; the terminal type. | `tmux new -A -s main`; `LANG` with OpenSSH's default `AcceptEnv`; a variable it refuses; `TERM=xterm` in vim. |
| R-SSH-29 | Session ▸ Connection ▸ Proxy: None, SOCKS 5, HTTP (host, port, user; the password asked when the proxy wants one, never kept) or a command (`%h`, `%p`, `%r`; `ProxyCommand`); a proxy reaches the first jump host; its refusals in plain words. | A real SOCKS 5 proxy (`ssh -D` on another machine), an HTTP proxy such as Squid with and without a password, `ProxyCommand nc …`, a proxy with jump hosts; Windows' `cmd /C` for a proxy command. |

### Phase 28: the terminal, lean (R-SSH-24 built 2026-11-16, R-SSH-19 and R-SSH-25 2026-11-17, Linux under WSL)

Automated (all pass): `parley-ssh-core`'s `term/find.rs` (matches in the scrollback and on the screen with and
without case, not overlapping, wide characters and æøå as cells; a match scrolled into view; the change counter),
`parley-ssh-ui`'s `find.rs` (the current match scrolled to, only those in view given, Match case), `session.rs`
(the bar over a pane: "1 of 2", Match case to "1 of 1", Close clearing the highlights), `window_keys.rs`
(Ctrl+Shift+F, Ctrl+F still the host's). R-SSH-19 and R-SSH-25: `parley-ssh-core`'s `term/plain.rs` (escape
sequences, carriage returns and strings left out, also cut in two by the network, a UTF-8 character too; recording
only while asked), `parley-ssh-ui`'s `log.rs` (a file named from its pattern with `~`; a text log and a raw one
appended to the same file), `session.rs` (a log keeps only what came while it was on; the bell noticed). Not yet
seen in the running app.

While building: the snippet picker's "No snippets yet" hint used `▸`, which the bundled fonts lack; opening the
picker without snippets would have crashed (Phase 38). Fixed.

| ID | Built | Still to check in the running app |
|---|---|---|
| R-SSH-19 | Settings ▸ Terminal ▸ Bell: Flash (default: the pane's border, 0.2 s), Sound (the launcher's beep, D37) or None; while the window is not focused the taskbar asks for attention; bells from tabs not in front too. | `printf '\a'`; tab completion with nothing to complete in bash; each setting; Windows' and macOS's taskbar or dock. |
| R-SSH-25 | Session editor ▸ On the server ▸ Session log: Off, Text (without escape sequences) or Raw output, to a file from a pattern (`~/parley-ssh-logs/&H-&Y&M&D-&T.log`), appended to with a line saying when it started; started when the shell starts; Session ▸ Start log / Stop log for the session in front (text when the session has none); an error said in the status bar and the log stopped; kept across reconnects. | A text log of vim and htop (readable?), a raw log replayed with `cat`; a folder that cannot be written; Windows paths. |
| R-SSH-24 | Edit ▸ Find… and Ctrl+Shift+F (Cmd+F on macOS) open Parley 5250's find bar over the pane, with Match case; Enter and Shift+Enter or the arrows go to the next and previous match, scrolled into view; matches in view are highlighted in the theme's yellow, the current one solid; Escape or 🗙 closes. Searched again only when the text, Match case or the output changed. | `dmesg` or a long `journalctl` with 10,000 lines of scrollback; matches while output keeps coming; each theme's highlight; Escape giving the keyboard back to the shell. |


### Phase 35: shell integration (built 2026-11-16, Linux under WSL)

Automated (all pass): `parley-ssh-core`'s `term/integration.rs` (an OSC 8 link over its text and ended by an empty
URI; written addresses without the punctuation after them, a balanced bracket kept; OSC 133 prompts, output and exit
status; marks of dropped lines gone and none on the alternate screen; OSC 9 and 777 notifications but not ConEmu's
numbered OSC 9; OSC 52 set, a query never answered), `term/find.rs` (a line put at the top); `parley-ssh-ui`'s
`screen.rs` (Ctrl+click on a link opens it and sends nothing to the host), `screen/scrollbar.rs` (Ctrl+Shift+Up and
Down from command to command; a command's output from any of its lines; a failed command's mark red), `session.rs`
(a command over 10 s and the host's notifications become notes, a short one not; the clipboard set only when the
setting allows). Not yet seen in the running app.

While building: the screen had no scroll bar although R-SSH-8 names one; a slim one at the right edge now scrolls by
drag or click and carries R-SSH-45's marks. And eframe's `links` feature was off, so egui's links opened nothing on
the desktop in all three programs (Parley 5250's addresses on the screen, the newer-version link): it is on now.

| ID | Built | Still to check in the running app |
|---|---|---|
| R-SSH-44 | OSC 8 links and written `http(s)://`, `ftp://`, `file://` addresses: underlined under the pointer, Ctrl+click (Cmd+click) opens in the browser (logged), also while a program has the mouse; right-click ▸ Copy link. | `ls --hyperlink=auto`, `gcc` errors with links, a URL in `git log`; Cmd+click on macOS; Windows' default browser. |
| R-SSH-45 | OSC 133 A/C/D marks; each prompt marked on the scroll bar, red when its command failed; Ctrl+Shift+Up/Down (Cmd+Up/Down) to the previous and next command; right-click on a command ▸ Select command output, Copy command output. | bash with `bash-preexec` or VS Code's/WezTerm's integration script, zsh, fish 4 (built in), PowerShell with its prompt marks; long output; the scroll bar in each theme. |
| R-SSH-46 | OSC 9 and OSC 777;notify from the host, and a command (OSC 133) that ran 10 s or more ending, as a desktop notification (`notify-rust` in the launcher, on a thread) and the taskbar asking for attention, only while the window is not focused; from tabs not in front too. | `sleep 12` in another window; `printf '\e]9;Build done\a'`; notifications on Windows, macOS and Linux (GNOME, KDE). |
| R-SSH-47 | OSC 52 sets the clipboard (Settings ▸ Terminal: "Programs on the server may set the clipboard", on); never read; each time in the event log. | tmux with `set -g set-clipboard on`, vim's OSC 52 yank, over a jump host; the setting off. |

### Phase 36: the command palette (built 2026-11-16, Linux under WSL)

Automated (all pass): `parley-ssh-ui`'s `palette.rs` (words, letters in order, a swapped pair and two typos in a
long query found, none in three letters, æøå; recent first, then the best fit, the detail searched; each recent key
once, at most 20), `tests.rs` (Ctrl+Shift+P, "setings" running Settings and listed first next time, snippets and
shortcuts listed; a saved session connecting in the start screen, then in a new tab, kept once as a recent
connection without its name). Not yet seen in the running app.

| ID | Built | Still to check in the running app |
|---|---|---|
| R-SSH-48 | Ctrl+Shift+P (Cmd+Shift+P): "Go to…" over the window finds open tabs, saved sessions, `~/.ssh/config` hosts, the last ten connections, snippets and every menu action, each with its shortcut; matches whole words, letters in order and a typo (from four letters); arrows and Enter or a click; the 20 chosen last first (`palette_recent`, kept in the settings). A session, host or recent connection opens in the start screen in front, else in a new tab. | Many sessions and config hosts; typing fast; macOS's Cmd+Shift+P; light and dark. |
