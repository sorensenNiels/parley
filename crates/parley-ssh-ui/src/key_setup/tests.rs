use super::*;
use parley_ssh_core::net::Stream;

const KEY: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAV5CINKVHRsJdOO4vr5nwVr0ASkuG8DODBufVj2pCQ9";
const OTHER: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAID4h3fHUfGd/+K7rKTBQ1aDn4eeuDmwKgTzydjnS3Mee";

/// A fresh `~/.ssh`, removed when dropped.
struct SshDir(PathBuf);

impl SshDir {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("parley-ui-key-setup-{name}-{}", std::process::id()));
        // Left over from an earlier run that panicked: harmless if it is not there.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("made");
        Self(dir)
    }

    /// A key file `name` with its `.pub` beside it.
    fn key(&self, name: &str, line: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, "not read: the .pub is").expect("written");
        std::fs::write(
            self.0.join(format!("{name}.pub")),
            format!("{line} æøå@test\n"),
        )
        .expect("written");
        path
    }
}

impl Drop for SshDir {
    fn drop(&mut self) {
        // Only the test's own folder; one left behind is harmless.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn opened(dir: &SshDir, key_files: &[PathBuf], saved: Option<&str>) -> KeySetup {
    let (setup, orders) = KeySetup::open(
        7,
        "niels@ibmi".into(),
        saved.map(str::to_owned),
        key_files,
        &dir.0,
        "niels@desk".into(),
    );
    assert!(
        matches!(orders.as_slice(), [Order::Send(Command::AgentKeys)]),
        "asks the agent at once"
    );
    setup
}

/// The command an order runs on a side channel, with its job.
fn exec_of(orders: &[Order]) -> (JobId, String) {
    match orders {
        [Order::Send(Command::Exec(exec))] => (exec.id, exec.command.clone()),
        other => panic!("one command, not {other:?}"),
    }
}

fn answer(setup: &mut KeySetup, job: JobId, output: &str, status: u32) -> Vec<Order> {
    setup.feed(Event::JobOutput {
        id: job,
        stream: Stream::Out,
        data: output.as_bytes().to_vec(),
    });
    setup.feed(Event::JobEnded {
        id: job,
        ending: JobEnding::Exited(status),
    })
}

fn line(text: &str) -> PublicLine {
    PublicLine::parse(text).expect("a key line")
}

#[test]
fn the_session_s_key_comes_first_then_the_agent_s_then_the_usual_ones_r_ssh_67() {
    let dir = SshDir::new("order");
    let own = dir.key("work", KEY);
    let usual = dir.key("id_ed25519", OTHER);
    let mut setup = opened(&dir, std::slice::from_ref(&own), Some("IBM i"));
    assert!(matches!(&setup.choices[0], Choice::File(p, Ok(_)) if *p == own));
    assert!(matches!(&setup.choices[1], Choice::File(p, Ok(_)) if *p == usual));
    assert_eq!(setup.choices[2], Choice::New);
    assert_eq!(setup.chosen, 0);

    setup.feed(Event::AgentKeys(Ok(vec![line(&format!("{OTHER} agent"))])));
    assert!(
        matches!(setup.choices[1], Choice::Agent(_)),
        "after the session's own"
    );
    assert_eq!(setup.chosen, 0, "the session's own key stays chosen");
    assert_eq!(setup.agent, AgentState::Listed);
}

#[test]
fn without_a_key_of_its_own_the_agent_s_first_key_is_chosen_unless_the_user_chose_r_ssh_67() {
    let dir = SshDir::new("agent");
    dir.key("id_rsa", OTHER);
    let mut setup = opened(&dir, &[], None);
    assert_eq!(setup.chosen, 0, "the usual key, until the agent answers");
    setup.feed(Event::AgentKeys(Ok(vec![line(KEY)])));
    assert_eq!(setup.chosen, 0);
    assert!(matches!(setup.choices[0], Choice::Agent(_)));

    let mut touched = opened(&dir, &[], None);
    touched.choose(1);
    touched.feed(Event::AgentKeys(Ok(vec![line(KEY)])));
    assert_eq!(
        touched.choices[touched.chosen],
        Choice::New,
        "still the user's choice"
    );

    let mut none = opened(&dir, &[], None);
    none.feed(Event::AgentKeys(Err("No agent is running.".into())));
    assert_eq!(
        none.agent,
        AgentState::Unavailable("No agent is running.".into())
    );
}

#[test]
fn set_up_checks_adds_and_remembers_the_key_in_the_saved_session_r_ssh_67() {
    let dir = SshDir::new("flow");
    let own = dir.key("work", KEY);
    let mut setup = opened(&dir, std::slice::from_ref(&own), Some("IBM i"));
    let (check, command) = exec_of(&setup.start());
    assert!(check.0 > KEY_JOBS, "in key setup's own range");
    assert_eq!(command, setup::check_command());

    let orders = answer(&mut setup, check, "OS400\nparley:open g /home/NIELS\n", 0);
    let (add, command) = exec_of(&orders);
    assert!(command.contains("authorized_keys") && command.contains(KEY));

    let orders = answer(&mut setup, add, "parley:added\n", 0);
    match orders.as_slice() {
        [
            Order::Remember {
                session,
                key: Choice::File(path, _),
            },
        ] => {
            assert_eq!((session.as_str(), path), ("IBM i", &own));
        }
        other => panic!("remembered, not {other:?}"),
    }
    let Stage::Done { added, to_fix, fix } = &setup.stage else {
        panic!("done")
    };
    assert_eq!(
        (*added, to_fix.len(), fix),
        (Added::Added, 1, &Fix::Offered)
    );

    // IBM i's folder: asked first, then fixed.
    assert!(setup.fix().is_empty(), "not before asking");
    setup.ask_fix();
    let (fix_job, command) = exec_of(&setup.fix());
    assert_eq!(command, "chmod go-w -- '/home/NIELS'");
    answer(&mut setup, fix_job, "", 0);
    assert!(matches!(
        &setup.stage,
        Stage::Done {
            fix: Fix::Fixed,
            ..
        }
    ));
}

#[test]
fn a_failure_is_said_and_an_unsaved_session_remembers_nothing_r_ssh_67() {
    let dir = SshDir::new("fail");
    let own = dir.key("work", KEY);
    let mut setup = opened(&dir, &[own], None);
    let (check, _) = exec_of(&setup.start());
    let (add, _) = exec_of(&answer(&mut setup, check, "Linux\n", 0));
    let orders = answer(
        &mut setup,
        add,
        "mkdir: /home/æøå/.ssh: Permission denied\n",
        3,
    );
    assert!(orders.is_empty());
    assert!(
        matches!(&setup.stage, Stage::Failed(why) if why.contains("Permission denied")),
        "{:?}",
        setup.stage
    );

    let mut again = opened(&dir, &[dir.0.join("work")], None);
    let (check, _) = exec_of(&again.start());
    let (add, _) = exec_of(&answer(&mut again, check, "Linux\n", 0));
    assert!(
        answer(&mut again, add, "parley:already\n", 0).is_empty(),
        "not saved: nothing to remember"
    );
    assert!(
        matches!(&again.stage, Stage::Done { added: Added::AlreadyThere, to_fix, .. } if to_fix.is_empty())
    );
}

#[test]
fn a_new_key_is_made_with_its_passphrase_typed_twice_then_set_up_r_ssh_68() {
    let dir = SshDir::new("make");
    let mut setup = opened(&dir, &[], Some("Linux"));
    assert_eq!(setup.choices, [Choice::New]);
    assert!(setup.start().is_empty(), "first the passphrase");
    let Stage::Make {
        passphrase, again, ..
    } = &mut setup.stage
    else {
        panic!("making")
    };
    passphrase.text_mut().push_str("æøå");
    again.text_mut().push_str("aeoa");
    assert!(setup.make().is_empty());
    assert!(
        matches!(&setup.stage, Stage::Make { error: Some(e), .. } if e.contains("not the same"))
    );

    let Stage::Make { again, .. } = &mut setup.stage else {
        panic!("making")
    };
    again.text_mut().clear();
    again.text_mut().push_str("æøå");
    let (_, command) = exec_of(&setup.make());
    assert_eq!(command, setup::check_command());
    assert!(dir.0.join("id_ed25519").is_file() && dir.0.join("id_ed25519.pub").is_file());
    assert!(
        matches!(&setup.choices[0], Choice::File(p, Ok(l)) if *p == dir.0.join("id_ed25519") && l.line.ends_with("niels@desk"))
    );
}

#[test]
fn closing_while_the_server_works_ends_the_job_r_ssh_67() {
    let dir = SshDir::new("close");
    let own = dir.key("work", KEY);
    let mut setup = opened(&dir, &[own], None);
    let (check, _) = exec_of(&setup.start());
    match setup.close().as_slice() {
        [Order::Send(Command::EndJob(job)), Order::Close] => assert_eq!(*job, check),
        other => panic!("ended and closed, not {other:?}"),
    }
}

#[test]
fn a_key_is_remembered_first_among_the_files_or_as_the_agent_key_r_ssh_67() {
    let mut session = crate::sessions::SavedSession {
        key_files: vec!["/k/old".into(), "/k/new".into()],
        ..Default::default()
    };
    remember(&mut session, &Choice::File("/k/new".into(), Ok(line(KEY))));
    assert_eq!(session.key_files, ["/k/new", "/k/old"]);
    remember(&mut session, &Choice::Agent(line(KEY)));
    assert!(session.use_agent);
    assert!(session.agent_key.starts_with("SHA256:"));
}

#[test]
fn only_key_setup_s_jobs_and_the_agent_s_keys_go_to_it_r_ssh_67() {
    let ended = |id| Event::JobEnded {
        id: JobId(id),
        ending: JobEnding::Exited(0),
    };
    assert!(is_key_event(&ended(KEY_JOBS + 1)));
    assert!(!is_key_event(&ended(1)), "the Transfer panel's");
    assert!(is_key_event(&Event::AgentKeys(Ok(Vec::new()))));
    assert!(!is_key_event(&Event::Log("x".into())));
}

mod dialog {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    type State = (KeySetup, f32, Vec<Order>);

