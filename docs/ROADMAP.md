# Parley roadmap

> The plan, and the spec for everything that is not done yet. Finished phases, with the text of their
> requirements and how each was verified, are in [phases/](phases/); [REQUIREMENTS.md](REQUIREMENTS.md) is the index of finished IDs.
> Look things up with `grep -n` (an ID such as `"R-NET-1 "`, `"| D3 "` or a heading) rather than reading the whole file.

**Status of this document:** first written while setting up the development environment (2026-11-12). Phases 1–4
are built and verified (the last on 2026-11-15); Phase 20, everyday screen work, is done too.

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
| [Phase 0: development environment](phases/phase-00-development-environment.md) | Done (2026-11-12): workspace, Makefile, setup scripts, CI, lints, Claude Code setup. |
| [Phase 1: connect and show a screen](phases/phase-01-connect-and-show-a-screen.md) | Done (2026-11-12): verified on Windows, macOS, Linux and WSL2 (FEATURES.md). |
| [Phase 2: saved sessions and auto sign-on](phases/phase-02-saved-sessions-and-auto-sign-on.md) | Done (2026-11-12): verified on Windows, macOS, Linux and WSL2 (FEATURES.md). Auto sign-on and saved passwords were removed again in Phase 3 (R-NET-7, D16). |
| [Phase 3: keyboard maps and macros](phases/phase-03-keyboard-maps-and-macros.md) | Done (2026-11-12): verified on Windows, macOS and Linux (FEATURES.md): R-DS-5, keyboard maps (R-KEY-2), macros (R-KEY-3, R-KEY-4), add-ons (R-ADD-1, R-ADD-2), auto sign-on removed (R-NET-7, D16). The macro editor (R-KEY-5), Stop in the status bar (R-KEY-6) and import and export of macros (R-KEY-7) verified 2026-11-15. |
| [Phase 4: a modern look](phases/phase-04-a-modern-look.md) | Done (2026-11-15): verified on Windows, macOS, Linux and WSL (FEATURES.md): icons (R-UI-13, D17), an interactive status bar (R-UI-14), one header with the session (R-UI-15), the 5250 keys in a Keys menu in the status bar (R-UI-16), a shortcut sheet (R-UI-17), no error for a key in a protected area (R-KEY-8); both commands after installing (R-NF-2). |
| Phases 5–19: SQL | Parley SQL's phases: [sql/ROADMAP.md](sql/ROADMAP.md). |
| [Phase 20: everyday screen work](phases/phase-20-everyday-screen-work.md) | Done (2026-11-15): checked by the owner in the running app (FEATURES.md): R-UI-20..23, R-NET-10. |
| [Phase 21: protocol gaps](phases/phase-21-protocol-gaps.md) | Done (2026-11-16): R-DS-6, R-DS-7 (IDEA-7), R-DS-8 (IDEA-4) done; R-UI-24, R-UI-25 (IDEA-6) done; R-KEY-9 (IDEA-2) done; R-KEY-10 (IDEA-3) done; R-KEY-11 (IDEA-14) done; IDEA-5: R-DS-9 extended attributes done; the GUI is Phase 22. |
| [Phase 24: more settings](phases/phase-24-more-settings.md) | Done (2026-11-16), checked by the owner on Windows, macOS, Linux and WSL: R-NET-11, R-UI-28, R-KEY-12, R-EBC-2; Parley SQL's are Phase 25 in [sql/ROADMAP.md](sql/ROADMAP.md). |
| Phases 26–39: SSH | Parley SSH's phases: [ssh/ROADMAP.md](ssh/ROADMAP.md). |
| Phase 22: GUI elements (beta) | Paused (2026-11-15): R-DS-10 windows built but never sent by DKSOFT00 (see §4); then R-DS-11 selection fields and the mouse, R-DS-12 scroll bars, behind a session switch (D38). |

## 4. Phases

Finished phases are in [phases/](phases/), one file each.

### Phase 22 — GUI elements (planned)

From IDEA-5 (the owner, 2026-11-15): IBM i's GUI-like 5250 (enhanced user interface level 1) draws pop-up windows,
menus, radio buttons, check boxes, push buttons and scroll bars. A display that claims it gets all of them, so
Parley claims it only behind a session switch, off by default, while it is built in steps (D38): windows (R-DS-10),
selection fields and the mouse (R-DS-11), scroll bars (R-DS-12). Each step is checked on DKSOFT00 with a trace and a
macro the owner runs.

