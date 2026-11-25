# Ideas: Working with results

> One theme of [../IDEAS.md](../IDEAS.md); the next free number is on the last line of [../../IDEAS.md](../../IDEAS.md).
> Claude's suggestions, 2026-11-25: a result as a place to keep working rather than an output pane. Builds on the grid
> (R-SQL-8, R-SQL-16), a row's values (R-SQL-15), Copy as (R-SQL-38), compare (R-SQL-41) and quick charts (R-SQL-42).

## See what a result holds

| ID | Idea |
|---|---|
| IDEA-44 | **Column profiles in the headings:** each heading shows a small histogram, nulls, distinct values and min/max of the fetched rows; a click opens the whole profile. |
| IDEA-45 | **Cells drawn by what they hold:** a bar inside numeric cells, null drawn so it can't be taken for blank, JSON and XML folded as a tree, long text opened on hover. |
| IDEA-46 | **IBM i's old data shown as meant:** numeric dates (`DEC(8,0)` as `YYYYMMDD`, `CYYMMDD`) shown as dates with the stored value on hover, CHAR's trailing blanks as faint dots, the field's TEXT on every heading. |
| IDEA-47 | **Colour a numeric column** by value with one click (a heat map) to find outliers. |

## Ask the result questions without writing SQL

| ID | Idea |
|---|---|
| IDEA-48 | **A "Group by" bar:** drag a heading onto it and rows fold into groups with count, sum and average, nested if more are dragged; **"Make it SQL"** writes the `GROUP BY` as a statement around the original query and runs it on the host over every row. |
| IDEA-49 | **Filter chips:** a cell's "only this", "not this" or a range on a number becomes a chip above the grid; the chips can be sent to the host as a `WHERE` around the query. |
| IDEA-50 | **Ask the assistant about this result** (D25): a question in words becomes SQL with the result's query as a `WITH`. |
| IDEA-51 | **Sum, average and count of the selected cells** in the status bar, as a spreadsheet does. |

## Move through the database from a result

| ID | Idea |
|---|---|
| IDEA-52 | **Follow foreign keys:** a value in a constrained column opens the row it points to as a card, with a trail back; the other way, "12 orders refer to this customer". For files without constraints, links guessed from column names and TEXT (`CUSNO` → `CUSMST`) that the user confirms and Parley keeps. |
| IDEA-53 | **Act on IBM i values:** a value that looks like a job (`123456/USER/JOB`), an object (`LIB/OBJ`), an IFS path or a spooled file offers its job log, its description, or opening it in Parley (`WRKJOB`) or Parley SSH. Opening the other programs may need a decision on how the three talk (R-P-5, D36). |

## Results over time

| ID | Idea |
|---|---|
| IDEA-54 | **Watch mode:** run a query again every N seconds and mark the cells that changed, with a small history graph per number (`ACTIVE_JOB_INFO`, message queues, lock waits). |
| IDEA-55 | **Changes since the last run:** running a query again marks new, gone and changed rows by itself, without opening compare (R-SQL-41). |

## The shape that fits

| ID | Idea |
|---|---|
| IDEA-56 | **A view chosen by the result's shape:** one value as a big number, one row as a card, parent and child columns as a tree (recursive `WITH`, object dependencies), start and end timestamps as a Gantt chart of jobs, sizes as a treemap. |
| IDEA-57 | **A results notebook:** results kept as cards under the statements that made them, with notes, exported as an HTML or Markdown report. |

## Wide and large results

| ID | Idea |
|---|---|
| IDEA-58 | **Find a column** by name or TEXT with the fuzzy finder in a wide file (200 columns); hide, move and freeze columns. |
| IDEA-59 | **Sort on the host when more rows wait:** a sort today orders only the fetched rows (R-SQL-8); when more are waiting, run the query again with `ORDER BY` instead, or say plainly that only the fetched rows are sorted. |

## Changing data

| ID | Idea |
|---|---|
| IDEA-60 | **Edit cells with a review step:** edits collect as the `UPDATE` statements they make, reviewed, then committed under commitment control; only for tables with a key or by RRN. Would need a decision: nothing in the plan lets the grid change data. |
