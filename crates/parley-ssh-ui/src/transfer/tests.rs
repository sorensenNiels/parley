use super::*;

fn end(pane: PaneId, host: &str) -> End {
    End {
        pane,
        label: format!("niels@{host}"),
        user: "niels".into(),
        host: host.into(),
        port: 22,
        host_key: Some("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAV5".into()),
        through_jump: false,
    }
}

fn panel() -> Panel {
    Panel {
        source: Some(1),
        source_path: "/data/æøå/".into(),
        target: Some(2),
        target_path: "/backup".into(),
        ..Panel::default()
    }
}

fn out(id: JobId, text: &str) -> Event {
    Event::JobOutput {
        id,
        stream: Stream::Out,
        data: text.as_bytes().to_vec(),
    }
}

fn ended(id: JobId, ending: JobEnding) -> Event {
    Event::JobEnded { id, ending }
}

const NONE: [Order; 0] = [];

const RSYNC: &str = "rsync  version 3.2.7  protocol version 31\nparley-rsync: rsync\n";

/// Starts and answers both checks with `a` and `b`; the orders that follow.
fn checked(p: &mut Panel, a: &str, b: &str) -> Vec<Order> {
    let orders = p.start(end(1, "a"), end(2, "b"));
    let [Order::Exec(1, ca), Order::Exec(2, cb)] = orders.as_slice() else {
        panic!("a check on each end: {orders:?}")
    };
    assert_eq!(ca.command, transfer::check_command());
    assert!(!ca.agent && !cb.agent, "no agent for a check");
    let (ia, ib) = (ca.id, cb.id);
    assert_eq!(p.feed(1, out(ia, a)), NONE);
    assert!(
        p.feed(1, ended(ia, JobEnding::Exited(0))).is_empty(),
        "waits for the other"
    );
    p.feed(2, out(ib, b));
    p.feed(2, ended(ib, JobEnding::Exited(0)))
}

/// The dry run's job, once both checks found rsync.
fn dry_run(p: &mut Panel) -> Exec {
    let orders = checked(p, RSYNC, RSYNC);
    let [Order::Exec(1, dry)] = orders.as_slice() else {
        panic!("the dry run on the source: {orders:?}")
    };
    dry.clone()
}

/// Answers the dry run with `listed` and confirms; the copy's job.
fn copy(p: &mut Panel, listed: &str) -> Exec {
    let dry = dry_run(p);
    p.feed(1, out(dry.id, listed));
    p.feed(1, ended(dry.id, JobEnding::Exited(0)));
    let orders = p.confirm();
    let [Order::Exec(1, copy)] = orders.as_slice() else {
        panic!("rsync on the source: {orders:?}")
    };
    copy.clone()
}

const LISTED: &str = "<f+++++++++ ny.pdf\n<f.st...... priser.csv\n";

#[test]
fn copy_runs_a_dry_run_first_and_copies_once_confirmed_r_ssh_56() {
    let mut p = panel();
    let dry = dry_run(&mut p);
    assert!(dry.agent, "the dry run signs on to the target too (D58)");
    assert!(
        dry.command.contains("--dry-run --itemize-changes"),
        "{}",
        dry.command
    );
    assert!(
        dry.command.contains("'/data/æøå/' 'niels@b:/backup'"),
        "{}",
        dry.command
    );
    p.feed(1, out(dry.id, "<f+++++++++ ny.pdf\n*deleting   x\n"));
    p.feed(2, out(dry.id, "<f+++++++++ from the target, not counted\n"));
    assert_eq!(p.confirm(), NONE, "not before the dry run has ended");
    p.feed(1, ended(dry.id, JobEnding::Exited(0)));
    let Some(Run {
        stage: Stage::Review { changes, .. },
        ..
    }) = &p.run
    else {
        panic!("the list to review")
    };
    assert_eq!(changes.added.names, ["ny.pdf"]);
    assert_eq!(changes.deleted.names, ["x"]);

    let orders = p.confirm();
    let [Order::Exec(1, copy)] = orders.as_slice() else {
        panic!("rsync on the source: {orders:?}")
    };
    assert!(copy.agent, "the agent for the copy (D58)");
    assert!(
        copy.command.contains("--partial --info=progress2"),
        "{}",
        copy.command
    );
    assert!(!copy.command.contains("--dry-run"));
}

