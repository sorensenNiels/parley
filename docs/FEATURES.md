# Parley features

> Features that are implemented **and verified**, by requirement ID. The requirements they were built to are in
> [REQUIREMENTS.md](REQUIREMENTS.md); open work and new requirements are in [ROADMAP.md](ROADMAP.md).
> Look an ID up with `grep -n` rather than reading the whole file.
> SQL (the SQL window, Parley SQL) has its own: [sql/FEATURES.md](sql/FEATURES.md).
>
> Add an entry only when the requirement is fully met, has automated tests where practical, and has
> been checked in the running app on at least one OS. Note which OSes were checked. Then move the
> requirement's row from the roadmap to [REQUIREMENTS.md](REQUIREMENTS.md).

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
  rules for the enhanced interface are known (ROADMAP §4).

## Implemented

- **R-UI-18** (2026-11-14): Settings from a gear, no Help menu, built. **Automated** (`make test`): the menu bar
  starts with the app and ends with the gear, with no Help menu; Settings opens from the gear and not from Tools;
  Tools is not there while macros and SQL are both off; Check for updates runs from Settings ▸ About. **Checked in
  the running app** by the owner, Linux under WSL (2026-11-14).

- **R-UI-19** (2026-11-15): a larger gear. **Automated** (`make test`): the menu bar test checks the gear is wider
  than it was at the menus' text size. **Checked in the running app** by a screenshot of Parley and Parley SQL, Linux
  under WSL (2026-11-15).

### Phase 1: connect and show a screen (2026-11-12) — R-NET-1..3, R-DS-1..4, R-EBC-1, R-KEY-1, R-UI-1..4, R-NF-1

Verified by the owner in the running app on Windows, macOS, Linux and WSL2 (2026-11-12).

How it was checked:
- **Automated** (`make test`): telnet negotiation and framing; record header; every command and order in R-DS-1 and
  R-DS-4 with hand-built records; field rules and keys; the exact bytes of Enter, Read Immediate, the query reply and
  Save Screen; CCSID 37/277/1140/1142 tables (generated from iconv by `scripts/gen-ebcdic.py`); 6000 random records
  without a panic; a whole session over TCP against a fake host (`tests/connection.rs`); the terminal widget and form
  in egui_kittest (typing, Enter, a click moving the cursor, the screen exposed to assistive technology).
- **Against PUB400.COM** (`cargo run -p parley-core --example probe`): TN5250 negotiation on port 23 and TLS on 992
  (certificate checked against the OS store); the sign-on screen shown with both input fields and the cursor where the
  host put it; CCSID 277 (`KBDTYPE` DMB) and the 3477-FC terminal type accepted; Help answered by a Write Error Code
  ("Function key not allowed."), Reset putting the line back; Enter with empty fields answered by
  "CPF1296 - Sign-on information required." (the host accepted Parley's input record).
- **Running app** (`parley pub400.com --screenshot FILE`): the sign-on screen drawn with the host's colours, reverse
  image and underlines, the block cursor in the user field, "Ready" in the status line.


### Phase 2: saved sessions, auto sign-on and the look (2026-11-12) — R-NET-4, R-NET-5, R-UI-5..12

Verified by the owner in the running app on Windows, macOS, Linux and WSL2 (2026-11-12), except auto sign-on (R-NET-4) on
Linux. *R-NET-4 was removed afterwards with the saved passwords (R-NET-7, D16); its checks below are history.*

How it was checked:
- **Automated**: the DES and SHA-1 password substitutes against RFC 4777's test vectors (sections 5.1 and 5.2) and
  the RFC's example negotiation byte for byte; no credentials without a host seed; a truncated or escaped seed;
  typed sign-on sends the same record as an operator typing, only once; keep-alive bytes; in egui_kittest, the start
  screen, saving a session with its password in a (memory) keychain only, required name and host, delete and rename
  moving the password, one session marked for program start; the bundled terminal fonts measured as monospace (and
  Inter not), a missing font falling back, choosing a font in the settings, OK keeping it and Cancel restoring; the
  close button (×) of the settings, the session dialog and the shortcuts panel (R-UI-7); settings pages, Restore
  defaults per page, and the dialog keeping its place from frame to frame (R-UI-8); R-UI-9: Inter / Inter SemiBold /
  JetBrains Mono families, thin floating scroll bars, every symbol the UI draws present in the bundled fonts, the
  session dialog keeping its place. Screenshots (Linux/WSLg): start screen, PUB400 connected, session dialog and
  settings dialog in dark, settings dialog in light. R-UI-10: the name (and icon) in the menu bar and Help at its right
  edge, in kittest and a screenshot. R-UI-11/12: version comparison, GitHub's answer read from a fixture (newer, same,
  404, error, garbage), unsafe download URLs left out, the `curl` command, the request naming only the app (against a
  local fake server); in kittest with a fake checker: the menu bar link, no check when turned off, Check for updates
  saying what it found, a failure silent at start-up but shown on Check now, the download command menu on Linux, and
  no checker meaning no check. Against GitHub (once, by hand): the About page said "up to date" for 0.2.1, and a check
  as 0.1.0 found 0.2.0 with the `.deb` and AppImage `curl` commands.
