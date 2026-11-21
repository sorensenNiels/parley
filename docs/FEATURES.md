# Parley features

> Features that are implemented **and verified**, by requirement ID. The requirements they were built to are in
> [REQUIREMENTS.md](REQUIREMENTS.md); open work and new requirements are in [ROADMAP.md](ROADMAP.md).
> Look an ID up with `grep -n` rather than reading the whole file.
> SQL (the SQL window, Parley SQL) has its own: [sql/FEATURES.md](sql/FEATURES.md).
>
> Add an entry only when the requirement is fully met, has automated tests where practical, and has
> been checked in the running app on at least one OS. Note which OSes were checked. Then move the
> requirement's row from the roadmap, and this entry, to its phase's file in [phases/](phases/), with a row in
> [REQUIREMENTS.md](REQUIREMENTS.md).

**Verified on:** Windows, macOS, Linux (desktop) and Linux under WSL2/WSLg, by the owner in the running app (2026-11-12);
Claude's checks ran on Linux/WSL2 against PUB400.COM (IBM i 7.5) and the owner's IBM i (sign-on screen only).

## In progress

- **R-UI-37** (step 1 of Phase 42 built 2026-11-21, Linux under WSL; D61): screens kept in the engine
  (`parley-core`'s `session/history.rs`). **Automated** (`make test`): the same screen twice is kept once, the oldest
  go past the limit, and the limit stays within 10 to 500; text after a non-display attribute is blanked in the copy
  and looks the same; on a whole session (`tests/session.rs`) the sign-on screen is kept with the cursor's place, a
  write that leaves the keyboard locked keeps nothing and its unlock keeps the screen with "æøå"; a typed password
  is in the live screen but in no kept screen; the history and its limit go over to the session that replaces it.
  **Still to build**: the view (R-UI-38), saving and the setting (R-UI-39), keeping the history across a reconnect in
  the UI.

- **R-DS-11, R-DS-12** (built 2026-11-20, Linux under WSL; D57): GUI selection fields and the mouse; scroll bars,
  Write Data and programmable mouse buttons, behind the session switch "GUI elements (beta)", which is back in the
  session dialog and stored. **Automated** (`make test`): `parley-core`'s `screen/selection.rs` (radio buttons, check
  boxes and a menu bar laid out and written, selecting and its reply bytes, a field or scroll bar that does not fit,
  removing and clearing, a slider kept inside), `session/mouse.rs` (the event codes), `tests/gui.rs` with IBM i's own
  bytes where published (a host's radio buttons chosen with Down and Space and sent as X'0021'; IBM i's menu bar
  clicked and a mnemonic, sent as X'0023' and X'0021'; IBM i's push buttons by mnemonic as X'0022', Enter, and the
  Roll Right AID; Remove GUI Selection Field; a field only for displays without a mouse skipped; a choice not
  available; a field cut short; a scroll bar moved and sent with Roll Up, one past the bottom, removed; Write Data
  with æøå; one- and two-event mouse buttons; nothing without the switch), `tests/session.rs` (the query reply's
  pointer device and level 2; the switch stored), `parley-ui-egui`'s `terminal.rs` (kittest: a click on a radio
  button; a scroll bar's arrow; its slider dragged to the end without selecting), `terminal/gui.rs` (arrows, pages
  and drags), `lib.rs` (the session dialog's switch). **Seen** (`--screenshot`, Linux under WSL, a fake host sending a
  menu bar, radio buttons, check boxes, a list with a scroll bar and push buttons: `scripts/gui-demo-host.py`).
  **To check in the running app**: choosing with keys, mnemonics and clicks, the scroll bar's arrows and slider, and
  what the fake host prints that Parley sent. Not seen from a real IBM i (D57).

- **R-DS-10** (2026-11-15): GUI windows behind the session switch "GUI elements (beta)" (D38). **Automated** (`make
  test`): a window is written as the 5494 writes it (border attribute and characters, centred title `Æble`, cleared
  inside) and kept; removing it puts back what it covered and frees the cursor; Remove All and Clear Unit take every
  window away; one that does not fit is refused, one at the same place and size replaces the other; a long title is
  cut and a right footer sits right; on a whole session WDSF Create Window draws it only with the switch on, keeps
  the cursor inside, Remove GUI Window takes it away, and the query reply's byte 4 is X'0E' only with the switch on;
  the UI's frame covers the border characters but not the title; the session dialog saves the switch.
  **First try on DKSOFT00** (the owner, switch on, tour macro, `PARLEY_TRACE`): the host asked the query but sent
  no WDSF at all with byte 4 = X'0E' in a 58-byte reply. The reply now claims what tn5250j's enhanced mode claims
  (byte 4 X'07', byte 5 X'08' level 3, 64 bytes); two more tries, the second on a device created fresh by that
  reply (PARLYGUI, 3477 FC), and the host still sent no WDSF. **Not seen live**: Phase 22 is paused until IBM i's
  rules for the enhanced interface are known (ROADMAP §4). The switch was then taken out of the session dialog
  and is no longer stored (D38); window frames come from R-UI-26 instead. *Resumed 2026-11-20 (D57)*: the switch is
  back and stored; to be checked in the running app with `scripts/gui-demo-host.py` once it sends a window.

## Verified

How each finished requirement was verified is in its phase file:

- [Phase 0 — Development environment](phases/phase-00-development-environment.md)
- [Phase 1 — Connect and show a screen](phases/phase-01-connect-and-show-a-screen.md)
- [Phase 2 — Saved sessions and auto sign-on](phases/phase-02-saved-sessions-and-auto-sign-on.md)
- [Phase 3 — Keyboard maps and macros](phases/phase-03-keyboard-maps-and-macros.md)
- [Phase 4 — A modern look](phases/phase-04-a-modern-look.md)
- [Phase 20 — Everyday screen work](phases/phase-20-everyday-screen-work.md)
- [Phase 21 — Protocol gaps](phases/phase-21-protocol-gaps.md)
- [Phase 24 — More settings](phases/phase-24-more-settings.md)
- [Between phases](phases/between-phases.md)
