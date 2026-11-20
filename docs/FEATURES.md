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
  and is no longer stored (D38); window frames come from R-UI-26 instead.

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