- **Running app** (Linux/WSLg screenshots): the start screen lists the session migrated from the 0.1 settings; the
  session dialog with every option. WSL has no Secret Service, so the dialog says the keychain is unavailable.
  R-UI-6: the start screen, session dialog (two columns, fits a 1000×640 window), settings dialog with the font
  preview, and PUB400's sign-on screen in IBM Plex Mono with the roomier bars; `make package` puts the OFL files in
  the .deb (66 ms to list this machine's installed monospace fonts).

### Phase 3: keyboard maps and macros (2026-11-12) — R-DS-5, R-KEY-2..4, R-ADD-1..2, R-NET-7

**R-DS-5** (2026-11-12): verified by the owner on their IBM i in the running app on Windows, macOS, Linux and WSL2
(signed on; a spooled file shown at 27×132 in a wide session).

How it was checked:
- **Against the manual**: IBM's 5494 Remote Control Unit Functions Reference (SC30-3533-02, chapter 12), cited next to
  the code. Start of Header matched (clears the format table; error row in byte 4) except that length 0 is now an
  error. Nulls as blanks and the negative zone matched; Read MDT Alternate now strips trailing nulls and sends only the
  address for an all-null field. The query reply sent a 2-byte model where the manual has 3, so every later byte was
  one early and the screen sizes landed in capability byte 0; fixed, with 256 input fields and the colour bit.
- **Automated** (`make test`): a test per fix in `tests/session.rs` (`*_r_ds_5`), each failing without its fix.
- **Against PUB400.COM**: the same sign-on screen at 24×80 and 27×132 before and after (it sends no Query before
  sign-on).

**R-KEY-3, R-KEY-4** (2026-11-12): recording and running macros; D14 for the files. **Automated**
(`make test`): the file format (round trip, every key name, comments, errors naming the line); recording (text, keys,
`wait` after Enter, the password left out, refused keys and their Reset left out, clicks left out); playing (waiting
for the host, asking for the password, Reset stopping it, the 30 s time-out, stopping at an error, starting on
the first screen); the macro folder (save, list, rename, delete, a bad file listed with its line); in kittest the save,
password and Macros dialogs (each keeping its place), Run macro in the session dialog, and a renamed session taking
its macros along. **Against PUB400.COM** (`probe --macro`): a macro pressing Help waited for the host and stopped at
its "Function key not allowed" with the error on screen. **In the running app** (WSL2, the owner): a recorded
sign-on macro as the session's Run macro, without auto sign-on, signs on and asks for the password (after a fix: it
used to wait for a screen past sign-on that it brings itself). **Verified** by the owner in the running app on Windows, macOS and Linux (2026-11-12).