#[test]
fn the_switches_go_into_both_commands_r_ssh_56() {
    let mut p = panel();
    p.switches = Switches {
        delete: true,
        keep: false,
        compress: false,
        excludes: "*.log\n\ncache/\n".into(),
        route: Route::Straight,
    };
    let dry = dry_run(&mut p);
    assert!(
        dry.command.contains(
            "-rl --dry-run --itemize-changes -8 --delete --exclude='*.log' --exclude='cache/'"
        ),
        "{}",
        dry.command
    );
    let copy = copy(&mut p, LISTED);
    assert!(
        copy.command.contains("--delete --exclude='*.log'"),
        "{}",
        copy.command
    );
}

#[test]
fn an_up_to_date_target_copies_nothing_r_ssh_56() {
    let mut p = panel();
    let dry = dry_run(&mut p);
    p.feed(1, out(dry.id, ".d..t...... ./\n"));
    assert_eq!(p.feed(1, ended(dry.id, JobEnding::Exited(0))), NONE);
    assert!(!p.running());
    assert_eq!(
        p.outcome,
        Some(Outcome::Done(
            "Nothing to copy: niels@b is up to date.".into()
        ))
    );
}

#[test]
fn a_finished_copy_says_how_much_and_only_the_sources_job_counts_r_ssh_53() {
    let mut p = panel();
    let id = copy(&mut p, LISTED).id;
    p.feed(
        1,
        out(
            id,
            "  1,000,000  50%   2.00MB/s    0:00:01 (xfr#1, to-chk=1/2)\r",
        ),
    );
    p.feed(2, out(id, "  9,999,999  99%   2.00MB/s    0:00:01\r"));
    let Some(Run {
        stage: Stage::Copying {
            progress: Some(now),
            ..
        },
        ..
    }) = &p.run
    else {
        panic!("copying with progress")
    };
    assert_eq!(now.percent, 50, "only the source's job counts");
    p.feed(
        1,
        out(
            id,
            "  2,000,000 100%   2.00MB/s    0:00:02 (xfr#2, to-chk=0/2)\n",
        ),
    );
    p.feed(1, ended(id, JobEnding::Exited(0)));
    assert!(!p.running());
    assert_eq!(
        p.outcome,
        Some(Outcome::Done("Copied 2.0 MB in 0:00:02.".into()))
    );
}

#[test]
fn an_old_rsync_stops_before_anything_is_copied_r_ssh_57() {
    let mut p = panel();
    let orders = checked(
        &mut p,
        RSYNC,
        "rsync  version 2.6.9  protocol version 29\nparley-rsync: rsync\n",
    );
    assert_eq!(orders, NONE, "nothing copied");
    assert!(!p.running());
    let Some(Outcome::Failed(why)) = &p.outcome else {
        panic!("failed")
    };
    assert!(why.starts_with("niels@b has rsync 2.6.9"), "{why}");
}

#[test]
fn a_failed_dry_run_is_said_and_copies_nothing_r_ssh_56() {
    let mut p = panel();
    let dry = dry_run(&mut p);
    p.feed(
        1,
        Event::JobOutput {
            id: dry.id,
            stream: Stream::Err,
            data: b"niels@b: Permission denied (publickey).\n".to_vec(),
        },
    );
    p.feed(1, ended(dry.id, JobEnding::Exited(255)));
    assert!(!p.running());
    assert!(matches!(&p.outcome, Some(Outcome::Failed(why)) if why.contains("authorized_keys")));
}

