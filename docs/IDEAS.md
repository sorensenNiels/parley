# Parley ideas

> Ideas that might become requirements later. Nothing here is planned or promised: the plan is
> [ROADMAP.md](ROADMAP.md). Look an idea up with `grep -n` (`"| IDEA-13 "`) rather than reading the whole file.
> Ideas for SQL (the SQL window, Parley SQL) are in [sql/IDEAS.md](sql/IDEAS.md), for Parley SSH in [ssh/IDEAS.md](ssh/IDEAS.md); all three share one sequence of
> numbers, kept on the last line of this file.

## How an idea moves on

- A new idea gets the next free `IDEA-n` (last line of this file). IDs are never reused or renumbered.
- When an idea is picked for a phase, write it as one or more requirements in the roadmap (§5, next free ID of the
  area) and mark the row here *"→ R-XXX-n"*. Remove the row once its requirements are done; the number stays used.
- An idea that would change a decision in the roadmap ([DECISIONS.md](DECISIONS.md)) says so; the decision is taken before it is built.
- Group ideas by theme, one file per theme in [ideas/](ideas/); within a theme, newest last.

## Themes

- [Protocol gaps (known since Phase 1)](ideas/protocol-gaps.md)
- [Sessions and connection](ideas/sessions-and-connection.md)
- [Working with the screen](ideas/working-with-the-screen.md)
- [Automation](ideas/automation.md)

Next free ID: `IDEA-44`.
