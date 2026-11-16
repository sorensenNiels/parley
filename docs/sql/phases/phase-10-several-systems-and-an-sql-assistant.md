# Phase 10 — Several systems and an SQL assistant

> Finished phase of Parley SQL. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

From IDEA-32 and IDEA-33 (2026-11-14): run one read-only statement on several saved sessions, a result tab each
(R-SQL-43, D24); then an SQL assistant that turns plain words into SQL with the catalog's names only (R-SQL-44,
D25), an add-on that is off until turned on (R-ADD-3). The owner's choices: a tab per system, read-only statements
only across systems, names only to the model, Claude Opus 5.5.

**Status:** Done, verified (2026-11-14, Linux under WSL): one statement on several systems (R-SQL-43, D24), an SQL assistant from plain words (R-SQL-44, D25), as an add-on off by default (R-ADD-3).

## Requirements

| ID | Requirement |
|---|---|
| R-SQL-43 | **One statement on several systems** (2026-11-14; *removed with Parley's SQL window by R-SQL-70, D30*). "Run on…" in the SQL window lists the other saved sessions; ticked ones are signed on in the window (a user and a password typed for each, used once and forgotten, D16), each in a helper of its own. "Run on all" runs the statement under the cursor on the window's system and every ticked one at once, only if it reads (`SELECT`, `VALUES`, `WITH`; never `CL:`, never two statements), and opens a result tab per system named after it; an error on one system is said in Messages under its name and the others go on. A system's rows are its first block; more are not fetched (said in the tab). The sign-ons last until the window closes or the system is unticked (D24). |
| R-SQL-44 | **An SQL assistant** (2026-11-14). An "Ask" field over the editor takes a question in plain words; Parley sends it to Claude (`claude-opus-5-5`) with the current schema and library list, and lets the model look up a library's tables and a table's columns through the schema browser's catalog (names, types, headings and texts only). The answer's SQL goes into a new script tab with the model's short explanation as comments; nothing runs by itself. Before the first question the window says what is sent and where; a request in flight can be cancelled; errors (no key, refused, network) are said plainly (D25). |
| R-ADD-3 | **The SQL assistant is an add-on, off by default** (2026-11-14. *Changed by R-SQL-48: the switch is on Settings ▸ SQL assistant, not Add-ons, and the key may also be in the keychain.*) "SQL assistant" in Settings ▸ Add-ons ("Ask Claude to write SQL from plain words; sends table and column names to Anthropic"), off until turned on; it needs the SQL add-on and an Anthropic API key in the environment variable `ANTHROPIC_API_KEY`, which Parley reads when asking and never stores. Turned off, nothing of it shows and nothing is sent (D25). |

## Verification

- **R-SQL-43** (2026-11-14): one statement on several systems, built. "Run on…" in the run row lists the other saved
  sessions; ticking one asks for its user and password (used once, D16) and signs on in a helper of its own;
  "Run on all (n)" runs the statement under the cursor here and on each, only if it is one statement that reads
  (`SELECT`, `VALUES`, `WITH`; not `CL:`, not with markers); every system's rows get a tab named after it, its first
  block only (the cursor is closed; "more not fetched" said), errors in Messages under the system's name.
  **Automated:** what reads, sign-on and answers per system with its name, ticking and unticking, the window's
  refusals and tabs, the sign-on dialog's buttons in place. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-44, R-ADD-3** (2026-11-14): the SQL assistant, built. "SQL assistant" in Settings ▸ Add-ons, off by
  default; on, an Ask row over the editor says what is sent and where. A question goes to Claude Opus 5.5
  (Messages API `2026-10-21` over `ureq`, refusal fallback `"default"`, effort `medium`) with the release, current
  schema and library list; the model looks up a library's tables and a table's columns through two tools answered
  from the schema browser's catalog (names, types, headings and texts only, D25) and answers through a third; the
  SQL opens in a new script tab under its explanation as comments, never run. The key comes from
  `ANTHROPIC_API_KEY` when asking and is dropped after. **Automated:** the request (model, tools, fallback, no
  sampling settings), replies (lookups with thinking blocks sent back unchanged, the answer, words only, refusal,
  cut off), HTTP errors said plainly, lookups answered from the cache and waiting while it is read, the script
  text, no key, cancel, the add-on switch. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).
