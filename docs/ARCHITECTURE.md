# Parley architecture

> How Parley is built, with pictures. The why behind each choice is a decision in [ROADMAP.md](ROADMAP.md) §7
> (`D1`…); the product view is [PRD.md](PRD.md).

## Crates

![Parley's three crates and the systems they talk to](images/architecture.svg)

| Crate | Holds | Never holds |
|---|---|---|
| `parley-core` | The TN5250 engine: telnet negotiation, 5250 records, the data stream, the screen model, EBCDIC, RFC 4777 sign-on, the macro format, the TCP/TLS connection thread, the version check. | UI code; printing. |
| `parley-ui-egui` | Everything the user sees, in egui: the 5250 screen widget, keyboard map, menus, status bar, session, settings and macro dialogs. Reaches the keychain and macro folder through traits (`Keychain`, `MacroStore`), so tests use memory. | Platform code. |
| `parley-app` | The `parley` binary: window, command line, the real keychain (`keyring`) and macro files, `--screenshot`. | Protocol logic. |

## One connection at run time

![The UI thread, the session and the network thread](images/runtime.svg)

- **`Session` is sans-I/O (D4).** `receive(bytes)` takes what the host sent; keys go in through `key(..)`;
  `take_outbound()` hands back the bytes to send. It never touches a socket or a clock, so a test is a list of bytes
  in and the exact bytes expected out.
- **The net thread** (`parley_core::net`) owns the socket. It reads with a 15 ms time-out, feeds the session, writes
  what the session queued, and sends a telnet NOP after 60 s idle when keep-alive is on (R-NET-5). TLS is rustls with
  the operating system's trust store (D6).
- **The UI thread** locks the session to draw the screen and to pass keys from the keyboard map, or from a macro
  being played (`session/playback`).
- `PARLEY_TRACE=file` appends the inbound bytes as hex, for building test fixtures. Outbound data is never traced:
  it holds what the user typed.

## Host to screen

1. **telnet** — option negotiation (terminal type, binary, end-of-record, NEW-ENVIRON for device name, code page
   and sign-on) and splitting the stream into records at IAC EOR (RFC 1205, RFC 2877, RFC 4777).
2. **record** — the 5250 record header: opcode and flags.
3. **datastream** — commands (Write To Display, Read MDT Fields, Save/Restore Screen, Write Error Code, Query, …)
   and orders (Set Buffer Address, Start of Field, Repeat to Address, …), cited against IBM's 5250 Functions
   Reference (SC30-3533).
4. **screen** — the character buffer, attributes, format table (fields), cursor and operator information.

## Keys to host

1. **keymap** (UI) turns a PC key into a 5250 key (R-KEY-1; per-session maps are R-KEY-2).
2. **input** applies the field rules: input-capable, numeric only, monocase, field exit required, auto enter.
3. **reply** builds the answer for an AID key: cursor address, AID code and the modified fields.
4. **telnet** escapes IAC bytes and adds EOR; the bytes wait in the outbound queue for the net thread.

## Sign-on and secrets

- Passwords live only in the OS keychain (D11), under service "Parley" and the session's name; `Password` has a
  `Debug` that hides it.
- **Screen sign-on** types user and password into the first screen with a user field and a non-display field.
- **Bypass sign-on** answers the host's NEW-ENVIRON request with the user and an RFC 4777 password substitute (DES or
  SHA-1) computed from the host's seed, so the password never crosses the network (R-NET-4).

## Macros

A macro is a text file, one step per line (`text "…"`, key names such as `enter` or `f3`, `wait`, `password`,
`move ROW COLUMN`), in the `macros` folder beside the settings (D14). Recording leaves out passwords (a `password`
step plays the keychain's) and refused keys; playing waits for the host to unlock the keyboard after each AID key
and stops at an operator error, Reset or Esc (R-KEY-3, R-KEY-4).

## Testing

- Unit tests next to the code; whole-session tests in `crates/parley-core/tests` with recorded or hand-built
  records — never a live IBM i in `cargo test`.
- UI tests with `egui_kittest`, including a test per dialog that its buttons stay in place.
- `make verify` runs format, clippy (pedantic), all tests and `cargo deny`, as CI does.
- By hand: `make probe HOST=pub400.com` drives a real host without the UI; the running app is checked on each OS
  before a requirement counts as done ([FEATURES.md](FEATURES.md)).

## Regenerating the pictures

The diagrams in `docs/images/*.svg` are hand-written SVG; edit them as text and check them with
`resvg --use-fonts-dir crates/parley-ui-egui/assets/fonts docs/images/runtime.svg target/runtime.png`.
The screenshots come from `parley --screenshot FILE` (a PPM; convert it to PNG) run with a throwaway data folder
(`XDG_DATA_HOME=…` on Linux) so no real sessions show.