#[test]
fn cancel_ends_the_dry_run_at_once_and_the_copy_once_rsync_has_ended_r_ssh_56() {
    let mut p = panel();
    let dry = dry_run(&mut p);
    assert_eq!(p.cancel(), [Order::End(1, dry.id)]);
    assert!(!p.running(), "a dry run changes nothing: no need to wait");
    assert_eq!(p.outcome, Some(Outcome::Cancelled));

    let dry = dry_run(&mut p);
    p.feed(1, out(dry.id, LISTED));
    p.feed(1, ended(dry.id, JobEnding::Exited(0)));
    assert_eq!(p.cancel(), NONE, "nothing runs while the list is reviewed");
    assert!(!p.running());
    assert_eq!(p.outcome, None);

    let id = copy(&mut p, LISTED).id;
    assert_eq!(p.cancel(), [Order::End(1, id)]);
    assert!(p.running(), "until rsync has ended");
    p.feed(1, ended(id, JobEnding::Exited(20)));
    assert_eq!(p.outcome, Some(Outcome::Cancelled));
}

#[test]
fn what_cannot_be_copied_is_said_at_once_r_ssh_53() {
    let mut p = panel();
    assert!(
        p.start(end(1, "a"), end(1, "a")).is_empty(),
        "the same session twice"
    );
    p.target_path = "  ".into();
    assert_eq!(p.start(end(1, "a"), end(2, "b")), NONE, "no path");
    assert!(!p.running());
}

#[test]
fn a_session_lost_while_copying_ends_the_transfer_r_ssh_56() {
    let mut p = panel();
    checked(&mut p, RSYNC, RSYNC);
    assert_eq!(p.lost(3), NONE);
    assert!(p.running(), "another session is of no concern");
    assert_eq!(p.lost(2), NONE, "nothing forwarded to stop");
    assert!(!p.running());
    assert!(matches!(&p.outcome, Some(Outcome::Failed(why)) if why.contains("niels@b")));
}