**R-ADD-1, R-ADD-2** (2026-11-12): Settings ▸ Add-ons, with Macros as the first add-on (on by default);
D15. **Automated** (`make test`): each add-on starts with its default, only a change from it is kept, unknown ids
still load (`addons.rs`); in kittest the Add-ons page turning Macros off on Save, Cancel leaving it as it was, the
dialog keeping its place; while off no macro items in Tools, no Run macro in the session dialog, the macro folder not
read (and read again when turned on), a session's Run macro kept but not run on connect; turned off while connected,
a recording asks for its name and a running macro stops (against a local port that never answers). **Verified** by the owner in the running app on Windows, macOS and Linux (2026-11-12).

**R-KEY-2** (2026-11-12): keyboard maps. **Automated** (`make test`): the Default map gives R-KEY-1's keys
(Ctrl alone and Ctrl+R Reset, Ctrl+Plus with or without Shift); a map of one's own changes them, runs a macro, and
can drop Ctrl alone; keys read back as written, unknown bindings are kept and ignored; keys that type a character
without Ctrl, or with Ctrl+Alt (AltGr), are refused; a deleted map falls back to Default; renaming a macro moves its
keys. In kittest: the Default map has nothing to change; a copy gets a new key by clicking + and pressing it; a key
another function has moves only after "Use it for …"; renaming a map moves the sessions that use it; deleting it
gives them Default; the session dialog chooses the map; the Keyboard page keeps the dialog in place. **Seen** in a
`--screenshot` of the page (WSL2, dark). **Verified** by the owner in the running app on Windows, macOS and Linux (2026-11-12).

**R-NET-7** (2026-11-12): auto sign-on and saved passwords removed (D16; R-NET-4 and the planned R-NET-6 go).
**Automated** (`make test`): telnet's answer to RFC 4777's request with a host seed holds only the device name; a
session on a sign-on screen sends nothing until a key; a macro's `password` step always asks; the session dialog
asks for no user or password; `Password`'s `Debug` hides it. The RFC 4777 code and the `des`, `sha1` and
`getrandom` dependencies are gone. **Verified** by the owner in the running app on Windows, macOS and Linux (2026-11-12).

### Phase 3 (the rest) and Phase 4: a modern look (2026-11-15) — R-KEY-5..8, R-UI-13..17, R-NF-2

Verified by the owner in the running app on Windows, macOS, Linux and Linux under WSL (2026-11-15), including
R-NF-2's `.msi` (PATH and Start menu) and the macOS links.

- **R-NF-2** (2026-11-15): both commands after installing. Built; **automated** (`make test`): `--sql` first and a
  start by the name `parley-sql` hand over to Parley SQL, anything else is Parley. **Checked on Linux under WSL:**
  the `.deb` holds `/usr/bin/parley`, `/usr/bin/parley-sql` and both menu entries; the AppImage answers
  `--version`, `--sql --version` and, through a link named `parley-sql`, `--version` as Parley SQL.
- **R-KEY-5** (2026-11-12): editing a macro in Parley. **Automated** (`make test`): in kittest, Edit opens the
  file's text, a change is saved, text that does not read names its line and cannot be saved, Cancel leaves the file,
  New macro… saves a macro written by hand, the editor keeps its place; in parley-app the text is written and read
  back with its comments. **Seen** in a `--screenshot` of the editor (WSL2, dark).

- **R-KEY-6** (2026-11-12): Stop beside REC in the status bar. **Automated** (`make test`): in kittest, with a
  session connected to a local port, Stop is there only while recording, clicking it ends the recording and asks for
  the name, and the boxes beside it keep their place.

- **R-KEY-7** (2026-11-12): importing and exporting macro files through the system's file window (rfd, GTK 3 on
  Linux as Tortuga). **Automated** (`make test`): in parley-app, files are copied in as they are (comments,
  `æøå` in the name), a file that does not read is refused with its line, a name already taken is refused, export
  copies the file and a missing macro is an error; in kittest, Import… and Export… report what happened and the list
  shows an imported macro at once; Open folder is gone. `cargo deny` passes with the GTK crates. 