**Paused (2026-11-15).** R-DS-10 is built and tested, but DKSOFT00 never sent a GUI structured field: not with level 1
claimed in a 58-byte reply, not with tn5250j's enhanced claim (byte 4 X'07', byte 5 X'08', 64 bytes), and not on a
display device created fresh by that reply (PARLYGUI, type 3477 FC). Something else on IBM i decides when it uses the
enhanced interface. Before R-DS-11: find IBM's rules for it (a system, user, job, device or display file setting, or
how IBM's own emulators identify themselves), then check R-DS-10 live.

### Later (unordered, to be planned)

Printer sessions (IDEA-18); see [IDEAS.md](IDEAS.md). SQL work is planned in [sql/ROADMAP.md](sql/ROADMAP.md).

## 5. Requirements

Open requirements, in the order they are planned. The done ones are in [REQUIREMENTS.md](REQUIREMENTS.md); new
ones come from [IDEAS.md](IDEAS.md). Areas: `NET` connection and telnet, `DS` the 5250 data stream,
`EBC` character sets, `KEY` keyboard, `UI` user interface, `ADD` add-ons, `NF` non-functional. The `SQL` area, and
the SQL window's add-on switches, probes and assistant, are in [sql/ROADMAP.md](sql/ROADMAP.md).

| ID | Requirement | Phase |
|---|---|---|
| R-DS-10 | **GUI windows, behind a switch** (2026-11-15, from IDEA-5; D38; *the switch was taken out of the session dialog again on 2026-11-15, as IBM i never sent windows; the engine part stays, see Phase 22*; SC30-3533-04 §15.6.13.1–4). A session setting "GUI elements (beta)", off by default; when on, the query reply claims what tn5250j's enhanced mode claims: GUI-like characters, enhanced user interface level 1 and Write Error Code to Window (device capability byte 4 bits 5–7) and level 3 (byte 5 bit 4), in a 64-byte reply (with level 1 alone, DKSOFT00 sent no windows); the pointer device (byte 4 bit 4) waits for R-DS-11. Write to Display Structured Field (WDSF) then carries out Create Window (X'51': a window of the given depth and width at the current address, with its border and optional title as the border and title minor structures say, drawn as a framed panel over the screen; the cursor kept inside when its flag says so; up to 128 windows, one of the same place and size replacing the other), Unrestricted Window Cursor Movement (X'52'), Remove GUI Window (X'59') and Remove All GUI Constructs (X'5F'); Clear Unit, Clear Unit Alternate and Restore Screen remove them too. Text written inside a window shows in it. Off, nothing changes. | 22 |
| R-DS-11 | **GUI selection fields and the mouse** (2026-11-15, from IDEA-5; D38; SC30-3533-04 §15.6.13.5–6, §15.6.13.9). With the switch on: Define Selection Field (X'50') draws menus, single and multiple choice fields (radio buttons, check boxes) and push buttons with their choices, mnemonics and states; the arrow keys, the mnemonics and a click select a choice and the field's AID or Enter goes to the host with the selection as the manual says; Remove GUI Selection Field (X'58'); Write Data (X'5E') into a field. Details are taken from the manual when this is planned. | 22 |
| R-DS-12 | **GUI scroll bars and mouse buttons** (2026-11-15, from IDEA-5; D38; SC30-3533-04 §15.6.13.7–8, §15.6.13.10). With the switch on: Define Scroll Bar Field (X'53'), drawn beside a list and sending the scroll AID with the new position when dragged or clicked; Remove GUI Scroll Bar Field; Programmable Mouse Buttons. Details are taken from the manual when this is planned. | 22 |

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
- **One sequence of IDs for all roadmaps.** Parley SQL keeps its own `R-SQL` counter and Parley SSH its `R-SSH`; every other area, decisions
  (`D`), phases and ideas (`IDEA`, in [IDEAS.md](IDEAS.md)) take their next number here, whichever roadmap uses it,
  so a number never means two things.

Next free IDs (removed rows keep their numbers): `R-P-6` · `R-NET-12` · `R-DS-13` · `R-EBC-3` · `R-KEY-13` · `R-UI-29` · `R-ADD-4` · `R-NF-3` · `D49` · `Phase 40` (`R-SQL`: [sql/ROADMAP.md](sql/ROADMAP.md); `R-SSH`: [ssh/ROADMAP.md](ssh/ROADMAP.md); ideas: last line of IDEAS.md).
