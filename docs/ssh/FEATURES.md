# Parley SSH features

> Checks of Parley SSH requirements still in progress, by requirement ID. The requirements are in
> [ROADMAP.md](ROADMAP.md). Add an entry only when the requirement is fully met, has automated tests where practical,
> and has been checked in the running app on at least one OS; note which OSes were checked. Then move the
> requirement's row from the roadmap, and this entry, to its phase's file in [phases/](phases/), with a row in
> [REQUIREMENTS.md](REQUIREMENTS.md).

## In progress

- **R-SSH-67, R-SSH-68, Phase 32 step 1** (2026-11-21, Linux under WSL; D63): the engine. `Event::Started` says how
  the user signed on (`SignedOnWith`: none, agent, key file, keyboard-interactive, password); `Command::AgentKeys`
  lists the running agent's keys; `key_setup` builds and reads the commands that add a key to `authorized_keys`,
  tell IBM i and find folders the group or others can write, and the Fix; `key_setup::make_key` makes an Ed25519 key
  (`ssh-key` at russh's version, with its `getrandom` feature). **Automated** (`make test`): `key_setup.rs` (a key
  line parsed with "æøå" in its comment, one line only; the answers added, already there and each failure with the
  server's words; IBM i's open folders read and fixed one by one, a quote in a path; with a real `sh` in a temporary
  home: a key added once with `~/.ssh` 700 and the file 600, the same key with another comment found, a file without
  its last line end, an existing folder left as it is, the check finding a group- and an others-writable folder and
  the Fix closing them), `key_setup/make.rs` (a key read back with its passphrase "hemmeligt æøå" and not with a
  wrong one, 600 and 700, its public line read back; never over an existing `.pub`, numbered names, no passphrase),
  `tests/net.rs` (every sign-on test now says how it signed on; the agent's keys listed; no agent said).

- **R-SSH-67, R-SSH-68, Phase 32 step 2** (2026-11-21, Linux under WSL; D63): the window. After a password or
  keyboard-interactive sign-on a bar above the terminal offers "Sign on with a key next time? Set up… · Not for this
  session" (kept in the saved session as `key_offer_off`); Session ▸ Set up key sign-on… and the palette open the same
  dialog (`key_setup`): the session's key files, the agent's keys, the usual `~/.ssh` keys and Make a new key; Make
  asks a comment and the passphrase twice; then the check and the key added over side channels in key setup's own job
  range; the result, IBM i's folders with Fix… asked first; the saved session (and the live one, for a restart) gets
  the key file first in its list or the agent key as its only one. **Automated** (`make test`): `key_setup/tests.rs`
  (the order of the keys and which is chosen as the agent answers, also after the user chose; the whole flow from
  check to remembered, with IBM i's folder asked and fixed; a failure in the server's words; an unsaved session
  remembers nothing; a key made with the passphrase typed twice, a mismatch said; closing ends a running job;
  remembering a file or an agent key; which events are key setup's; kittest: the dialog's buttons stay in place, Make
  and back, Set up runs the check; the offer bar's two answers), `session.rs` (offered after a password, not after a
  key or when the session said not; key setup's events kept apart from the Transfer panel's), `tests.rs` (Not for this
  session and the key set up kept in the saved session and the live one). **To check in the running app** (step 3):
  from a password sign-on to a key sign-on on the next connection on a Linux server and on IBM i, with a home folder
  the group can write fixed; a new key made; an agent key; light and dark.

- **R-SSH-55 and part of R-SSH-66, Phase 31 step 5** (2026-11-21, Linux under WSL): copies between this computer
  and a server. "This computer" is a choice for either end of the Transfer panel, with File… and Folder… buttons
  (the launcher's file window, `DesktopFiles`). rsync runs here (`transfer/local`) with `-e "parley-ssh --relay
  PORT TOKEN"`; the relay (`net/pipe`, `parley-ssh --relay`) connects back to the window on a loopback port with a
  one-time token (128 random bits, new for the dry run and for the copy, compared in constant time) and the command,
  and the window runs that command on an exec channel of the session's open connection (`Command::Pipe`), so
  nothing signs on again and no agent is needed. Cancel ends the job at the server, so rsync here stops by itself
  and keeps its partial files. On Windows rsync gets no drive letter (see R-SSH-66 below). A saved transfer names this
  computer with an empty name. Settings ▸ Transfers names another rsync; otherwise the one that came with Parley SSH
  (beside the program, or in the Mac app's Resources), else the one on the `PATH`. **R-SSH-66 so far**:
  `scripts/build-rsync.sh` builds rsync 3.4.1 from its pinned source (SHA-256 checked; the tarball's signature by
  Andrew Tridgell checked when pinned) with its own zlib and popt, with `rsync-COPYING.txt` (GPLv3) and
  `rsync-SOURCE.txt`; the release workflow and `make package` run it on macOS and the Parley SSH app carries it;
  the Parley SSH `.deb` depends on `rsync`. **Windows** (2026-11-21): `scripts/fetch-rsync-windows.py` (Python 3.14,
  standard library only) fetches MSYS2's `rsync-3.4.1-1` and the packages of the six DLLs it loads (msys-2.0,
  msys-crypto-3, msys-iconv-2, msys-lz4-1, msys-xxhash-0, msys-zstd-1; their own imports checked to stay within
  these and Windows' DLLs), each against its pinned SHA-256; each package's signature was checked when pinned
  against MSYS2's keyring (key 5F944B02…A0AA7F57, certified by three of its master keys, not revoked). It takes only
  rsync.exe and those DLLs, with every licence (the texts the packages lack are in `crates/parley-app/windows/rsync/`)
  and `rsync-SOURCE.txt` naming each part's licence and MSYS2 source package. The installer template puts them in an
  `rsync` folder of the install folder (component `ParleySshRsync` of the Parley SSH feature, from
  `CARGO_PACKAGER_RSYNC_DIR`), off the PATH the installer sets; Parley SSH looks there first. MSYS2's rsync gets no drive
  letter whatever its mount table (`transfer::windows_place`): it starts in the desktop path's drive with the rest
  as a relative path, and finds `parley-ssh` by name, its folder put first on rsync's PATH (`Launch`). The release
  workflow's Windows job and `make package` on Windows run the script. **About page** (2026-11-21): the shared
  About page takes what a program ships beside itself (`about::Included`, `About::show_with`); Parley SSH lists the
  bundled rsync among the Includes with its version, read from the `rsync-SOURCE.txt` beside it, its licence and the
  folder its licence and sources notice are in, and nothing where none came with it (Linux). The Includes group now
  shows in builds that do not look for updates too, as JTOpen's notice asks to be in plain view. Tested by
  `about::tests` (an included item drawn) and `transfer::window::tests` (the version from the notice, none without
  one); seen in a screenshot with a stand-in bundled rsync, the Includes below the fold. **Automated**: `net::pipe::tests` (tokens; a
  relay's token, command and data both ways; data right after the header kept; no window, a silent relay),
  `tests/net.rs` (a pipe carries a relay's data through an exec channel and back, not to the UI or the screen; ending
  it closes the relay's connection), `transfer::desktop::tests` in the core (the desktop's rsync arguments both ways,
  the relay's words through rsync's `-e` splitting, Windows paths), `transfer::tests::the_desktops_rsync_is_checked…`,
  `ssh_start` (the relay's command line), the panel's `transfer::tests` (upload: the checks, the dry run here, the
  relay's token good once and new for the copy, Cancel at the server; download; no rsync here; no relay port; this
  computer twice; on Windows the drive as the start folder and the relay by name), `transfer::local::tests` (output
  and exit status, the start folder and the PATH, a missing program, an ended one), `transfer::desktop::tests` in the
  core (`windows_place`: drive, UNC, relative, trailing slash), `tests.rs`
  (the Transfers page; this computer's path from the file window). **Run for real** (Linux under WSL): rsync 3.4.1
  through `parley-ssh --relay` with a stand-in for the window running `rsync --server` locally: a dry run, a copy
  up and a copy back, with `æøå` and a quote in the paths and an exclude. Two bugs found that way and fixed: the
  relay's stdout was line-buffered (rsync hung), and the header reader could swallow rsync's first bytes.
  `scripts/build-rsync.sh` built and ran on Linux; not yet run on macOS. `scripts/fetch-rsync-windows.py` ran on
  Linux: the fourteen files, the DLLs' imports read with objdump, the template's component naming the same fourteen;
  rsync.exe not run (no Windows here). **To check**: a copy up and down with a real server in the app on Linux,
  macOS and Windows (on Windows, a path on another drive and one with `æøå`); the release workflow's macOS and Windows
  jobs (run by hand), the installed `rsync` folder, and that `rsync` is not found on the PATH in a new terminal.

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