- **R-KEY-8** (2026-11-13): a key typed in a protected area is ignored, with no error to Reset (D18).
  **Automated** (`make test`): in parley-core, a character (also `æ`), Delete, Backspace, Erase EOF and Field Exit at a
  protected position leave the keyboard unlocked and the screen unchanged, and Tab then goes to the next input field.

- **R-UI-13** (2026-11-12): icons from the bundled Phosphor font (D17). **Automated** (`make test`): every
  icon in `icons::ALL` is in the icon font, which is the only font in its family and stays out of the text families
  (Inter has glyphs of its own at some of the same codepoints). **Seen** in `--screenshot`s on PUB400's sign-on screen
  (WSL2, light and dark).

- **R-UI-14** (2026-11-12): the interactive status bar. **Automated** (`make test`): the state, TLS, keyboard
  map, code page, size and cursor show, INS only in insert mode; a narrow bar drops size and code page but keeps TLS
  and the cursor; the clock times the host from an AID key to the unlocked keyboard; the lock is noticed in the frame Enter
  goes down, the hourglass has its own slot beside the check, which stays, and Keys does not move; the REC pill and Stop keep the
  state pill in place (R-KEY-6); choosing a map in the chip saves it to the session. **Seen** in `--screenshot`s (WSL2,
  light and dark: Ready, 125 ms, 24×80, CCSID 37, Default, 5, 25).

- **R-UI-15** (2026-11-12): one header with the session pill. **Automated** (`make test`): menus and Help are
  there before connecting; the pill shows host:port and its menu disconnects; Connect to another session while
  connected closes the open one first. **Seen** in `--screenshot`s (WSL2, light and dark).

- **R-UI-16** (2026-11-12, changed 2026-11-13): the 5250 keys in a Keys menu in the status bar. **Automated**
  (`make test`): Attention and F15 are sent from the menu, the menu lists System Request and F24 while connected, and
  there is no Keys button before connecting. **Seen** in a `--screenshot` (WSL2, light): Ready, Keys, then the chips.


- **R-UI-17** (2026-11-12): the keyboard shortcut sheet. **Automated** (`make test`): the Default map's keys
  under Settings' headings, Ctrl+/ opens it from the screen and it stays open the next frame, Esc closes it (not on
  the frame it opens), both close buttons, Change keys… opens Settings ▸ Keyboard, the filter, and the dialog keeps
  its place.

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

### Phase 21: protocol gaps (2026-11-15) — R-DS-6, R-DS-7

