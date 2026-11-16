# Phase 21 — Protocol gaps

> Finished phase of Parley 5250. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

The commands and fields Parley still turns away (R-P-2), from IDEA-7, then IDEA-4, 6, 2, 3, 5 and 14 in that order,
each checked against IBM's 5494 Remote Control Unit Functions Reference (SC30-3533-04, chapter 15) and RFC 1205.
First Save/Restore Partial Screen (R-DS-6) and Read MDT Immediate Alternate (R-DS-7), which a host now gets an
error for.

**Status:** Started (2026-11-15): R-DS-6, R-DS-7 (IDEA-7), R-DS-8 (IDEA-4) done; R-UI-24, R-UI-25 (IDEA-6) done; R-KEY-9 (IDEA-2) done; R-KEY-10 (IDEA-3) done; R-KEY-11 (IDEA-14) done; IDEA-5: R-DS-9 extended attributes done; the GUI is Phase 22.

## Requirements

| ID | Requirement |
|---|---|
| R-DS-6 | **Save and Restore Partial Screen** (2026-11-15, from IDEA-7; SC30-3533-04 §15.22, §15.24). Save Partial Screen (ESC X'03' and five bytes: flag, top row, left column, depth, width) is answered as Save Screen is (D7): Parley keeps the whole screen, as the 5494 does ("saves the entire display screen in all cases, ignoring these four parameters"), and replies with opcode Save Screen and `ESC X'13'`, a 2-byte length counting itself, and its id; the reply goes after the rest of the record has been carried out. Restore Partial Screen (ESC X'13', a 2-byte length counting itself, the data) puts back what was saved, as Restore Screen does, and the commands after it in the record are carried out. Restore data Parley did not make, or a length that runs past the record, is reported as a bad record. |
| R-DS-7 | **Read MDT Immediate Alternate** (2026-11-15, from IDEA-7; SC30-3533-04 §15.20, RFC 1205 §5.3). ESC X'83', with no control characters, is answered at once, without waiting for an AID key or unlocking the keyboard: the cursor's row and column, AID X'00' (RFC 1205), then each modified field after a Set Buffer Address, its leading and embedded nulls kept and trailing nulls left out, as Read MDT Fields Alternate. The query reply says Parley supports it (device capability byte 0, bit 7). |
| R-DS-8 | **Function keys that send no field data** (2026-11-15, from IDEA-4; SC30-3533-04 §15.6.9 "Start of Header Order"). When a Start of Header has all seven bytes, its bytes 5–7 are the command key switches: a set bit (byte 5 bits 0–7 for F24–F17, byte 6 for F16–F9, byte 7 for F8–F1, bit 0 the high-order bit) makes that function key send only the cursor's row and column and its AID, without field data, as tn5250 and tn5250j read the manual's "prevents input data … from being returned". A shorter Start of Header, Clear Format Table and Clear Unit return every key to sending data; Save/Restore Screen keep the switches with the screen. The keys still lock the keyboard and wait for the host as before. |
| R-DS-9 | **Extended attributes** (2026-11-15, from IDEA-5; the owner chose extended attributes first, then the GUI behind a switch; SC30-3533-04 §15.6.11). Write Extended Attribute (WEA, X'12') writes an extended primary attribute (type X'01': X'80' normal, X'81'–X'9F' column separators, blink, underline, high intensity, reverse image) or an extended foreground colour (type X'03': X'82'–X'8F' blue, green, turquoise, red, pink, yellow, white, two shades each shown alike; X'81' the background colour) at the current address, without moving it. An extended attribute holds from its position until the next one of its type, takes precedence over the screen attributes, and X'80' gives the screen attributes back; Clear Unit clears them and Save/Restore Screen keep them. Extended ideographic attributes (type X'05') are ignored, as Parley does not claim them; another type is a bad record. The query reply claims extended primary attributes in DP mode (device capability byte 2 bit 5) and 7 extended colours with the others mapped onto them (bits 6–7 = B'01'). Roll does not move extended attributes (IBM says to use them for output fields only). |
| R-KEY-9 | **Operator error codes** (2026-11-15, from IDEA-2). The status bar's error pill shows the 5250 operator error code before the message, as IBM's displays do: "X 0008 Only letters, …". The codes are a standard 5250 terminal's, as ASNA's 5250 Terminal Error Codes list them (it says they are the standard codes; 0012 is also in IBM's SC30-3533-04): 0004 a field that takes no keyboard input, 0008 alphabetic only, 0009 numeric only, 0010 digits 0–9 only, 0011 the sign position, 0012 no room to insert, 0016 Field- not allowed. An error with no 5250 code (a character not in the session's code page, which is Parley's own check) and the host's own messages show without a code. |
| R-KEY-10 | **Mandatory enter, mandatory fill and self-check fields** (2026-11-15, from IDEA-3; SC30-3533-04 §15.6.12.1, §15.6.12.2; IBM's DDS for display files, CHECK keyword, for the algorithms; the owner chose lock until Reset). Parley checks these fields as a 5250 does; a failed check is an operator error with its standard code (R-KEY-9), nothing goes to the host, and keys wait for Reset. **Mandatory enter** (FFW bit 12): Enter while such a field has nothing typed is 0007 and puts the cursor in it, but only when something on the screen was typed (DDS CHECK(ME)); Field Exit, Field+ or Field- past it with nothing typed is 0021. **Mandatory fill** (FFW bits 13–15 = 111): leaving a field that has some but not all positions typed is 0014 with Tab or Back Tab and 0017 with Field Exit, Field+ or Field-; Field Exit from the first position empties the field instead. **Self-check** (FCW X'B1A0' modulus 10, X'B140' modulus 11; the check digit is the last position): checked when the field is left by typing its last position, Tab, Back Tab or Field Exit; a wrong check digit is 0015 and puts the cursor at the field's start; a modulus 11 number whose remainder is 1 has no valid check digit; characters count by their low 4 bits (A = 1), nulls and blanks as 0, so an empty field passes. Leaving by arrow keys or a click is not checked. |
| R-KEY-11 | **Type-ahead** (2026-11-15, from IDEA-14). Settings ▸ Terminal ▸ "Type-ahead", off by default. When on, keys pressed while the keyboard waits for the host (X SYSTEM) are kept, at most 256, and carried out in order once the host unlocks it; a key that waits for the host again (Enter, an F key) leaves the rest for the next unlock, and an operator error drops the rest so the operator sees it. Reset drops the keys kept; Attention, System Request and Reset act at once and are never kept, nor a click. The status bar shows "3 keys typed ahead" while keys wait. Keys typed ahead are recorded in a macro when they are carried out. |
| R-UI-24 | **Blinking text** (2026-11-15, from IDEA-6; the owner chose blinking with an off switch). Positions with the 5250 blink attribute (Style `blink`, SC30-3533-04 screen attributes) blink: their characters show for half a second and hide for half a second, while the rest of the screen stays still; Parley draws again only while blinking text is on the screen. Settings ▸ Terminal ▸ "Blinking text" (on by default) turns it off: the text then shows steady in reverse image, so it still stands out. The cursor does not blink. |
| R-UI-25 | **The alarm** (2026-11-15, from IDEA-6; the owner chose flash and sound with a setting; D37). When the host sounds the alarm (CC2 bit 5 of Write To Display or a read command), the border around the 5250 screen flashes for a moment and a short beep (880 Hz, 150 ms) plays on the computer's default sound output. Settings ▸ Terminal ▸ "Alarm": Sound and flash (default), Flash only, Off. Without a sound output the beep is left out quietly; the flash still shows. |
| R-UI-26 | **Frames for the host's character windows** (2026-11-15, asked by the owner after Phase 22 paused). IBM i draws its pop-up windows on a plain 5250 with characters, as the 5494 does (SC30-3533-04 §15.6.13.1, figures 22–24): a top row of `.` (the title in it), `:` down both sides, and a bottom row `:` `.`…`.` `:`. Parley finds such windows on the screen and draws their border as a frame line in the border's colour, as for GUI windows (R-DS-10), with the title and footer in the line's gaps. A window is found only when both side columns hold `:` on every row between a top and a bottom row that are all `.` but for one title or footer, so dotted leaders ("User . . . :") are not taken for windows. Settings ▸ Terminal ▸ "Window frames", on by default, turns it off. |

## Verification

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

- **R-UI-26** (2026-11-15): frames for the windows IBM i draws with characters. **Automated** (`make test`): a window
  of `.` and `:` with a title (`Hjælp`) and `æøå` inside is found, dotted leaders ("User . . . . :") and a bottom
  row without a top are not; a GUI window's border is found as the same frame and drawn once; the frame leaves the
  title alone; Window frames is on by default and the Terminal page turns it off. **Checked in the running app** by the
  owner on DKSOFT00 (help and F4 list windows), Linux under WSL (2026-11-15).
