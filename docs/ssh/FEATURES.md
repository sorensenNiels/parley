# Parley SSH features

> Checks of Parley SSH requirements still in progress, by requirement ID. The requirements are in
> [ROADMAP.md](ROADMAP.md). Add an entry only when the requirement is fully met, has automated tests where practical,
> and has been checked in the running app on at least one OS; note which OSes were checked. Then move the
> requirement's row from the roadmap, and this entry, to its phase's file in [phases/](phases/), with a row in
> [REQUIREMENTS.md](REQUIREMENTS.md).

## In progress

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