#[test]
fn a_transfer_is_saved_and_loaded_with_its_switches_r_ssh_56() {
    let mut p = panel();
    p.name = " Nightly æøå ".into();
    p.switches.delete = true;
    let saved = p.saved("prod".into(), "backup".into());
    assert_eq!(saved.name, "Nightly æøå");
    assert_eq!(
        (saved.source.as_str(), saved.target.as_str()),
        ("prod", "backup")
    );
    let json = serde_json::to_string(&saved).expect("saved");
    let back: SavedTransfer = serde_json::from_str(&json).expect("read");
    assert_eq!(back, saved);
    let old: SavedTransfer = serde_json::from_str(r#"{"name":"x"}"#).expect("read");
    assert!(old.switches.keep && old.switches.compress && !old.switches.delete);

    let mut q = Panel::default();
    q.load(&saved, Some(7), None);
    assert_eq!((q.source, q.target), (Some(7), None));
    assert_eq!(q.source_path, "/data/æøå/");
    assert!(q.switches.delete);
}

#[test]
fn the_trailing_slash_is_said_in_words_r_ssh_56() {
    assert_eq!(
        copies_text("/data/æøå/", "/backup"),
        "Copies what is inside /data/æøå/ into /backup."
    );
    assert_eq!(
        copies_text("/data/æøå", "/backup/"),
        "Copies æøå itself into /backup/, as /backup/æøå."
    );
    assert_eq!(
        copies_text("rapport.pdf", ""),
        "Copies rapport.pdf itself into the target."
    );
}

/// Through Parley SSH up to the dry run: the door asked of the target, passed to the source, the port forwarded
/// there; the forwarding's job and the dry run.
fn through(p: &mut Panel) -> (JobId, Exec) {
    let orders = checked(p, RSYNC, RSYNC);
    let [Order::OpenDoor(2, id)] = orders.as_slice() else {
        panic!("the target's door first: {orders:?}")
    };
    let id = *id;
    assert_eq!(
        p.door(1, id),
        None,
        "a door from the source is not the target's"
    );
    assert_eq!(p.door(2, JobId(999)), None, "another job's door");
    assert_eq!(p.door(2, id), Some(1), "forwarded on the source");
    let forwarded = |port| Event::Forwarded { id, port };
    assert_eq!(
        p.feed(2, forwarded(Ok(1))),
        NONE,
        "only the source forwards"
    );
    let orders = p.feed(1, forwarded(Ok(40_022)));
    let [Order::Exec(1, dry)] = orders.as_slice() else {
        panic!("the dry run: {orders:?}")
    };
    (id, dry.clone())
}

#[test]
fn a_copy_through_parley_reaches_the_target_on_the_sources_forwarded_port_r_ssh_54() {
    let mut p = panel();
    p.switches.route = Route::Through;
    let (relay, dry) = through(&mut p);
    assert!(dry.agent);
    assert!(
        dry.command.contains("ssh -p 40022 ") && dry.command.contains("'niels@127.0.0.1:/backup'"),
        "{}",
        dry.command
    );
    assert!(
        dry.command.contains("HostKeyAlias=parley-target"),
        "B's own key, whatever the address"
    );
    p.feed(1, out(dry.id, LISTED));
    p.feed(1, ended(dry.id, JobEnding::Exited(0)));
    let orders = p.confirm();
    let [Order::Exec(1, copy)] = orders.as_slice() else {
        panic!("copying: {orders:?}")
    };
    assert!(copy.command.contains("ssh -p 40022 "), "{}", copy.command);
    assert_eq!(
        p.feed(1, ended(copy.id, JobEnding::Exited(0))),
        [Order::Unforward(1, relay)],
        "the forwarding stops with the copy"
    );
    assert!(matches!(p.outcome, Some(Outcome::Done(_))));
}

#[test]
fn a_target_behind_a_jump_host_goes_through_parley_by_itself_r_ssh_54() {
    let mut p = panel();
    let mut b = end(2, "b");
    b.through_jump = true;
    let orders = p.start(end(1, "a"), b);
    let [Order::Exec(1, ca), Order::Exec(2, cb)] = orders.as_slice() else {
        panic!("the checks")
    };
    let (ia, ib) = (ca.id, cb.id);
    p.feed(1, out(ia, RSYNC));
    p.feed(1, ended(ia, JobEnding::Exited(0)));
    p.feed(2, out(ib, RSYNC));
    assert!(matches!(
        p.feed(2, ended(ib, JobEnding::Exited(0))).as_slice(),
        [Order::OpenDoor(2, _)]
    ));
}

#[test]
fn every_way_a_run_through_parley_ends_stops_the_forwarding_r_ssh_54() {
    // Cancelled while the list is reviewed.
    let mut p = panel();
    p.switches.route = Route::Through;
    let (relay, dry) = through(&mut p);
    p.feed(1, out(dry.id, LISTED));
    p.feed(1, ended(dry.id, JobEnding::Exited(0)));
    assert_eq!(p.cancel(), [Order::Unforward(1, relay)]);

    // Up to date.
    let (relay, dry) = through(&mut p);
    assert_eq!(
        p.feed(1, ended(dry.id, JobEnding::Exited(0))),
        [Order::Unforward(1, relay)]
    );

    // The target's connection lost while looking.
    let (relay, _) = through(&mut p);
    assert_eq!(p.lost(2), [Order::Unforward(1, relay)]);

    // The panel closed while copying: rsync is ended and the forwarding stopped at once.
    let (relay, dry) = through(&mut p);
    p.feed(1, out(dry.id, LISTED));
    p.feed(1, ended(dry.id, JobEnding::Exited(0)));
    let copy = p.confirm();
    let [Order::Exec(1, copy)] = copy.as_slice() else {
        panic!("copying")
    };
    assert_eq!(
        p.abandon(),
        [Order::End(1, copy.id), Order::Unforward(1, relay)]
    );
    assert!(!p.running());

    // A failed copy says that the target may not carry the connection.
    let (relay, dry) = through(&mut p);
    p.feed(
        1,
        Event::JobOutput {
            id: dry.id,
            stream: Stream::Err,
            data: b"kex_exchange_identification: Connection closed by remote host\n".to_vec(),
        },
    );
    assert_eq!(
        p.feed(1, ended(dry.id, JobEnding::Exited(255))),
        [Order::Unforward(1, relay)]
    );
    assert!(
        matches!(&p.outcome, Some(Outcome::Failed(why)) if why.contains("AllowTcpForwarding")),
        "{:?}",
        p.outcome
    );
    assert!(!p.offer_through, "already through Parley");
}

#[test]
fn a_source_that_cannot_forward_or_reach_the_target_is_said_r_ssh_54() {
    let mut p = panel();
    p.switches.route = Route::Through;
    let orders = checked(&mut p, RSYNC, RSYNC);
    let [Order::OpenDoor(2, id)] = orders.as_slice() else {
        panic!("the door")
    };
    let refused = Event::Forwarded {
        id: *id,
        port: Err("The source server does not allow forwarding".into()),
    };
    assert_eq!(p.feed(1, refused), NONE, "nothing forwarded to stop");
    assert!(matches!(&p.outcome, Some(Outcome::Failed(why)) if why.contains("does not allow")));

    // Straight, a source that cannot reach the target is offered the way through Parley SSH.
    p.switches.route = Route::Straight;
    let dry = dry_run(&mut p);
    p.feed(
        1,
        Event::JobOutput {
            id: dry.id,
            stream: Stream::Err,
            data: b"ssh: connect to host b port 22: Connection timed out\n".to_vec(),
        },
    );
    p.feed(1, ended(dry.id, JobEnding::Exited(255)));
    assert!(p.offer_through);
    assert!(matches!(&p.outcome, Some(Outcome::Failed(why)) if why.contains("cannot reach")));
    p.start(end(1, "a"), end(2, "b"));
    assert!(!p.offer_through, "a new run forgets the offer");
}

fn setup() -> Setup {
    Setup {
        rsync: "/usr/bin/rsync".into(),
        program: "/opt/Parley SSH/parley-ssh".into(),
        port: 40_123,
    }
}

/// Starts a desktop transfer between `source` and `target` and answers both checks; the orders that follow.
fn desktop_checked(p: &mut Panel, source: End, target: End) -> Vec<Order> {
    p.setup = setup();
    let orders = p.start(source, target);
    let mut checks = Vec::new();
    for order in &orders {
        match order {
            Order::Spawn(id, argv) => {
                assert_eq!(
                    argv,
                    &["/usr/bin/rsync", "--version"],
                    "the desktop's own rsync"
                );
                checks.push((DESKTOP, *id, "rsync  version 3.4.1  protocol version 32\n"));
            }
            Order::Exec(pane, exec) => checks.push((*pane, exec.id, RSYNC)),
            other => panic!("a check: {other:?}"),
        }
    }
    assert_eq!(checks.len(), 2, "{orders:?}");
    let mut next = Vec::new();
    for (pane, id, answer) in checks {
        p.feed(pane, out(id, answer));
        next = p.feed(pane, ended(id, JobEnding::Exited(0)));
    }
    next
}

/// The `-e` program's token in `argv`.
fn token_of(argv: &[String]) -> String {
    let at = argv.iter().position(|a| a == "-e").expect("-e");
    argv[at + 1].rsplit(' ').next().expect("a token").to_owned()
}

#[test]
fn a_desktop_upload_runs_rsync_here_with_the_relay_as_its_remote_shell_r_ssh_55() {
    let mut p = panel();
    p.source = Some(DESKTOP);
    let orders = desktop_checked(&mut p, End::desktop(), end(2, "b"));
    let [Order::Spawn(dry, argv)] = orders.as_slice() else {
        panic!("the dry run here: {orders:?}")
    };
    assert_eq!(argv[0], "/usr/bin/rsync");
    assert!(argv.contains(&"--dry-run".to_owned()), "{argv:?}");
    assert!(
        argv.contains(&"--rsync-path='rsync'".to_owned()),
        "the server's rsync: {argv:?}"
    );
    let token = token_of(argv);
    assert_eq!(token.len(), 32);
    let at = argv.iter().position(|a| a == "-e").expect("-e");
    assert_eq!(
        argv[at + 1],
        format!("'/opt/Parley SSH/parley-ssh' --relay 40123 {token}")
    );
    assert_eq!(argv[argv.len() - 2..], ["/data/æøå/", "parley:/backup"]);

    assert_eq!(p.pipe("not the token"), None);
    let Some((2, pipe_job)) = p.pipe(&token) else {
        panic!("the relay's command runs on the server's connection")
    };
    assert_eq!(p.pipe(&token), None, "a token is good once");
    let dry = *dry;
    p.feed(
        2,
        out(dry, "<f+++++++++ from the server's pane, not counted\n"),
    );
    p.feed(DESKTOP, out(dry, LISTED));
    p.feed(DESKTOP, ended(dry, JobEnding::Exited(0)));
    let orders = p.confirm();
    let [Order::Spawn(copy, argv)] = orders.as_slice() else {
        panic!("the copy here: {orders:?}")
    };
    assert!(argv.contains(&"--info=progress2".to_owned()));
    let second = token_of(argv);
    assert_ne!(second, token, "a new token for the copy");
    assert_eq!(p.pipe(&token), None);
    let Some((2, copy_pipe)) = p.pipe(&second) else {
        panic!("the copy's relay")
    };
    assert_ne!(copy_pipe, pipe_job);
    assert_eq!(
        p.cancel(),
        [Order::End(2, copy_pipe)],
        "ended at the server, so rsync here keeps its partial files"
    );
    p.feed(DESKTOP, ended(*copy, JobEnding::Exited(12)));
    assert_eq!(p.outcome, Some(Outcome::Cancelled));
}

#[test]
fn a_desktop_download_takes_the_servers_path_first_r_ssh_55() {
    let mut p = panel();
    p.target = Some(DESKTOP);
    p.target_path = "/home/niels/hentet/".into();
    let orders = desktop_checked(&mut p, end(1, "a"), End::desktop());
    let [Order::Spawn(dry, argv)] = orders.as_slice() else {
        panic!("the dry run here: {orders:?}")
    };
    assert_eq!(
        argv[argv.len() - 2..],
        ["parley:/data/æøå/", "/home/niels/hentet/"]
    );
    // Cancelled before the relay came: rsync here is ended.
    assert_eq!(p.cancel(), [Order::End(DESKTOP, *dry)]);
    assert!(!p.running());
}

#[test]
fn no_rsync_here_or_no_relay_port_is_said_before_anything_is_copied_r_ssh_55() {
    let mut p = panel();
    p.setup = setup();
    let orders = p.start(End::desktop(), end(2, "b"));
    let Some(Order::Spawn(id, _)) = orders.first() else {
        panic!("the desktop's check: {orders:?}")
    };
    let id = *id;
    p.feed(
        DESKTOP,
        ended(
            id,
            JobEnding::Refused("/usr/bin/rsync could not be started".into()),
        ),
    );
    let Some(Order::Exec(2, check)) = orders.get(1) else {
        panic!("the server's check")
    };
    p.feed(2, out(check.id, RSYNC));
    assert_eq!(p.feed(2, ended(check.id, JobEnding::Exited(0))), NONE);
    assert!(
        matches!(&p.outcome, Some(Outcome::Failed(why)) if why.starts_with("this computer has no rsync")),
        "{:?}",
        p.outcome
    );

    p.setup.port = 0;
    assert_eq!(p.start(End::desktop(), end(2, "b")), NONE);
    assert!(matches!(&p.outcome, Some(Outcome::Failed(why)) if why.contains("could not listen")));
    p.setup = setup();
    assert_eq!(
        p.start(End::desktop(), End::desktop()),
        NONE,
        "this computer twice"
    );
}
