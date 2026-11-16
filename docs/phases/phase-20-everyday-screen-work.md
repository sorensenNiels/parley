# Phase 20 — Everyday screen work

> Finished phase of Parley 5250. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

Chosen by the owner (2026-11-15) as the first terminal phase after Phase 4: what makes a day at the green screen
easier, all in the UI, without new protocol. In this order: hotspots (R-UI-20, from IDEA-19), find on the screen
(R-UI-21, IDEA-22), selecting and copying part of the screen, also as a table (R-UI-22, IDEA-20), 5250 colour schemes
(R-UI-23, IDEA-23) and reconnecting by itself when the network drops (R-NET-10, IDEA-16). Then, as candidates for
later phases: the protocol gaps (IDEA-7 first, then IDEA-4, 6, 2, 3, 5, 14), several sessions as tabs (IDEA-15),
importing sessions from ACS and PCOMM (IDEA-17).

**Status:** Done (2026-11-15): checked by the owner in the running app (FEATURES.md): R-UI-20..23, R-NET-10.

## Requirements

| ID | Requirement |
|---|---|
| R-NET-10 | **Reconnect when the connection drops** (2026-11-15, from IDEA-16; the owner chose automatic retries, per session). A session setting "Reconnect when the connection drops", off by default. When it is on and the connection ends by a network failure (a read, write or TLS error; not when the host ends the job and closes the connection, not on Disconnect, and not when the first connect fails), Parley connects again after 2, 4, 8, 16, 32 and then every 60 s; the status bar shows "Reconnecting in 8 s" with Cancel, and Connect again connects at once. A connect that fails waits for the next try. Once connected, the session's Run macro runs again as on any connect (R-KEY-4). The Offline note says whether the network failed or the host ended the session. |
| R-UI-20 | **Hotspots** (2026-11-15, from IDEA-19; the owner chose a plain click). Text on the screen outside input fields that names something to do is a hotspot: hovering it underlines it and shows a hand pointer, and a plain click does it; a click anywhere else still moves the cursor (R-UI-1). The hotspots: a function key and its label (`F3=Exit`, `F12=Cancel`, also with blanks around `=`, F1–F24) presses that key; an option and its label (`2=Change`, `4=Delete`: one or two digits, not after `F`) types the number into the option field, the input field on the cursor's row or else the first input field after the legend, replacing what was there, and puts the cursor after it, without pressing Enter; `More...` (or `More…`), `Bottom` and a `+` alone at the end of a row send Roll Up (Page Down); a URL (`http://` or `https://` up to the next blank) opens in the browser. While the keyboard is locked or in error, a click on a key hotspot does what that key would do then (nothing, or Reset rules as R-KEY-1). A hotspot click while recording a macro is recorded as the key or text it sends (R-KEY-3). |
| R-UI-21 | **Find on the screen** (2026-11-15, from IDEA-22). Ctrl+F (a new bindable function `find` in the Default map, R-KEY-2) and Edit ▸ Find… open a find bar above the screen with a text field; every match on the screen is highlighted as the text is typed (case is ignored, `ÆØÅ` finds `æøå`; accents are not, `aeo` does not find `æøå`), with "n of m" beside it; Enter and Shift+Enter (or the arrows beside the count) move to the next and previous match, shown stronger; a match may not span rows. The highlights follow the screen as the host changes it. The 5250 cursor does not move. Esc or 🗙 closes the bar and gives the keyboard back to the screen. |
| R-UI-22 | **Select and copy part of the screen, also as a table** (2026-11-15, from IDEA-20; the owner chose drag plus Copy as table). Dragging with the left button selects a rectangle of positions, drawn highlighted; a click without a drag still moves the cursor or presses a hotspot (R-UI-20). Copy (Ctrl+C) copies the selection as text, one line per row with trailing blanks trimmed, or the whole screen when nothing is selected (R-UI-1). A new bindable function `copy-table` (Ctrl+T in the Default map, also in Edit; not Ctrl+Shift+C, which a map cannot tell from Ctrl+C, R-KEY-2) copies the selection, or the whole screen, as tab-separated text: a column ends at two or more screen columns blank on every row of the selection, or at one blank on every row that holds a field attribute on one of them, so a list's columns paste into a spreadsheet as columns; each cell is trimmed. The selection goes when a key is typed, the screen changes, or a click lands elsewhere. |
| R-UI-23 | **5250 colour schemes** (2026-11-15, from IDEA-23; the owner chose a default in the settings plus one per session). Three schemes: **Classic** (today's colours on black), **Light** (the seven 5250 colours darkened to read on a light background) and **Custom** (a background and the seven colours chosen by the user, one custom set for Parley). Settings ▸ Terminal chooses the default scheme, with a preview of the seven colours on the background (beside the terminal font, which is there too); the session dialog has "5250 colours: As in settings / Classic / Light / Custom", so production can look different from test. Changes show at once; Cancel restores them. The cursor, reverse image, column separators and the find and selection highlights (R-UI-21, R-UI-22) use the scheme's colours. |

## Verification

### Phase 20: everyday screen work (2026-11-15) — R-UI-20..23, R-NET-10

Checked by the owner in the running app (2026-11-15); `make verify` on Linux under WSL.

- **R-UI-20** hotspots. **Automated**: in parley-core, `F3=Exit`-style legends (labels of several words, blanks
  around `=`, F25 and text inside words refused), options (`2=Change`), `More...`/`More…`/`Bottom`/a lone `+` at the
  end of a row, URLs (`æøå` in them), input fields never hotspots; an option goes into the field on the cursor's
  row, else the first after the legend, else nowhere; a recorded option keeps its `move`. In kittest: a click on
  F3=Exit sends F3, on an option types it without Enter, on a URL opens it, a hand under the pointer, a click beside
  still moves the cursor. **Against PUB400.COM** (probe): the sign-on screen's two URLs are its only hotspots.

- **R-UI-21** find on the screen. **Automated**: case ignored, accents not, no overlaps, nothing across rows; in
  kittest Ctrl+F opens the bar, Enter and Shift+Enter step with the keyboard kept, "No matches", Esc and 🗙 close,
  Edit ▸ Find… opens it focused.

- **R-UI-22** select and copy, also as a table. **Automated**: a rectangle as lines (trimmed, cut at the screen edge),
  as a table split at blank columns (one blank column only at a field attribute), blank rows left out; in kittest a
  drag selects without moving the cursor and Copy copies it, the whole screen without one, a key, a click or a new
  screen drops it, Ctrl+T copies a table. Copy as table is Ctrl+T, not Ctrl+Shift+C (a map cannot tell that from
  Ctrl+C, R-KEY-2); keyboard maps copied before this version do not have Find and Copy as table until bound.

- **R-UI-23** colour schemes. **Automated**: every Light colour reads on its background (WCAG 4.5:1), Custom starts
  as Classic; in kittest Settings ▸ Terminal changes the screen at once and Cancel puts it back, Custom's colour
  buttons keep the dialog in place, a session's scheme wins over the settings', the session dialog saves one.

- **R-NET-10** reconnect when the connection drops. **Automated**: in parley-core a reset after connecting is
  `Dropped`, a host closing in order is `Closed`; the waits 2, 4 … 60 s; in kittest (local sockets) a dropped
  connection counts down and connects again, a failed try waits longer, Cancel stops it, without the setting it
  stays offline, the session dialog turns it on.
