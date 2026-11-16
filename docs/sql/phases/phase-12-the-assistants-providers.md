# Phase 12 — The assistant's providers

> Finished phase of Parley SQL. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

Asked by the owner (2026-11-14): the SQL assistant gets a settings page of its own, where it is turned on and off,
and works with the major AI providers and with a local model through llama.cpp (R-SQL-48). The choices are D27.
Also asked: the results below or beside the editor (R-SQL-49), the editor's height dragged (R-SQL-50) and the side
panel folded away (R-SQL-51).

**Status:** Done, verified (2026-11-14, Linux under WSL): Settings ▸ SQL assistant in both programs, with the switch, the provider (Anthropic, OpenAI, Google Gemini, Mistral, llama.cpp on a local `llama-server`, or any OpenAI-compatible server), the model and address, and the API key in the keychain (R-SQL-48); D27.

## Requirements

| ID | Requirement |
|---|---|
| R-SQL-48 | **The SQL assistant's settings and providers** (2026-11-14; D27. *Changes R-SQL-44, R-ADD-3 and R-SQL-47.*) Settings in both programs has a page "SQL assistant": the switch that turns the assistant on (off by default; in Parley it also needs the SQL add-on, and says so), the provider (Anthropic (Claude), OpenAI, Google Gemini, Mistral, llama.cpp (local), OpenAI-compatible), the model (empty: the provider's default; for llama.cpp the server's own), the address (llama.cpp, default `http://localhost:8080/v1`, and OpenAI-compatible, which must give one), and the API key: where it comes from (the provider's environment variable, which comes first, the keychain, or none, said as a warning when the provider needs one), a field that saves a key in the operating system's keychain at once with Save key, and Remove. The key is never in the settings or a log. The page says what is sent and to whom; for llama.cpp it says to start `llama-server` with `--jinja`, and Parley asks it for tool calls only (`tool_choice: "required"`), as small models otherwise write their calls as text. Restore defaults turns the assistant off and resets the provider, model and address, and leaves a saved key. The Add-ons page and Parley SQL's SQL page no longer have the switch. The Ask row names the provider and model; without a needed key or an address nothing is sent and the window says how to give one. |
| R-SQL-49 | **Results below or beside the editor** (2026-11-14; as Tortuga's R-SET-12, D12). Settings ▸ Appearance has a "SQL window" group (in Parley SQL always; in Parley while the SQL add-on is on) with "Results": Below (the default: the editor above, Results and Messages below, as before), Beside (the editor with its Ask row, Run buttons and error line on the left, Results and Messages on the right) and Automatic (beside when the window's work area is at least 1100 points wide, else below). It takes effect in an open window at once. Beside, a divider between the two can be dragged to change their widths (each keeps at least 320 points) and double-clicked to share them equally; the widths last while the window is open. Restore defaults on Appearance sets Below. |
| R-SQL-50 | **Resize the editor** (2026-11-14; asked by the owner). With the results below the editor (R-SQL-49), a divider between them can be dragged up and down to change the height of the editor's text; the text keeps at least 90 points and the results at least 120. Double-clicking the divider gives back the usual height (as high as the text needs, up to a share of the window). The height lasts while the window is open. |
| R-SQL-51 | **Fold the side panel** (2026-11-14; asked by the owner). The panel with History, Snippets and Database has a small button at the top right of its tabs, "Hide the side panel", that folds it to a narrow strip at the window's right edge; the strip has one button, "Show History, Snippets and Database", that opens it again at the width it had. The editor and results get the room. Whether it is folded lasts while the window is open. *Changed 2026-11-15 by R-SQL-82 (the panel is on the left; folded is kept between runs).* |

## Verification

- **R-SQL-50, R-SQL-51** (2026-11-14): the editor's height and folding the side panel, built. **Automated**
  (`make test`): the editor's height stays within its limits; in kittest dragging the divider below the editor down
  by 80 points moves the results down by as much, dragging it past the bottom leaves the results in the window, and
  the side panel folds (the editor gets wider, History is gone) and opens again. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-49** (2026-11-14): results below or beside the editor, built. **Automated** (`make test`): Automatic is
  below under 1100 points and beside from there; each column keeps its least width; the setting is kept by name and chosen on the Appearance page; in
  kittest the Results tab is under the Run button when Below and to its right when Beside, where the editor fills the column down to the Run buttons, with a divider that stays
  in place, and Automatic shows the divider only in a wide window. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).

- **R-SQL-48** (2026-11-14): the SQL assistant's settings page and providers, built. **Automated** (`make test`):
  each provider has an address and model unless given, a local one needs no key; the Anthropic and OpenAI formats
  build their requests and read tool calls, answers, words, refusals and cut-off replies from hand-built fixtures
  (arguments as text or as an object, a call without an id); HTTP errors are said by provider (Gemini's list form);
  without a needed key or an address nothing is sent; the key comes from the environment, then the keychain, and
  never shows in `Debug` or the settings; in kittest the page turns the assistant on and keeps its place, a key is
  saved to and removed from a keychain in memory, llama.cpp shows its address and the `--jinja` hint, and Parley's
  Add-ons page no longer has the switch. **Seen** (WSL2): the engine, through `examples/askprobe.rs` and a
  made-up library, asked the owner's `llama-server` (b8681, Qwen2.5-Coder-7B Q4_K_M): it listed the tables, read two
  tables' columns and answered with a correct join in four rounds (~40 s). Without `tool_choice: "required"` that model
  wrote its calls as text, so llama.cpp now forces calls. **Checked in the running app** by the owner, Linux under WSL (2026-11-14).
