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
