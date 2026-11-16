# Phase 3 — Keyboard maps and macros

> Finished phase of Parley 5250. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

First a correctness step (R-P-2): the points of the data stream built from memory checked against IBM's manual
(R-DS-5, from IDEA-1). Then keyboard maps chosen per session (R-KEY-2, from IDEA-9) and macros, recorded (R-KEY-3) and run (R-KEY-4), from IDEA-11. Settings ▸ Add-ons
(R-ADD-1, D15), with macros as the first add-on (R-ADD-2). Auto sign-on and saved passwords removed (R-NET-7, D16);
R-NET-6 (why a bypass sign-on failed) is dropped with it.

**Status:** Done (2026-11-12): verified on Windows, macOS and Linux (FEATURES.md): R-DS-5, keyboard maps (R-KEY-2), macros (R-KEY-3, R-KEY-4), add-ons (R-ADD-1, R-ADD-2), auto sign-on removed (R-NET-7, D16). The macro editor (R-KEY-5), Stop in the status bar (R-KEY-6) and import and export of macros (R-KEY-7) verified 2026-11-15.

## Requirements

| ID | Requirement |
|---|---|
| R-NET-7 | **No auto sign-on, no saved passwords** (D16; replaces R-NET-4). Parley never signs on by itself: the operator signs on on the host's sign-on screen, or a macro does, asking for the password at its `password` step (R-KEY-3, R-KEY-4). The session dialog has no user, password, auto sign-on or password-encryption fields, and Parley never sends a user or password substitute in NEW-ENVIRON (RFC 4777), even when the host offers a seed. Passwords Parley 0.2 saved in the operating system's keychain are deleted at the first start of this version, for every saved session; one that cannot be deleted is reported (`parley:` on the terminal) and tried again at the next start. Old settings with sign-on fields still load (the fields are ignored). R-NET-6 (why a bypass sign-on failed) is dropped. Tested: telnet's answer to RFC 4777's request with a seed holds only the device name; a session on a sign-on screen sends nothing until a key; the session dialog asks for no user or password. |
| R-DS-5 | **The data stream checked against the manual** (2026-11-12, from IDEA-1). Each point Phase 1 built from memory is checked against IBM's 5250 Functions Reference (SC30-3533) and cited next to the code: Start of Header clears the format table; Read MDT Fields sends nulls as blanks; a signed numeric field is sent without its sign position, a minus making the last digit's zone X'D'; the bytes of the reply to the 5250 Query. A point that differs is fixed, with a test that fails without the fix; a point the manual leaves open is noted in the code. |
| R-KEY-2 | **Keyboard maps** (from IDEA-9). Settings ▸ Keyboard lists every 5250 key and Parley action (Enter, Field Exit, Field+/-, Reset, F1–F24, Attention, System Request, Roll, Erase EOF, Insert, Copy, Paste, …) with the PC keys bound to it; select one and press a key to add a binding, remove a binding, "Restore defaults". The map R-KEY-1 defines is the built-in **Default** map, which cannot be changed; the user can copy it under a new name, edit, rename and delete their own maps. A key bound twice in one map is flagged and the new binding wins only after the user confirms. A key egui cannot tell apart (e.g. left and right Ctrl, R-KEY-1) cannot be bound. The session dialog has "Keyboard map" (Default unless chosen); a session whose map was deleted falls back to Default. Maps are saved with the settings. *As built (2026-11-12): a key matches only with the same Ctrl, Alt and Shift (Shift is ignored on keys that type a character, whose shifted character egui already names); a key that types a character can be bound only with Ctrl and without Alt (AltGr); Ctrl+C, Ctrl+V and Ctrl+X are the system's clipboard keys, always copy and paste and cannot be bound (egui never reports them as keys), but Copy and Paste can get more keys; while the Macros add-on is on, the map also holds Record macro (start and stop) and a Run macro per macro; renaming a map or a macro moves its keys and sessions along.* |
| R-KEY-3 | **Record a macro** (from IDEA-11). Tools ▸ Record macro starts recording on the connected session; the status bar shows "REC" until Tools ▸ Stop recording, then Parley asks for a name and whether the macro is only for this session (else for all). A macro is the 5250 keys and text typed (not mouse clicks), with a `wait` step after each AID key, Attention and System Request. Keys the session refuses are not recorded, nor the Reset that clears their error. Text typed into a non-display field is never recorded: it becomes one `password` step, which asks for the password each time the macro is played (Parley keeps none, R-NET-7). Recording stops when the connection ends or on Disconnect, and the name is asked then. Macros are saved as text files (D14), one per macro, so users can edit and share them; renaming a session moves its macros along. *A key in the keyboard map starts and stops recording (R-KEY-2).* |
| R-KEY-4 | **Run a macro** (from IDEA-11). Tools ▸ Run macro ▸ lists the macros for all sessions and for this one; the session dialog's "Run macro" (disabled since R-UI-5) runs one at connect, from the first unlocked screen, so the macro can sign on itself. The status bar shows "⏵ Macro" while one runs. A running macro waits at each `wait` step until the host has answered and unlocked the keyboard (giving up after 30 s with a message), stops at an operator error (the error stays on screen), and stops when the user presses Reset or Esc (Attention), chooses Tools ▸ Stop macro, or the connection ends; other keys are ignored while it runs. Tools ▸ Macros… lists the macros to rename or delete, shows their folder (Open folder, Copy path, Read again for files edited by hand). A macro file that cannot be read is listed with the reason (and the line) and cannot be run. *A key in the keyboard map can run a given macro (R-KEY-2).* *(Open folder removed 2026-11-12 by R-KEY-7: Edit, Import and Export replace it.)* |
| R-KEY-5 | **Edit a macro in Parley** (2026-11-12, asked by the owner). Tools ▸ Macros… has "Edit" on each macro and "New macro…": a dialog with the macro's file as text (monospace, comments and blank lines kept as typed) and, for a new one, its name. The text is checked as it is typed with the reader that runs macros (R-KEY-4); a line that does not read is named ("line 7: unknown key teleport") and Save stays off until it reads; a short line under it gives the syntax. Save writes the file and the list reads it again; Cancel and 🗙 leave the file as it was. A new macro needs a name no macro has. |
| R-KEY-6 | **Stop recording from the status bar** (2026-11-12, asked by the owner). While a macro is being recorded, the status bar's macro box shows "REC" and a small Stop button beside it; Stop does what Tools ▸ Stop recording does (asks for the name). The macro box keeps one width, so the boxes beside it don't move when a recording starts or stops, and the bar keeps its height. *(Changes R-KEY-3: Stop recording was only in the Tools menu.)* |
| R-KEY-7 | **Import and export macros** (2026-11-12, asked by the owner). Tools ▸ Macros… has "Import…" beside Copy path and "Export…" on each macro, both opening the system's file window (GTK 3 on Linux, also under WSL, as Tortuga's file windows; the xdg portal fails silently where none runs). Import takes one or more `.macro` files and copies each in as it is, named after the file; a file that does not read as a macro (its line named), whose name is not a valid macro name, or whose name a macro already has is not taken, and the dialog says which were imported and why the others were not. Export saves a copy of the file where the operator chooses (`NAME.macro` offered) and says where. Cancelling either changes nothing. GTK 3 is a new build and run dependency on Linux (CI, release, the .deb, `scripts/setup-linux.sh`). *R-KEY-4's "Open folder" is removed (the owner, 2026-11-12): Edit, New macro…, Import… and Export… do what it was for; the dialog still shows the folder with Copy path, and Read again.* |
| R-ADD-1 | **Settings ▸ Add-ons** (D15; as Tortuga's R-ADD-1). A page "Add-ons" of the settings dialog, under App before About, lists every add-on with its name, one line on what it does and a switch. **Each add-on has its own default** (Macros: on). The settings keep only the ids that differ from their default (turned on, or turned off), so an add-on a later version brings starts with its default and an id this version doesn't know still loads. A switch takes effect when the dialog is saved; Cancel and the close button leave the add-ons as they were; Restore defaults on the page puts every add-on back to its default. |
| R-ADD-2 | **Macros are an add-on, on by default** (D15). "Macros" in Settings ▸ Add-ons ("Record keystrokes and play them back, also when a session connects."). While it is off, nothing of it is in the app: the Tools menu has no Record macro, Run macro ▸, Stop macro or Macros…, the status bar has no macro box, the session dialog has no Run macro, a session's Run macro is not run when it connects (the choice is kept for when the add-on is on again), and the macro folder is not read. Turning it off while connected stops a running macro, and ends a recording by asking for its name as Stop recording does, so nothing is lost; the Macros… dialog closes. Macro files stay where they are. *(Changes R-KEY-3, R-KEY-4: their menu items and the session dialog's Run macro exist only while the add-on is on.)* |

## Verification

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

*From the section "Phase 3 (the rest) and Phase 4: a modern look (2026-11-15) — R-KEY-5..8, R-UI-13..17, R-NF-2"; its heading and notes are in [Phase 4](phase-04-a-modern-look.md).*

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
