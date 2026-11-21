# Parley SSH features

> Checks of Parley SSH requirements still in progress, by requirement ID. The requirements are in
> [ROADMAP.md](ROADMAP.md). Add an entry only when the requirement is fully met, has automated tests where practical,
> and has been checked in the running app on at least one OS; note which OSes were checked. Then move the
> requirement's row from the roadmap, and this entry, to its phase's file in [phases/](phases/), with a row in
> [REQUIREMENTS.md](REQUIREMENTS.md).

## In progress

- **R-SSH-56 (the rest), Phase 31 step 3** (2026-11-21, Linux under WSL): Copy… now runs a dry run first
  (`--dry-run --itemize-changes -8`, with the agent as the copy) and lists what will be added (+), changed (~) and
  deleted (-, in red, with a warning), counted in full with the first 200 names of each; an up-to-date target says
  so and copies nothing; Copy (or "Copy and delete N" as a danger button) runs the copy; Cancel ends the dry run at
  once. The source path's trailing `/` is a choice of "The folder itself" / "What is inside", said in a sentence
  under the paths. Switches: keep permissions, owner and times (`-a`, else `-rl`; on), compress (on), delete extra
  files at the target (off), excludes one pattern a line. Saved transfers by name with their sessions' saved names
  (`Settings::transfers`): chosen from the panel's list, saved and forgotten under it, and run from the palette
  ("Transfer" items), which opens a session not open yet and starts the dry run once both are connected. The panel
  is split into `transfer` (the steps), `transfer/view` and `transfer/window`. **Automated**: `transfer::changes::tests`
  (itemized lines of every kind with `æøå`, a line and a character split between reads, the list's limit),
  `transfer::run::tests` (the dry run's and the switches' command lines), `transfer::tests` (dry run then confirm;
  the switches in both commands; an up-to-date target; a failed dry run; cancel in each stage; save, load and old
  saved transfers without switches; the trailing `/` in words), `tests.rs` (a saved transfer in the palette; one
  whose session is no longer saved says so). **Seen** (`--screenshot`, dark, the panel opened by a temporary line:
  the form, switches and Save; the dry run's list only in tests). **To check on the owner's servers**: the dry
  run's list for a real folder with a change, an addition and (with Delete on) a deletion; an up-to-date target;
  excludes; saving a transfer and running it from the palette with one session closed.

- **R-SSH-53, R-SSH-56 (first part), Phase 31 step 2** (2026-11-20, Linux under WSL): server to server, direct,
  from the Transfer panel (Session ▸ Transfer files…, the palette, Ctrl+Shift+Y / Cmd+Y): a source and a target
  session with a path each, Copy, progress, Cancel, and the outcome in plain words. Copy checks rsync on both ends,
  then runs `transfer::direct_command` on the source with the agent for that job only and B's host key (from
  `Event::Secured`'s new `host_key`) in a temporary known_hosts file made with `set -C` and removed by a `trap`.
  A target reached through a jump host is refused for now (R-SSH-54). **Automated**: `transfer::run::tests` (the
  exact command with `æøå` and a quote in the paths, IPv6, a missing or odd host key, quoting, progress lines with
  `,` and `.` separators, bytes, exit codes and ssh's messages, a file's `Permission denied` not read as a refused
  sign-on), `tests/net.rs` (the host key line in `Secured`), `parley-ssh-ui`'s `transfer::tests` (check then copy
  with the agent; progress only from the source's job; an old rsync stops it; a failed and a cancelled copy; a jump
  host, the same session twice and no path refused; a lost session), `tests.rs` (Ctrl+Shift+Y opens the panel, Copy
  disabled without sessions, Close). **Seen** (`--screenshot`, dark, the panel opened by a temporary line).
  **To check on the owner's servers**: a copy between two servers with the agent, a cancel and a second Copy that
  goes on, a wrong path, a target without the agent's key; the panel in light mode.

- **R-SSH-57, Phase 31 step 1** (2026-11-20, Linux under WSL): side channels and the check for the servers; the
  desktop's rsync comes with step 5 and the panel that shows the check with step 2. `parley-ssh-core`'s `net/jobs.rs`
  runs commands on exec channels beside the shell (`Command::Exec`, `Command::EndJob`, `Event::JobOutput`,
  `Event::JobEnded`), and lets the server use the agent only while a job that asked for it runs (D58);
  `transfer.rs` finds the server's rsync (on the `PATH`, then `/QOpenSys/pkgs/bin` and `/usr/local/bin`) and reads
  its version. **Automated** (`make test-ssh`): `transfer::tests` (GNU rsync 3.1–3.4, 2.6.9, openrsync, none, a
  shell that is not Bourne's, nonsense), `tests/net.rs` (a job's stdout and stderr and exit status with the shell's
  screen untouched; ending a job; a job asking for an agent that is not running; the agent forwarded during a job
  and refused after it). **To check on the owner's servers**: the check's script through OpenSSH, which runs it the
  same way (an exec channel without a terminal), on a Linux server and the IBM i.