- **R-DS-6, R-DS-7** (2026-11-15): Save/Restore Partial Screen and Read MDT Immediate Alternate, built against
  SC30-3533-04 §15.20, §15.22, §15.24 and RFC 1205 §5.3. **Automated** (`make test`, `tests/session.rs`): the save
  reply (opcode Save Screen, ESC X'13', a length of 6, Parley's id) comes after the rest of the record; Restore
  Partial puts back the screen, its fields and unlocked keyboard and the record goes on (a Read Immediate after it is
  answered); foreign data, a length past the record and a truncated length are reported; X'83' answers at once with
  cursor, AID X'00' and only modified fields after SBA, trailing nulls left out; the query reply sets byte 0 bit 7.
  **Where the sources differ:** the manual says X'83' is "the same as READ IMMEDIATE" (no cursor or AID); RFC 1205
  says AID X'00' after the cursor, which Parley follows. **Not checked against a real IBM i** (the test profile could
  not sign on); marked done by the owner (2026-11-15). In a later trace of DKSOFT00 (help windows, F4 list windows,
  F12) the host sent neither command: it rewrites the whole screen.

- **R-DS-8** (2026-11-15): function keys a Start of Header switches off send no field data (SC30-3533-04 §15.6.9).
  **Automated** (`make test`): F3 and F24 switched off send cursor and AID only and still lock the keyboard, F4 sends
  its data; a 6-byte Start of Header leaves every key sending data; Clear Format Table switches them back on and
  Save/Restore Screen keeps them; the bit order (byte 5 = F24–F17, byte 7 = F8–F1). **Against the owner's IBM i**
  (DKSOFT00, the owner signed on in Parley with `PARLEY_TRACE`, inbound only): the DSPLIB prompt's Start of Header
  switches off F5 (`00 00 10`); replaying the recorded screen with a changed field, F5 sends 3 bytes (cursor, AID)
  and F4 and F12 send the field. Marked done by the owner (2026-11-15).

- **R-UI-24, R-UI-25** (2026-11-15): blinking text and the alarm (IDEA-6, D37). **Automated** (`make test`): the blink phase (shown the first half of each second, hidden the second); with Blinking text off
  it is steady; the flash lasts a moment; a local host sending Write To Display with the CC2 alarm bit beeps and
  flashes once with Sound and flash, flashes without a beep with Flash, does neither with Off; the Terminal page
  saves the alarm and the switch. The beep builds with ALSA (`libasound2-dev`) and `make verify` passes; the beep, the flash and blinking text were not
  seen or heard in the running app. Marked done by the owner (2026-11-15).

- **R-KEY-9** (2026-11-15): operator error codes before the message (IDEA-2). **Automated** (`make test`): a letter in a
  digits-only field is 0010 with its message; 0012 for no room to insert; no code for the code page check and the
  host's message; the status bar shows "X 0012 No room to insert: the field is full.". The codes come from ASNA's
  5250 Terminal Error Codes (which says they are the standard 5250 ones); only 0012 is also in IBM's SC30-3533-04.
  Not seen in the running app; marked done by the owner (2026-11-15).

- **R-KEY-10** (2026-11-15): mandatory enter, mandatory fill and self-check fields (IDEA-3). **Automated** (`make
  test`): modulus 10 and 11 against IBM's DDS example (13739 → 1) and the Luhn example, remainder 1 has no check
  digit, letters by their low bits, an empty field passes; on a built screen: Enter with an empty mandatory enter
  field is 0007, puts the cursor there, sends nothing and waits for Reset, and is not enforced when nothing was
  typed; Field Exit past it is 0021; a half-filled mandatory fill field is 0014 with Tab and 0017 with Field Exit,
  and Field Exit from its first position empties it; a wrong check digit is 0015 with the cursor back at the field,
  a right one moves on. Not tried on a real screen; marked done by the owner (2026-11-15).

- **R-KEY-11** (2026-11-15): type-ahead (IDEA-14). **Automated** (`make test`): off, keys while the host works are
  dropped; on, they are typed when the next screen unlocks the keyboard (also `æ`); an Enter typed ahead goes and
  leaves the rest for the screen after; Reset drops them; an error while replaying drops the rest; at most 256 are
  kept and turning it off drops them; the status bar says "2 keys typed ahead"; the Terminal page saves the switch.
  Not tried in the running app; marked done by the owner (2026-11-15).

- **R-DS-9** (2026-11-15): extended attributes (IDEA-5, first part). **Automated** (`make test`): an extended colour
  holds until the next, a null continues it and X'80' gives the screen's back; primary attributes replace the
  screen's highlighting (underline, reverse, high intensity as white, also on `æøå`); X'81' hides text; Clear Unit
  drops them; an address past the end is ignored; on a whole screen WEA takes no position, colours red then green,
  ideographic attributes are ignored and an unknown type is a bad record; the query reply's byte 2 is X'05'.
  **Against DKSOFT00** (the owner ran a read-only tour macro with `PARLEY_TRACE`: help, a prompt and
  its list, WRKSPLF, WRKACTJOB, DSPLIB, GO CMDLIB): the host sent no extended attributes on those screens. Marked
  done by the owner (2026-11-15).
