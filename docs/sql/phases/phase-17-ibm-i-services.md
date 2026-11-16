# Phase 17 — IBM i Services

> Finished phase of Parley SQL. Open work: [../ROADMAP.md](../ROADMAP.md); index of requirements: [../REQUIREMENTS.md](../REQUIREMENTS.md).

## Story

From IDEA-25 (the owner, 2026-11-15), aimed at the signed-on user since Parley SQL has no 5250 session (R-SQL-70): a
"System" node under the IBM i at the root of the side panel's tree (R-SQL-83), with the user's spooled files
(R-SQL-78), message queue (R-SQL-79) and jobs with their job logs (R-SQL-80). The tree is the way in, a list opens as a
tab under the editor, and a spooled file or job log opens in a viewer window of its own; all read-only on the second
connection (D32). Plus more IBM i Services starter snippets (R-SQL-81).

**Status:** Done, verified (2026-11-15, Linux under WSL; the probe on the owner's IBM i 7.3): a System node under the IBM i in the side panel's tree (D32): spooled files (R-SQL-78), the message queue (R-SQL-79), jobs and job logs (R-SQL-80); more starters (R-SQL-81).

## Requirements

| ID | Requirement |
|---|---|
| R-SQL-78 | **Spooled files** (2026-11-15; IDEA-25, D32). Under the IBM i at the root of the side panel's tree (R-SQL-83), a node "System" (closed until opened; whether it is open is kept like a group's) holds "Spooled files (n)", "Message queue" and "Jobs (n)"; their counts are read when System is opened and on Refresh (System's right-click menu, which also lists the three views; so does the IBM i's). Clicking "Spooled files" opens a tab "Spooled files" under the editor, after Results, Messages and History, with a close button: the signed-on user's spooled files (`QSYS2.OUTPUT_QUEUE_ENTRIES`), newest first, in the result grid with its filter and Copy: name, user data, job, pages, status, created. Clicking the node again shows the tab; the tab's Refresh reads the list again. Opening one (double-click or Open) reads its text (`SYSTOOLS.SPOOLED_FILE_DATA`) into a viewer window of its own (an egui viewport, as R-SQL-6's window was): monospace, with a find field, Copy and "Save as…" to a `.txt` file through the file window; several can be open. Nothing changes on the host. |
| R-SQL-79 | **The message queue** (2026-11-15; IDEA-25, D32). "Message queue" under System opens a tab "Message queue" (named so as not to be taken for the statements' Messages tab) as R-SQL-78 does: the messages in the signed-on user's message queue (`QSYS2.MESSAGE_QUEUE_INFO`, queue `QUSRSYS/user`), newest first: time, message ID, type, first-level text and who sent it; selecting one shows its second-level text under the grid. The node and the tab say how many messages are newer than the newest seen in this window ("Message queue (3 new)"). No reply or removal. |
| R-SQL-80 | **Your jobs and their job logs** (2026-11-15; IDEA-25, D32). "Jobs" under System opens a tab "Jobs" as R-SQL-78 does: the signed-on user's active jobs (`QSYS2.ACTIVE_JOB_INFO`): job name, subsystem, type, status, CPU used and function; the SQL window's own two jobs are marked. Opening one shows its job log (`QSYS2.JOBLOG_INFO`) in the viewer window of R-SQL-78, a message a line with its time, ID and text. |
| R-SQL-81 | **More IBM i Services starters** (2026-11-15; IDEA-25, adds to R-SQL-27). About ten more read-only "did you know" snippets: disk use (`QSYS2.SYSDISKSTAT`), system values, job queues, output queues, the largest IFS files, an object's authorities, user profiles whose password expires soon, journals, data areas and subsystems. Each names the IBM i release it needs, and each was run on the owner's IBM i (7.3, read-only as NODEJT400, D22) or is marked as needing a later release. |

## Verification

- **R-SQL-78..81** (2026-11-15): IBM i Services (Phase 17). **Automated** (`make test`): each view's SQL reads
  the signed-on user's own within 1000 entries with a heading per column, names from the host quoted and a spooled file
  number that is not one refused, rows becoming a viewer's text, new messages counted after the newest seen (core);
  the window's own jobs marked, a failed list saying why, a spooled file opening in a viewer with its text, Find going
  round, and from the tree: System's labels with counts, a click opening the list as a tab, Open showing the viewer, the
  tab closing (UI); the 22 starters read and fill. **On the owner's IBM i** (7.3, read-only as NODEJT400, D22):
  `make probe-sql ARGS="--tools NODEJT400"` ran the three lists, a job log, the newest spooled file's text and every
  starter, the output checked by the owner (2026-11-15). **Checked in the running app** by the owner, Linux under WSL
  (2026-11-15).