    fn harness(setup: KeySetup) -> Harness<'static, State> {
        Harness::builder()
            .with_size(egui::vec2(900.0, 600.0))
            .build_ui_state(
                |ui, (setup, height, orders): &mut State| {
                    parley_ui_egui::test_support::install_fonts(ui);
                    orders.extend(setup.show(ui.ctx(), height));
                },
                (setup, 0.0, Vec::new()),
            )
    }

    #[test]
    fn the_dialog_keeps_its_buttons_in_place_and_set_up_runs_the_check_r_ssh_67() {
        let dir = SshDir::new("dialog");
        let own = dir.key("work", KEY);
        let mut harness = harness(opened(&dir, &[own], Some("IBM i")));
        harness.run();
        let first = harness.get_by_label("Set up").rect();
        harness.run_steps(3);
        assert_eq!(
            harness.get_by_label("Set up").rect(),
            first,
            "the button stays put"
        );
        harness.get_by_label_contains("Make a new key").click();
        harness.run();
        harness.get_by_label("Next").click();
        harness.run();
        assert!(matches!(harness.state().0.stage, Stage::Make { .. }));
        harness.get_by_label("Cancel").click();
        harness.run();
        harness.get_by_label_contains("Key file").click();
        harness.run();
        harness.get_by_label("Set up").click();
        // The spinner repaints for ever.
        harness.run_steps(2);
        let (_, command) = exec_of(&harness.state().2);
        assert_eq!(command, setup::check_command());
        harness.get_by_label_contains("Adding the key on niels@ibmi");
    }

    #[test]
    fn the_offer_bar_says_set_up_or_not_for_this_session_r_ssh_67() {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(700.0, 40.0))
            .build_ui_state(
                |ui, asked: &mut Vec<Offer>| {
                    parley_ui_egui::test_support::install_fonts(ui);
                    asked.extend(offer_bar(ui));
                },
                Vec::new(),
            );
        harness.run();
        harness.get_by_label("Set up…").click();
        harness.run();
        harness.get_by_label("Not for this session").click();
        harness.run();
        assert_eq!(harness.state(), &[Offer::SetUp, Offer::NotForThisSession]);
    }
}
