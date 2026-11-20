# Phase 37 — Themes and fonts

> Finished phase of Parley SSH. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

Themes people know by look, following the system's light and dark, fonts with Nerd Font symbols and emoji. R-SSH-20
(rewritten).

**Status:** Done (2026-11-20), checked by the owner on Windows, macOS, Linux and WSL: R-SSH-20.

## Requirements

| ID | Requirement |
|---|---|
| R-SSH-20 | **Themes and fonts** (*rewritten 2026-11-16, D46; was PuTTY's Window, Appearance and Colours pages; details 2026-11-16, D49*). Settings ▸ Appearance: a terminal theme from a gallery, each shown as a small preview of coloured text: Parley dark and light, Solarized dark and light, Nord, Dracula, Gruvbox dark and light, Catppuccin Latte and Mocha, Tokyo Night and Tokyo Night Day. One theme for when the system is dark and one for when it is light (Parley SSH's theme setting, Light, Dark or System, chooses which); the theme colours the screen only (D49). The terminal font, bundled JetBrains Mono or an installed monospace font, with real bold, italic and bold italic (bundled faces, D49; an installed font's own faces when it has them); Nerd Font symbols and emoji drawn from bundled fallbacks (monochrome). The font size from 8 to 32 with Ctrl+= / Ctrl+- / Ctrl+0 (Cmd on macOS) for every pane, kept. The cursor: block, bar or underline, blinking or not. Padding around the screen (0–24 px). Every change shows at once. *Completes R-SSH-8's italic.* |

## Verification

- **R-SSH-20** (built 2026-11-16, Linux under WSL; D49). **Automated** (`make test`): `parley-ssh-core`'s `term/tests.rs` (the user's cursor until the host chooses, back with `CSI 0 SP q` and RIS); `parley-ssh-ui`'s `screen/themes.rs` (every theme listed once, dark or light as its colours say, text and cursor readable on its background, kept by name), `fonts.rs` (the bundled faces and both fallbacks in every face's family, a missing font falling back with its faces, the fallbacks holding powerline and emoji glyphs, a face the font lacks faked), `screen.rs` (padding and a bigger font leave fewer cells; the cursor set), `window_keys.rs` (Ctrl+= / Ctrl+- / Ctrl+0, Cmd on macOS), `tests.rs` (View sizes the screen's text within 8–32; the Appearance page choosing a dark theme and the cursor with its buttons staying put). Seen: Parley SSH starts with the new fonts (`--screenshot`, Linux under WSL).
  - R-SSH-20: 12 themes (`screen/themes.rs`), one for dark and one for light, previewed on Settings ▸ Appearance with a colour strip in each list; the font (bundled or installed, `font_choice` shared with Parley 5250) with real bold, italic and bold italic (JetBrains Mono's bundled faces, an installed font's own), Symbols Nerd Font Mono and Noto Emoji as fallbacks; size 8–32 with Ctrl+= / Ctrl+- / Ctrl+0 and View's three items; cursor block, bar or underline, blinking or not (DECSCUSR 0 gives it back); padding 0–24. The themes' own colours are kept: Solarized Light's text is 4.1:1 and Catppuccin Latte's cursor 2.3:1. View's Larger/Smaller/Actual size now size the screen's text; Appearance's Text size still sizes everything. Checked: Each theme with `ls --color`, vim and htop, in light and dark; a starship or oh-my-posh prompt with Nerd symbols; emoji; bold and italic (`printf '\e[1mbold \e[3mitalic\e[0m'`) with JetBrains Mono and with an installed font; Ctrl+= / Ctrl+- / Ctrl+0; the cursor choices and vim changing the cursor; padding; the Appearance page in light and dark.
  **Checked in the running app** by the owner on Windows, macOS, Linux and WSL (2026-11-20).
