# Parley requirements

> The index of every requirement that is implemented **and verified**. When one is, its row moves from
> [ROADMAP.md](ROADMAP.md) (open requirements) to its phase's file in [phases/](phases/), with how it was
> verified, and gets a row here; the text of each finished requirement is in its phase file.
> Look a requirement up by ID (`grep -rn "R-NET-1 " docs/`).

| ID | Requirement | Phase |
|---|---|---|
| R-NET-1 | Open a TCP connection to a host and port (default 23) and … | [Phase 1](phases/phase-01-connect-and-show-a-screen.md) |
| R-NET-2 | TLS connections (default port 992), checking the server … | [Phase 1](phases/phase-01-connect-and-show-a-screen.md) |
| R-NET-3 | Request a device name through the NEW-ENVIRON option (RFC … | [Phase 1](phases/phase-01-connect-and-show-a-screen.md) |
| R-NET-4 | *(Removed 2026-11-12 by R-NET-7, D16: Parley no longer … | [Phase 2](phases/phase-02-saved-sessions-and-auto-sign-on.md) |
| R-NET-5 | Keep-alive (2026-11-12): a session with it on sends telnet … | [Phase 2](phases/phase-02-saved-sessions-and-auto-sign-on.md) |
| R-NET-7 | No auto sign-on, no saved passwords | [Phase 3](phases/phase-03-keyboard-maps-and-macros.md) |
| R-NET-10 | Reconnect when the connection drops | [Phase 20](phases/phase-20-everyday-screen-work.md) |
| R-NET-11 | Keep-alive interval | [Phase 24](phases/phase-24-more-settings.md) |
| R-DS-1 | Parse the 5250 data stream commands needed for sign-on and … | [Phase 1](phases/phase-01-connect-and-show-a-screen.md) |
| R-DS-2 | A screen model: character buffer, field table (attributes … | [Phase 1](phases/phase-01-connect-and-show-a-screen.md) |
| R-DS-3 | Send AID keys (Enter, F1–F24, Page Up/Down, Clear, Help … | [Phase 1](phases/phase-01-connect-and-show-a-screen.md) |
| R-DS-4 | The other commands an IBM i sends in normal use: Write … | [Phase 1](phases/phase-01-connect-and-show-a-screen.md) |
| R-DS-5 | The data stream checked against the manual | [Phase 3](phases/phase-03-keyboard-maps-and-macros.md) |
| R-DS-6 | Save and Restore Partial Screen | [Phase 21](phases/phase-21-protocol-gaps.md) |
| R-DS-7 | Read MDT Immediate Alternate | [Phase 21](phases/phase-21-protocol-gaps.md) |
| R-DS-8 | Function keys that send no field data | [Phase 21](phases/phase-21-protocol-gaps.md) |
| R-DS-9 | Extended attributes | [Phase 21](phases/phase-21-protocol-gaps.md) |
| R-EBC-1 | Convert EBCDIC to and from Unicode for CCSID 37 (US) and … | [Phase 1](phases/phase-01-connect-and-show-a-screen.md) |
| R-EBC-2 | More code pages | [Phase 24](phases/phase-24-more-settings.md) |
| R-KEY-1 | A default keyboard mapping: Enter; Shift+Enter or … | [Phase 1](phases/phase-01-connect-and-show-a-screen.md) |
| R-KEY-2 | Keyboard maps | [Phase 3](phases/phase-03-keyboard-maps-and-macros.md) |
| R-KEY-3 | Record a macro | [Phase 3](phases/phase-03-keyboard-maps-and-macros.md) |
| R-KEY-4 | Run a macro | [Phase 3](phases/phase-03-keyboard-maps-and-macros.md) |
| R-KEY-5 | Edit a macro in Parley | [Phase 3](phases/phase-03-keyboard-maps-and-macros.md) |
| R-KEY-6 | Stop recording from the status bar | [Phase 3](phases/phase-03-keyboard-maps-and-macros.md) |
| R-KEY-7 | Import and export macros | [Phase 3](phases/phase-03-keyboard-maps-and-macros.md) |
| R-KEY-8 | No error for a key in a protected area | [Phase 4](phases/phase-04-a-modern-look.md) |
| R-KEY-9 | Operator error codes | [Phase 21](phases/phase-21-protocol-gaps.md) |
| R-KEY-10 | Mandatory enter, mandatory fill and self-check fields | [Phase 21](phases/phase-21-protocol-gaps.md) |
| R-KEY-11 | Type-ahead | [Phase 21](phases/phase-21-protocol-gaps.md) |
| R-KEY-12 | Line breaks when pasting | [Phase 24](phases/phase-24-more-settings.md) |
| R-UI-1 | The screen in a monospace grid that scales with the window … | [Phase 1](phases/phase-01-connect-and-show-a-screen.md) |
| R-UI-2 | The operator information area under the screen: Ready / X … | [Phase 1](phases/phase-01-connect-and-show-a-screen.md) |
| R-UI-3 | A connection form (host, port, TLS, CA certificate file … | [Phase 1](phases/phase-01-connect-and-show-a-screen.md) |
| R-UI-4 | A menu bar (File, Edit, View, Tools, Help) and a toolbar … | [Phase 1](phases/phase-01-connect-and-show-a-screen.md) |
| R-UI-5 | Saved sessions (2026-11-12): the start screen lists them … | [Phase 2](phases/phase-02-saved-sessions-and-auto-sign-on.md) |
| R-UI-6 | A roomier look and settings (2026-11-12): Inter as the UI … | [Phase 2](phases/phase-02-saved-sessions-and-auto-sign-on.md) |
| R-UI-7 | Every popup and side panel has a close button (×) at the … | [Phase 2](phases/phase-02-saved-sessions-and-auto-sign-on.md) |
| R-UI-8 | The settings dialog in pages (2026-11-12): a list of groups … | [Phase 2](phases/phase-02-saved-sessions-and-auto-sign-on.md) |
| R-UI-9 | Tortuga's look (2026-11-12, D12): `theme.rs` ported from … | [Phase 2](phases/phase-02-saved-sessions-and-auto-sign-on.md) |
| R-UI-10 | The menu bar starts with the app icon (the launcher's PNG … | [Phase 2](phases/phase-02-saved-sessions-and-auto-sign-on.md) |
| R-UI-11 | New version available | [Phase 2](phases/phase-02-saved-sessions-and-auto-sign-on.md) |
| R-UI-12 | Download command on Linux | [Phase 2](phases/phase-02-saved-sessions-and-auto-sign-on.md) |
| R-UI-13 | Icons | [Phase 4](phases/phase-04-a-modern-look.md) |
| R-UI-14 | An interactive status bar | [Phase 4](phases/phase-04-a-modern-look.md) |
| R-UI-15 | One header | [Phase 4](phases/phase-04-a-modern-look.md) |
| R-UI-16 | The 5250 keys in a Keys menu | [Phase 4](phases/phase-04-a-modern-look.md) |
| R-UI-17 | A keyboard shortcut sheet | [Phase 4](phases/phase-04-a-modern-look.md) |
| R-UI-18 | Settings from a gear, no Help menu | [Between phases](phases/between-phases.md) |
| R-UI-19 | A larger gear | [Between phases](phases/between-phases.md) |
| R-UI-20 | Hotspots | [Phase 20](phases/phase-20-everyday-screen-work.md) |
| R-UI-21 | Find on the screen | [Phase 20](phases/phase-20-everyday-screen-work.md) |
| R-UI-22 | Select and copy part of the screen, also as a table | [Phase 20](phases/phase-20-everyday-screen-work.md) |
| R-UI-23 | 5250 colour schemes | [Phase 20](phases/phase-20-everyday-screen-work.md) |
| R-UI-24 | Blinking text | [Phase 21](phases/phase-21-protocol-gaps.md) |
| R-UI-25 | The alarm | [Phase 21](phases/phase-21-protocol-gaps.md) |
| R-UI-26 | Frames for the host's character windows | [Phase 21](phases/phase-21-protocol-gaps.md) |
| R-UI-27 | No View menu | [Phase 24](phases/phase-24-more-settings.md) |
| R-UI-28 | Cursor shape and blink | [Phase 24](phases/phase-24-more-settings.md) |
| R-UI-29 | Japanese, Chinese and Korean text | [Between phases](phases/between-phases.md) |
| R-UI-30 | Duplicate the session | [Phase 40](phases/phase-40-more-sessions-at-once.md) |
| R-UI-31 | Session groups | [Phase 40](phases/phase-40-more-sessions-at-once.md) |
| R-UI-32 | Starting a group | [Phase 40](phases/phase-40-more-sessions-at-once.md) |
| R-UI-33 | Sessions as tabs | [Phase 40](phases/phase-40-more-sessions-at-once.md) |
| R-UI-34 | Where more sessions open | [Phase 40](phases/phase-40-more-sessions-at-once.md) |
| R-UI-35 | A colour for each session | [Between phases](phases/between-phases.md) |
| R-UI-36 | Select with Shift and the arrow keys | [Between phases](phases/between-phases.md) |
| R-ADD-1 | Settings ▸ Add-ons | [Phase 3](phases/phase-03-keyboard-maps-and-macros.md) |
| R-ADD-2 | Macros are an add-on, on by default | [Phase 3](phases/phase-03-keyboard-maps-and-macros.md) |
| R-NF-1 | Installers for every platform, built by the Release … | [Phase 1](phases/phase-01-connect-and-show-a-screen.md) |
| R-NF-2 | Both commands after installing | [Phase 4](phases/phase-04-a-modern-look.md) |
| R-NF-3 | Choose the programs when installing on Windows | [Between phases](phases/between-phases.md) |
| R-NF-4 | Three Linux packages | [Between phases](phases/between-phases.md) |
| R-NF-5 | Three Mac apps | [Between phases](phases/between-phases.md) |
