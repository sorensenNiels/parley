# Phase 2 — Saved sessions and auto sign-on

> Finished phase of Parley 5250. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

Several named sessions with their own settings, a session dialog, passwords in the keychain, auto sign-on (typed or
bypassed per RFC 4777) and keep-alive: R-UI-5, R-NET-4, R-NET-5. *(Auto sign-on and the saved passwords were removed in
Phase 3 as a security risk: R-NET-7, D16.)* A roomier look and a settings dialog with the terminal
font: R-UI-6. Close buttons on popups: R-UI-7. Settings in pages: R-UI-8. Tortuga's look: R-UI-9. App name in the menu bar, Help on the right: R-UI-10. New version available, with the download command on Linux:
R-UI-11, R-UI-12.

**Status:** Done (2026-11-12): verified on Windows, macOS, Linux and WSL2 (FEATURES.md). Auto sign-on and saved passwords were removed again in Phase 3 (R-NET-7, D16).

## Requirements

| ID | Requirement |
|---|---|
| R-NET-4 | *(Removed 2026-11-12 by R-NET-7, D16: Parley no longer signs on by itself or keeps passwords.)* Auto sign-on (2026-11-12). *Screen*: on the first screen whose first two input fields are a user field and a non-display field, type the user and password and press Enter, once per connection. *Bypass*: answer the host's NEW-ENVIRON SEND with `VAR USER`, `USERVAR IBMRSEED` (a random client seed) and `USERVAR IBMSUBSPW` (the password substitute) per RFC 4777 section 5, DES (5.1) or SHA-1 (5.2) as the session's password-encryption setting says (QPWDLVL 0–1 or 2–3); the host needs QRMTSIGN `*VERIFY` or `*SAMEPRF`. Only when the host sends its seed: the password never goes in clear text. Checked against the RFC's test vectors and its example negotiation, byte for byte. |
| R-NET-5 | Keep-alive (2026-11-12): a session with it on sends telnet NOP (IAC X'F1') after 60 s without sending anything, so firewalls keep an idle connection. |
| R-UI-5 | Saved sessions (2026-11-12): the start screen lists them (Connect, Edit…, New session…) and File has New session…, Edit sessions… and Connect to ▸. A session dialog, as in IBM-style emulators: name (and a menu to switch session), host, port and TLS, CA certificate, code page, terminal size (27×132 / 24×80), device name; auto sign-on (user, password, enable, bypass the sign-on screen, password encryption); run macro (shown disabled until IDEA-11); exit when the host ends the session; confirm exit while connected; send keep-alive; connect on program start (one session at most); Help; Cancel, Delete, Apply, Connect (disabled while connected: one session per window). New sessions default to TLS on 992 (R-P-3). Passwords go to the operating system's keychain (D11), never to the settings file; renaming or deleting a session moves or removes its password. *(Auto sign-on and the password were removed by R-NET-7: the dialog no longer has them.)* The single connection Parley 0.1 remembered becomes the first saved session. |
| R-UI-6 | A roomier look and settings (2026-11-12): Inter as the UI font, larger text and more padding and spacing (sizes since R-UI-9: Tortuga's) in menus, toolbar, status bar, dialogs and the start screen (sessions as cards). Tools ▸ Settings…: theme (System / Light / Dark), size (90–175 %), and the terminal font: one of the bundled JetBrains Mono (default since R-UI-9), IBM Plex Mono and Hack, or any installed monospace font (found on a background thread; a font counts as monospace when i, M, W and . are equally wide), with a preview. Changes show at once; Cancel restores them; the settings are remembered. A saved font that is no longer installed falls back to the default with a note. Installers carry the fonts' OFL licence files. |
| R-UI-7 | Every popup and side panel has a close button (×) at the top right of its title row (2026-11-12): the session dialog (closes without saving, as Cancel), Settings (as Cancel: the old settings come back), the exit question (as Cancel) and the keyboard shortcuts panel (hides it). Screen readers name it "Close". |
| R-UI-8 | The settings dialog in pages (2026-11-12): a list of groups on the left (Appearance, Terminal; new settings add a group or go into one), the chosen page on the right with its title and a one-line description. The dialog keeps one size whichever page is shown. "Restore defaults" resets only the page shown. |
| R-UI-9 | Tortuga's look (2026-11-12, D12): `theme.rs` ported from Tortuga: Inter (Regular, SemiBold for headings) and JetBrains Mono, the same light and dark palette (teal accent), text sizes, spacing, corner radii and shadows, thin floating scroll bars; the menu bar, toolbar and status bar in the chrome colour; filled primary (Connect, Save, Enter, Exit) and danger (Delete) buttons; switches for on/off options and segmented choices; dialogs with Tortuga's frame and Cancel/confirm row; the settings dialog laid out as Tortuga's (pages under section headings, groups of rows on cards); a Density setting (Comfortable / Compact). The 5250 screen keeps its own colours. |
| R-UI-10 | The menu bar starts with the app icon (the launcher's PNG, 18 px) and "Parley" in SemiBold, then a separator and File, Edit, View, Tools; Help sits at the right edge (2026-11-12). As Tortuga shows its own name at the start of its top bar. |
| R-UI-11 | **New version available** (2026-11-12, D13; as Tortuga's R-SET-15). Parley asks GitHub for the latest published release (`api.github.com/repos/sorensenNiels/parley/releases/latest`, HTTPS with the operating system's trust store), sending nothing but the request (`User-Agent: Parley/<version>`). **At start-up**, in the background, unless Settings → About → "Look for a new version when Parley starts" (on by default) is off: a newer version shows as a quiet link in the menu bar, beside Help ("Parley 0.3.0 is available"), that opens its release page; nothing pops up, and a failed check says nothing. **Check now** on the About page, and Help ▸ **Check for updates…** (which opens the About page), say what they found: up to date, the newer version with a link, or why it failed. Only published releases count; a pre-release version is older than its release. The About page also shows the version, the platform it was built for, the licence and the source link, and copies the version. |
| R-UI-12 | **Download command on Linux** (2026-11-12; as Tortuga's R-SET-18). When a newer version is available, on Linux a **Copy download command** menu sits beside the link on Settings → About and lists the release's `.deb` and `.AppImage` by file name; picking one copies `curl -fLO '<url>'` and the page says what was copied. The URLs come from GitHub's answer (only `https` ones that can't break out of the quotes). Nothing is installed by the app. |

## Verification

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
