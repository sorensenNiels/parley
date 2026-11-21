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
fn what_cannot_be_copied_directly_is_said_at_once_r_ssh_53() {
    let mut p = panel();
    let mut b = end(2, "b");
    b.through_jump = true;
    assert_eq!(p.start(end(1, "a"), b), NONE);
    assert!(matches!(&p.outcome, Some(Outcome::Failed(why)) if why.contains("jump host")));
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
    p.lost(3);
    assert!(p.running(), "another session is of no concern");
    p.lost(2);
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
