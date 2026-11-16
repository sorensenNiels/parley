# Phase 24 — More settings

> Finished phase of Parley 5250. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

**Status:** Done (2026-11-16), checked by the owner on Windows, macOS, Linux and WSL: R-NET-11, R-UI-28, R-KEY-12, R-EBC-2; Parley SQL's are Phase 25 in [sql/ROADMAP.md](../sql/ROADMAP.md).

## Requirements

| ID | Requirement |
|---|---|
| R-NET-11 | **Keep-alive interval** (2026-11-16, the owner chose it from a review of fixed values). The session dialog's "Send keep-alive" gets "every N seconds" (10–3600, default 60), so a firewall that drops idle connections sooner is kept open. *(Changes R-NET-5's fixed 60 s.)* |
| R-EBC-2 | **More code pages** (2026-11-16, from the same review). Besides 37, 277, 1140 and 1142: 273 (Germany, Austria), 285 (United Kingdom), 297 (France), 500 (International), 871 (Iceland), and their euro variants 1141, 1146, 1147, 1148, 1149, generated from iconv by `scripts/gen-ebcdic.py` as the others; each announced with its keyboard type (AGB, UKB, FAB, BLI, ICB) and character set (697, or 695 for the euro pages) as R-EBC-1. Parley SQL's code page choice gets them too. |
| R-KEY-12 | **Line breaks when pasting** (2026-11-16, from the same review). Settings ▸ Terminal ▸ "Paste line breaks": "Next field" (default: a line break is Field Exit, as R-UI-1 has it) or "Next row" (the next line goes in at the same column one row down, as PCOMM can). A tab is always Field Exit. |
| R-UI-27 | **No View menu** (2026-11-16, asked by the owner). The header's View menu, which held only Keyboard shortcuts, is removed: Settings ▸ Keyboard shows and changes the keys, and the shortcut sheet still opens with its key (Ctrl+/ in the Default map, R-UI-17). The header reads File, Edit, Tools (while it has items), then the session pill and the gear. *(Changes R-UI-17: the sheet no longer opens from a menu.)* |
| R-UI-28 | **Cursor shape and blink** (2026-11-16, from the same review). Settings ▸ Terminal ▸ "Cursor": Block (default), Underline or Bar, and "Blinking cursor" (off by default; half a second on, half off, steady while typing). In insert mode the cursor shows as a bar, or as a block when the chosen shape is the bar, so insert mode stays visible. |

## Verification

### Phase 24: more settings, and no View menu (2026-11-16) — R-NET-11, R-UI-28, R-KEY-12, R-EBC-2, R-UI-27

- **Phase 24: more settings** (2026-11-16), built; `make verify` on Linux under WSL. **Automated** (`make test`):
  - **R-NET-11** the keep-alive interval defaults to 60 s, is kept within 10–3600 s, and old sessions read with
    60 s; the session dialog shows "Keep-alive every" only while keep-alive is on and saves it.
  - **R-UI-28** insert mode always looks different (block or underline → bar, bar → block); a blinking cursor stays
    on for a second after a key, then blinks; the Terminal page saves the shape and the blink.
  - **R-KEY-12** pasted text: CR, LF and CR LF are one line break, a tab is Field Exit; "Next field" puts the
    cursor in the next field, "Next row" one row down at the column the paste began in.
  - **R-EBC-2** the new pages have their national letters (ß, £, é, ç, Þ) and the euro variants the euro sign;
    character set 695 for the euro pages; keyboard types; the bundled terminal font has every character of every
    page; Parley SQL's `--ccsid` takes them.
  **Checked in the running app** by the owner on Windows, macOS, Linux and Linux under WSL (2026-11-16).

- **R-UI-27** (2026-11-16): no View menu. **Automated** (`make test`): the header has File, Edit, Tools and no View
  menu or Keyboard shortcuts item; Tools still goes when it has no items; Ctrl+/ still opens the sheet (R-UI-17's
  tests). **Checked in the running app** by the owner on Windows, macOS, Linux and Linux under WSL (2026-11-16).
