# Phase 15 — Data transfer

> Finished phase of Parley SQL. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

From IDEA-26 (the owner, 2026-11-14): every row of a result or a table to a CSV or Excel file (R-SQL-71, R-SQL-72),
and a CSV file into a table, all or nothing (R-SQL-73), as Access Client Solutions' Data Transfer does (D31).
Download needs no change to the helper; upload adds one request to it.

**Status:** Done, verified (2026-11-15, Linux under WSL): every row to CSV or Excel (R-SQL-71, R-SQL-72), a CSV file into a table, all or nothing (R-SQL-73); D31.

## Requirements

| ID | Requirement |
|---|---|
| R-SQL-71 | **Download every row** (2026-11-14; IDEA-26, D31). A result tab's "Download…" (beside Export…) and the schema browser's "Download…" on a table or view (which runs `SELECT * FROM lib.table`) write every row of the query to a file the operator picks in the file window, not only the rows fetched: the window fetches the rest of the open cursor in batches of "Rows at a time" (R-SQL-18) and writes each batch as it comes, so memory stays small. The format is CSV (UTF-8, comma, a header row of field names, quoted as Export… does) or Excel (R-SQL-72), chosen in the dialog with a switch for "IBM i column headings as the first row" (R-SQL-29) instead of the field names. The grid's filter and sort do not apply: the file holds the query's rows in the host's order. While it runs, the run row shows "Downloading… *n* rows" with Cancel; Cancel stops and removes the part written. When done, Messages says how many rows went to which file. A tab's download takes the rest of its cursor, so the tab offers no Fetch more afterwards. |
| R-SQL-72 | **Excel files** (2026-11-14; D31). Download's Excel format writes an `.xlsx` file (with `rust_xlsxwriter`; no Excel needed): one sheet named after the table, or "Result", with a bold header row that stays in view. Numbers are numbers (DECIMAL and NUMERIC with more than 15 significant digits are written as text, so no digit is lost), DATE and TIMESTAMP are Excel dates shown in ISO form, TIME is text, NULL is an empty cell, binary columns are hex (R-SQL-39). Past Excel's 1 048 576 rows a sheet, the rows go on in a new sheet ("Result (2)"). |
| R-SQL-73 | **Upload a CSV file into a table, all or nothing** (2026-11-14; IDEA-26, D31). The schema browser's "Upload CSV…" on a table (and "Upload CSV…" in the SQL window's top bar, which asks for the library and table) opens a dialog: the file (UTF-8, a BOM allowed), "First row is column names" (on), the separator (found from the first line: comma, semicolon or tab; changeable), and a preview of the first 10 rows under the table's columns, matched by name (ignoring case) or, without names, by position. Columns of the file that match none, and columns of the table left out, are listed; a column the table needs (NOT NULL without a default) left out is an error before anything is sent. An empty field is NULL where the column allows it. Upload inserts every row in one transaction, whatever the window's autocommit (R-SQL-5): the helper prepares one `INSERT` and sends the rows in batches; when every row went in it commits, otherwise it rolls back and the dialog says which row of the file failed, which column if known, and the host's message (a table that is not journaled says so, SQL7008). Progress shows rows sent with Cancel, which rolls back. When done, Messages says how many rows went into which table. While the window has changes not committed (R-SQL-5) the upload waits: it says to commit or roll back first, since its end commits. |

## Verification

- **R-SQL-71..73** (2026-11-15): data transfer (Phase 15, D31), built. **Automated** (`make test`, `make helper`): in
  `parley-core` CSV lines quoted as RFC 4180, a CSV download written batch by batch, a stopped download leaving no file,
  a folder that is not there named in the error, an Excel workbook (checked once by unzipping: a frozen bold header,
  DECIMAL as a number, DATE and TIMESTAMP as Excel dates, a 19-digit BIGINT as text), significant digits, sheet names;
  the CSV reader (quotes, separators and line breaks in quotes, BOM, blank lines, an unclosed quote naming its line),
  the separator found, columns matched by name or position, the columns a table needs, NULL for empty fields, the
  upload's requests and answers; in the helper's self-test the failed row of a batch. In the UI a tab's download
  writing the rows fetched then the rest, Cancel removing the file, a table's download with IBM i headings, a host
  error removing the file, an upload committing and saying how many, a failed row named by its line with SQL7008
  explained, Cancel rolling back, and both dialogs keeping their buttons in place. **Checked in the running app** by
  the owner, Linux under WSL (2026-11-15): downloads to CSV and Excel and an upload into a table. Claude did not run
  them against an IBM i (upload writes, and Claude may only read on the owner's system, D22).
