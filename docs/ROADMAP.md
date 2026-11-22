# Parley roadmap

> The plan, and the spec for everything that is not done yet. Finished phases, with the text of their
> requirements and how each was verified, are in [phases/](phases/); [REQUIREMENTS.md](REQUIREMENTS.md) is the index of finished IDs.
> Look things up with `grep -n` (an ID such as `"R-NET-1 "`, `"| D3 "` or a heading) rather than reading the whole file.

**Status of this document:** first written 2026-11-12; cleaned up for the final push on 2026-11-21 (D62). Open:
the owner's checks of Phase 22 and Phase 42, then the ideas still open ([IDEAS.md](IDEAS.md)). What is decided not
to be built now is in [PARKED.md](PARKED.md).

## 1. Vision

Parley 5250 (called Parley until 2026-11-16, D40) is a fast, modern TN5250 terminal emulator for IBM i, written in Rust. It is a desktop app for Windows, Linux and
macOS from one code base, starts instantly, and feels at home for people who use green screens all day, with a
modern take on what surrounds the screen: sessions, keyboard mapping, copy and paste, and scripting.

**Parley SQL**, the SQL window for IBM i (Parley's SQL add-on and the program `parley-sql`), has a plan of its own:
[sql/ROADMAP.md](sql/ROADMAP.md). The two share this repository, the principles below and one sequence of IDs
(see §8).

**Parley SSH**, an SSH client in the same look that does what PuTTY does and then more (D41), has its plan in
[ssh/ROADMAP.md](ssh/ROADMAP.md).

## 2. Product principles

| ID | Principle |
|---|---|
| R-P-1 | **Keyboard first.** Every 5250 function key and action is reachable from the keyboard, with mappings people know from existing emulators. |
| R-P-2 | **Correct before clever.** The 5250 data stream is implemented to IBM's specification and tested against recorded host traffic. |
| R-P-3 | **Secure by default.** TLS (port 992) is the default for new sessions; Parley never stores a password and never signs on by itself (R-NET-7, D16). |
| R-P-4 | **One engine.** All protocol logic lives in `parley-core`, with no UI or platform code, so it can be tested on its own. *Exception (D20, 2026-11-13): the SQL add-on runs its database work in a Java helper with IBM's JTOpen; Parley's side of it, the helper's protocol, is in `parley-core`.* *Changed by D36 (2026-11-15): Parley SQL's engine is `parley-sql-core`, beside `parley-core`, held to the same rule.* |
| R-P-5 | **Two programs, apart in the code** (2026-11-15, asked by the owner; D36). Parley (the terminal) and Parley SQL are separate crates: `parley-core` and `parley-ui-egui` are Parley's and never depend on `parley-sql-core` or `parley-sql-ui`; Parley SQL builds on Parley's engine only to connect and for code pages, and on its UI only for the shared look (theme, dialogs, icons, the settings frame). Each program keeps its own settings type in its own storage. `cargo deny` (`make verify`) fails a build that breaks this. |

## 3. Status

| Phase | State |
|---|---|
| [Phase 0: development environment](phases/phase-00-development-environment.md) | Done (2026-11-12). |
| [Phase 1: connect and show a screen](phases/phase-01-connect-and-show-a-screen.md) | Done (2026-11-12). |
| [Phase 2: saved sessions and auto sign-on](phases/phase-02-saved-sessions-and-auto-sign-on.md) | Done (2026-11-12); auto sign-on removed again in Phase 3 (D16). |
| [Phase 3: keyboard maps and macros](phases/phase-03-keyboard-maps-and-macros.md) | Done (2026-11-12; R-KEY-5..7 on 2026-11-15). |
| [Phase 4: a modern look](phases/phase-04-a-modern-look.md) | Done (2026-11-15). |
| Phases 5–19, 23, 25: SQL | [sql/ROADMAP.md](sql/ROADMAP.md). |
| [Phase 20: everyday screen work](phases/phase-20-everyday-screen-work.md) | Done (2026-11-15). |
| [Phase 21: protocol gaps](phases/phase-21-protocol-gaps.md) | Done (2026-11-16). |
| [Phase 24: more settings](phases/phase-24-more-settings.md) | Done (2026-11-16). |
| [Phase 40: more sessions at once](phases/phase-40-more-sessions-at-once.md) | Done (2026-11-17). |
| Phases 26–39, 41: SSH | [ssh/ROADMAP.md](ssh/ROADMAP.md). |
| Phase 22: GUI elements (beta) | Built behind a session switch (D38, D57): R-DS-10..12; the owner's checks in the running app are open ([FEATURES.md](FEATURES.md)), then it stays a beta switch (D60). |
| Phase 42: screen history | Built 2026-11-21 (D60, D61): R-UI-37..39; the owner's checks in the running app are open ([FEATURES.md](FEATURES.md)). |
| Phase 43: a start screen for many sessions | Planned 2026-11-22 (D64): R-UI-40..42, and R-SQL-105 for Parley SQL. |

## 4. Phases

Finished phases are in [phases/](phases/), one file each.

### Phase 22 — GUI elements

From IDEA-5 (the owner, 2026-11-15): IBM i's GUI-like 5250 (enhanced user interface level 1) draws pop-up windows,
menus, radio buttons, check boxes, push buttons and scroll bars. A display that claims it gets all of them, so
Parley claims it only behind a session switch, off by default, while it is built in steps (D38): windows (R-DS-10),
selection fields and the mouse (R-DS-11), scroll bars, Write Data and mouse buttons (R-DS-12). Each step is checked on DKSOFT00 with a trace and a
macro the owner runs.

**History.** Paused on 2026-11-15, when the owner's IBM i never sent a GUI structured field whatever the query reply
claimed (see R-DS-10's entry in [FEATURES.md](FEATURES.md)); resumed on 2026-11-20 (D57), with R-DS-11 and R-DS-12
built from the manual and tested with records built by hand from it. Once R-DS-10..12 are checked in the running app
the phase is done and the GUI stays a beta switch; nothing more unless a user asks (D60, [PARKED.md](PARKED.md)).

### Phase 42 — screen history

From IDEA-21 (the owner, 2026-11-21, D60; planned with D61): page back through the screens a session has shown,
read-only, to find, copy or save what was on one, for instance a job's message gone after the next Enter, or a list
the program has since moved past. R-UI-37..39.

Built in this order, each step tested and seen in the running app before the next:
1. **Screens kept** (R-UI-37): `parley-core` keeps a copy of the screen when the keyboard unlocks, with non-display
   fields blanked, in a history of its own (`session/history`), tested with records as the rest of the engine.
2. **The history view** (R-UI-38): the screen widget draws a kept screen read-only in place of the live one, under the
   history's bar, with the keys, find, selection and copy; the drawing is split from the live session's input so both
   use it.
3. **Saving a screen and the setting** (R-UI-39): text and HTML, the launcher's file window, and how many are kept.

Checked by the owner against PUB400's sign-on screen (no account: the password field shows the history's blanking)
and their own system.

### Phase 43 — a start screen for many sessions

Asked by the owner (2026-11-22, D64): Parley 5250 and Parley SQL get the start screen Parley SSH got in its Phase 41
(R-SSH-63..65): favourites marked with a star and listed first, the recent connections, a search at the top that
forgives a typo and is driven by the keyboard, "All sessions" folding, and quick connect beside the list. Parley 5250
keeps its session groups on it, in a section of their own. R-UI-40..42; Parley SQL's saved connections, R-SQL-105.

Built in this order, each step tested before the next:
1. **The shared parts** (D64): Parley SSH's list rows, folding headings and the search's score move into
   `parley-ui-egui`, which Parley SSH then uses; no change in how Parley SSH looks or behaves.
2. **Parley 5250's start screen** (R-UI-40..42) on them, in place of `connect`'s cards.
3. **Parley SQL's** (R-SQL-105) on the same screen.

Checked by the owner in the running apps with their own sessions and connections.

### Later (unordered, to be planned)

The ideas still open: Insert mode and trailing blanks (IDEA-8), printer sessions (IDEA-18), a command bar (IDEA-27);
see [IDEAS.md](IDEAS.md). Decided not now: [PARKED.md](PARKED.md).

## 5. Requirements

Open requirements, in the order they are planned. The done ones are in [REQUIREMENTS.md](REQUIREMENTS.md); new
ones come from [IDEAS.md](IDEAS.md). Areas: `NET` connection and telnet, `DS` the 5250 data stream,
`EBC` character sets, `KEY` keyboard, `UI` user interface, `ADD` add-ons, `NF` non-functional. The `SQL` area, and
the SQL window's add-on switches, probes and assistant, are in [sql/ROADMAP.md](sql/ROADMAP.md).

| ID | Requirement | Phase |
|---|---|---|
| R-DS-10 | **GUI windows, behind a switch** (2026-11-15, from IDEA-5; D38, D57; SC30-3533-04 §15.6.13.1–4). A session setting "GUI elements (beta)", off by default; when on, the query reply claims what tn5250j's enhanced mode claims: GUI-like characters, enhanced user interface level 1 and Write Error Code to Window (device capability byte 4 bits 5–7) and level 3 (byte 5 bit 4), in a 64-byte reply (with level 1 alone, DKSOFT00 sent no windows); the pointer device (byte 4 bit 4) waits for R-DS-11. Write to Display Structured Field (WDSF) then carries out Create Window (X'51': a window of the given depth and width at the current address, with its border and optional title as the border and title minor structures say, drawn as a framed panel over the screen; the cursor kept inside when its flag says so; up to 128 windows, one of the same place and size replacing the other), Unrestricted Window Cursor Movement (X'52'), Remove GUI Window (X'59') and Remove All GUI Constructs (X'5F'); Clear Unit, Clear Unit Alternate and Restore Screen remove them too. Text written inside a window shows in it. Off, nothing changes. | 22 |
| R-DS-11 | **GUI selection fields and the mouse** (2026-11-15, from IDEA-5; D38; *details 2026-11-20, D57: from IBM i's own bytes in IBM's TRC5250 traces and the names Wireshark and other emulators took from the manual, which was not at hand*; SC30-3533-04 §15.6.13.5–6). With the switch on, the query reply also claims a pointer device (byte 4 bit 4). Define Selection Field (X'50') defines an input field at the current address: a menu bar, radio buttons (single choice), check boxes (multiple choice), single and multiple choice lists and pull-downs, and push buttons, with their choices' text, state (bits B'10' selected and B'11' not available, as IBM i's menu bar bytes show), mnemonic and AID, and the colours of the presentation minor structure; only the minor structures meant for a GUI display count, and a field meant only for a display without a mouse is skipped. Choices are written into the screen, `columns` to a row (a menu bar in one row, a new one where a choice says), each as its state's attribute, an indicator (`/` selected, `_` not) or a push button's blanks, the text and a normal attribute; Parley draws radio buttons, check boxes, push-button boxes and mnemonic underlines over them. On a choice the cursor keys go to the next choice (or send the choice's Roll AID when it asks), Space or `/` selects (a check box toggles), a mnemonic selects its choice, Enter on a menu bar or push button selects it, and a click selects; a choice's own AID, or Enter for an auto-enter field, then goes to the host. The read commands send the field with the others in address order: one choice as its number plus X'1F' in two bytes (X'0000' for none), several as X'F1' or X'00' a choice. Selected choices go back when the keyboard unlocks if the field asks. Remove GUI Selection Field (X'58') blanks it; clearing the format table, Remove All GUI Constructs and removing the window it is in forget it. | 22 |
| R-DS-12 | **GUI scroll bars, Write Data and mouse buttons** (2026-11-15, from IDEA-5; D38; *details 2026-11-20, D57, as R-DS-11*; SC30-3533-04 §15.6.13.7–10). With the switch on, the query reply also claims enhanced user interface level 2 (byte 5 bit 0). Define Scroll Bar Field (X'53'): a vertical or horizontal bar of the given size at the current address, with the rows to scroll through and the slider's position, drawn with arrows; a click on an arrow moves it one row, beside the slider a page, and dragging the slider anywhere; then Roll Down (back) or Roll Up (on) goes to the host, and the read commands send the new position in four bytes at the bar's address (what IBM i expects back was in no source at hand). Remove GUI Scroll Bar Field (X'5B'). Write Data (X'54') puts its data into the entry field at the current address. Programmable Mouse Buttons (X'55'): each definition's event (left, right or middle pressed, released or double-clicked, with Shift or not), or two events, sends its AID, moving the cursor to the mouse first if it says; the click then does nothing else. | 22 |
| R-UI-37 | **Screens kept** (2026-11-21, IDEA-21, D60, D61). Each time the host has written and unlocks the keyboard, the session keeps a copy of the screen: its text, attributes, extended attributes and GUI windows, the cursor's place and when it came; the same screen as the last one kept is not kept again. Non-display fields and other non-display text are blanked in the copy, so a password never reaches the history. The newest screens are kept, as many as the setting says (R-UI-39, 50 by default), in memory only: the history lasts while the session's tab is open, across a reconnect, and is gone when the tab closes. | 42 |
| R-UI-38 | **The history view** (2026-11-21, D60, D61). Ctrl+Page Up (remappable, in the shortcut sheet and the Edit menu as Screen history, beside Find) shows the screen before the live one in its place, read-only, under a bar: "Screen 12 of 50 · 10:42:13", ‹ and ›, and Back to live. Ctrl+Page Up goes further back and Ctrl+Page Down forward; past the newest, and with Esc or Back to live, the live screen returns with the cursor where it was. While the history is open no key reaches the host and the status bar says History; the host's new screens are kept without moving the view, and the bar says how many came. Find (Ctrl+F, R-UI-20's bar) searches the screen shown, a selection and Ctrl+C or Ctrl+T copy from it, and hotspots are not offered. With no screen kept yet, Ctrl+Page Up says so in the status bar. | 42 |
| R-UI-39 | **Saving a screen, and how many are kept** (2026-11-21, D60, D61). The history's bar has Save…: the screen shown, as text (its rows, trailing blanks cut, UTF-8) or as HTML (one file of its own: the screen in the session's colour scheme and font family, each attribute's colour, underline, reverse and column separators as drawn, the session's name and when the screen came in its title), through the launcher's file window. Settings ▸ Terminal ▸ Screens kept: 10 to 500, 50 by default; lowering it drops the oldest at once. | 42 |
| R-UI-40 | **Favourite sessions** (2026-11-22, asked by the owner; D64; as R-SSH-63). A star on each saved session's row on the start screen marks it as a favourite or not, kept with the session; the favourites are listed first, under "Favourites", and also under "All sessions", which lists every saved session. | 43 |
| R-UI-41 | **Recent connections and quick connect** (2026-11-22, asked by the owner; D64; as R-SSH-64 and R-SSH-11's quick connect). Quick connect, a card on the start screen: Host (a name or address, `host:port` for another port), a TLS switch (on at first; port 992 with TLS, 23 without, unless the host names one) and Connect; the session is not saved, its tab and title named by its address, with the default settings. Under the favourites, "Recent" lists the last five connections made (saved sessions and quick connects), newest first, kept in the settings. A saved session (found by its name, or for a quick connect by its address) is shown by its name, with its star and Edit…, and connects with its settings as they are now; one since renamed or deleted, and a quick connect, is shown by its address with "Save as session…", which opens the session dialog filled in with it. Not shown while searching. | 43 |
| R-UI-42 | **A start screen for many sessions** (2026-11-22, asked by the owner; D64; as R-SSH-65). Why the last connection ended is said at the top. The search field (from four saved sessions on) is under it and has the keyboard when the start screen shows; it looks in names and addresses and forgives a typo; its matches are one list, the best fits first, then the favourites; Up and Down choose one, Enter connects it, Escape empties the search. Each saved session is one line: star, name, address, Edit…, Connect. Without a search, "All sessions" and "Groups" (R-UI-31: each group's name, its sessions, Edit… and Start, R-UI-32) show their count and fold under their heading; "All sessions" is open while nothing is listed above it; under favourites or recent connections it is folded until the user opens it and stays as left, also after a restart (the owner, 2026-11-22); "Groups" is open at first. Quick connect, New session… and Session groups… (the groups dialog) stay in view: beside the list when the window is wide enough, above it when not. | 43 |

## 6. Ideas

Ideas that might become requirements later are in [IDEAS.md](IDEAS.md) (`IDEA-n`, with their own next free ID).

## 7. Decisions

The decisions, with the question, the choice and the alternatives, are in [DECISIONS.md](DECISIONS.md).

## 8. How to change this roadmap

- New work gets a requirement in §5 with the next free ID of its area; IDs are never reused or renumbered.
- While a requirement is built, how it is checked goes under "In progress" in [FEATURES.md](FEATURES.md).
- A requirement that is implemented, tested and checked in the running app moves, with its full row and how it
  was verified, to its phase's file in [phases/](phases/) (`phases/between-phases.md` if no phase names it), and
  gets an index row (ID, title, phase) in [REQUIREMENTS.md](REQUIREMENTS.md).
- When a phase is finished, its story (its §4 section and §3 state) moves to a new file
  `phases/phase-NN-<slug>.md`, and its §3 row links to it.
- A choice that future work must respect becomes a decision in [DECISIONS.md](DECISIONS.md).
- An idea is not a plan: it goes in [IDEAS.md](IDEAS.md) (by theme in [ideas/](ideas/)) until it is picked, then becomes requirements here.
- What is decided not to be built now, an idea or a requirement, moves with its text to [PARKED.md](PARKED.md) (D62).
- **One sequence of IDs for all roadmaps.** Parley SQL keeps its own `R-SQL` counter and Parley SSH its `R-SSH`; every other area, decisions
  (`D`), phases and ideas (`IDEA`, in [IDEAS.md](IDEAS.md)) take their next number here, whichever roadmap uses it,
  so a number never means two things.

Next free IDs (removed rows keep their numbers): `R-P-6` · `R-NET-12` · `R-DS-13` · `R-EBC-3` · `R-KEY-13` · `R-UI-43` · `R-ADD-4` · `R-NF-6` · `D65` · `Phase 44` (`R-SQL`: [sql/ROADMAP.md](sql/ROADMAP.md); `R-SSH`: [ssh/ROADMAP.md](ssh/ROADMAP.md); ideas: last line of IDEAS.md).
