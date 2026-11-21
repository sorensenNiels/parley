# Parley SSH features

> Checks of Parley SSH requirements still in progress, by requirement ID. The requirements are in
> [ROADMAP.md](ROADMAP.md). Add an entry only when the requirement is fully met, has automated tests where practical,
> and has been checked in the running app on at least one OS; note which OSes were checked. Then move the
> requirement's row from the roadmap, and this entry, to its phase's file in [phases/](phases/), with a row in
> [REQUIREMENTS.md](REQUIREMENTS.md).

## In progress

- **R-SSH-55 and part of R-SSH-66, Phase 31 step 5** (2026-11-21, Linux under WSL): copies between this computer
  and a server. "This computer" is a choice for either end of the Transfer panel, with File… and Folder… buttons
  (the launcher's file window, `DesktopFiles`). rsync runs here (`transfer/local`) with `-e "parley-ssh --relay
  PORT TOKEN"`; the relay (`net/pipe`, `parley-ssh --relay`) connects back to the window on a loopback port with a
  one-time token (128 random bits, new for the dry run and for the copy, compared in constant time) and the command,
  and the window runs that command on an exec channel of the session's open connection (`Command::Pipe`), so
  nothing signs on again and no agent is needed. Cancel ends the job at the server, so rsync here stops by itself
  and keeps its partial files. Windows paths go to rsync as MSYS2's (`/c/Users/…`). A saved transfer names this
  computer with an empty name. Settings ▸ Transfers names another rsync; otherwise the one that came with Parley SSH
  (beside the program, or in the Mac app's Resources), else the one on the `PATH`. **R-SSH-66 so far**:
  `scripts/build-rsync.sh` builds rsync 3.4.1 from its pinned source (SHA-256 checked; the tarball's signature by
  Andrew Tridgell checked when pinned) with its own zlib and popt, with `rsync-COPYING.txt` (GPLv3) and
  `rsync-SOURCE.txt`; the release workflow and `make package` run it on macOS and the Parley SSH app carries it;
  the Parley SSH `.deb` depends on `rsync`. **Not yet**: rsync for the Windows installer (MSYS2's rsync and its
  DLLs), and the About page naming the bundled rsync with its version. **Automated**: `net::pipe::tests` (tokens; a
  relay's token, command and data both ways; data right after the header kept; no window, a silent relay),
  `tests/net.rs` (a pipe carries a relay's data through an exec channel and back, not to the UI or the screen; ending
  it closes the relay's connection), `transfer::desktop::tests` in the core (the desktop's rsync arguments both ways,
  the relay's words through rsync's `-e` splitting, Windows paths), `transfer::tests::the_desktops_rsync_is_checked…`,
  `ssh_start` (the relay's command line), the panel's `transfer::tests` (upload: the checks, the dry run here, the
  relay's token good once and new for the copy, Cancel at the server; download; no rsync here; no relay port; this
  computer twice), `transfer::local::tests` (output and exit status, a missing program, an ended one), `tests.rs`
  (the Transfers page; this computer's path from the file window). **Run for real** (Linux under WSL): rsync 3.4.1
  through `parley-ssh --relay` with a stand-in for the window running `rsync --server` locally: a dry run, a copy
  up and a copy back, with `æøå` and a quote in the paths and an exclude. Two bugs found that way and fixed: the
  relay's stdout was line-buffered (rsync hung), and the header reader could swallow rsync's first bytes.
  `scripts/build-rsync.sh` built and ran on Linux; not yet run on macOS. **To check**: a copy up and down with a
  real server in the app on Linux, macOS and Windows; the release workflow's macOS jobs (run by hand) and the app's
  bundled rsync.

- **R-SSH-54, the engine part of R-SSH-31, Phase 31 step 4** (2026-11-21, Linux under WSL): server to server
  through Parley SSH. `net/relay.rs`: `Command::OpenDoor` hands out a `Door` (the target connection's handle, now
  kept in an `Arc`, and its SSH port); `Command::Forward` asks the source to listen on `127.0.0.1:0`
  (`tcpip-forward`) and says the port in `Event::Forwarded`; each connection that comes in is carried through the
  door with `direct-tcpip` to `127.0.0.1:<the target session's port>` on the target, and a port Parley did not ask
  for is refused; `Command::Unforward` cancels it. The panel goes through Parley when "Through Parley SSH" is chosen
  over "Straight" (`Route`, saved with a transfer) or the target is reached through a jump host (no longer refused), and offers "Try through
  Parley SSH" when a straight dry run says the source cannot reach the target. rsync on the source then runs as in
  R-SSH-53 against `127.0.0.1:<port>` with the same agent and `HostKeyAlias` rules. The forwarding stops however
  the run ends: done, up to date, failed, cancelled, a session lost, or the panel closed (`Panel::abandon`). A
  failed copy through Parley adds that the target must allow forwarding to its own SSH port. **Automated**:
  `tests/net.rs` (a connection to the forwarded port on one test server meets the other test server's SSH banner
  through Parley, then the forwarding is cancelled; a server that refuses forwarding is said), `transfer::tests`
  (door, forward and the dry run against the forwarded port; a jump host's target goes through by itself; every
  ending stops the forwarding; a refused forwarding; the offer after "Connection timed out"),
  `transfer::run::tests` (`unreachable`). **Seen** (`--screenshot`, dark, the panel opened by a temporary line: the
  Straight / Through Parley SSH choice and its hint). **Known limit**: the target's SSH port is taken as its session's port, so a
  target reached through a port mapping (outside port 2222 to inside 22) cannot be reached this way. **To check on
  the owner's servers**: a copy between two servers that cannot reach each other (or with Through Parley SSH chosen), a target
  behind a jump host, a source with `AllowTcpForwarding no`, and Cancel and Close while copying (the port is closed
  on the source afterwards: `ss -ltn` there).

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
