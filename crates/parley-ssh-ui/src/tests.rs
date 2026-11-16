//! The window as a whole: the start screen, saved sessions (R-SSH-11), the command line's start (R-SSH-14), other
//! windows (R-SSH-15) and the View menu.

use std::sync::Mutex;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::*;

fn known() -> KnownHosts {
    KnownHosts::new(
        std::env::temp_dir()
            .join("parley-ssh-app-test")
            .join("known_hosts"),
        None,
    )
}

fn app(settings: Settings) -> Harness<'static, ParleySshUi> {
    Harness::builder()
        .with_size(egui::vec2(900.0, 700.0))
        .build_ui_state(
            |ui, app: &mut ParleySshUi| app.ui(ui),
            ParleySshUi::new(settings, known()),
        )
}

/// Types `text` into the dialog's field `label`, drawn over the start screen's fields of the same names: a click
/// gives it the keyboard first.
fn type_in_dialog(h: &mut Harness<'static, ParleySshUi>, label: &str, text: &str) {
    let at = h
        .get_all_by_label(label)
        .last()
        .expect("the dialog's field")
        .rect()
        .center();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run_steps(2);
    h.event(egui::Event::Text(text.to_owned()));
    h.run_steps(2);
}

fn prod() -> SavedSession {
    SavedSession {
        name: "Prod æøå".into(),
        host: "box.lan".into(),
        port: 2222,
        user: "me".into(),
        ..SavedSession::default()
    }
}

#[test]
fn quick_connect_keeps_the_last_host_and_says_what_is_missing_r_ssh_2() {
    let settings = Settings {
        last_host: "box.lan".into(),
        last_port: 2222,
        ..Settings::default()
    };
    let mut h = app(settings);
    h.run_steps(3);
    assert_eq!(h.get_by_label("Host").value().as_deref(), Some("box.lan"));
    assert_eq!(h.get_by_label("Port").value().as_deref(), Some("2222"));
    h.get_by_label("Connect").click();
    h.run_steps(3);
    assert!(
        h.query_by_label("Fill in the user name to sign on as.")
            .is_some()
    );
    assert!(h.state().session.is_none(), "nothing is opened");
    assert_eq!(h.state().title(), "Parley SSH");
}

#[test]
fn the_header_has_parley_sshs_menus() {
    let mut h = app(Settings::default());
    h.run_steps(2);
    for menu in ["Session", "Edit", "View", "Help"] {
        assert!(h.query_by_label(menu).is_some(), "{menu}");
    }
    assert!(h.query_by_label("Parley SSH").is_some());
}

#[test]
fn view_changes_the_text_size_within_its_limits() {
    let mut app = ParleySshUi::new(Settings::default(), known());
    let ctx = egui::Context::default();
    app.carry_out(&ctx, Action::TextLarger);
    assert_eq!(app.settings.text_size, 110);
    app.settings.text_size = TEXT_SIZES.1;
    app.carry_out(&ctx, Action::TextLarger);
    assert_eq!(app.settings.text_size, TEXT_SIZES.1);
    app.carry_out(&ctx, Action::TextActualSize);
    assert_eq!(app.settings.text_size, 100);
}

#[test]
fn a_new_session_is_saved_from_the_editor_and_listed_r_ssh_11() {
    let mut h = app(Settings::default());
    h.run_steps(2);
    h.get_by_label("New session…").click();
    h.run_steps(2);
    type_in_dialog(&mut h, "Name", "Web æøå");
    type_in_dialog(&mut h, "Host", "web.lan");
    type_in_dialog(&mut h, "User name", "me");
    h.run_steps(2);
    h.get_by_label("Save").click();
    h.run_steps(3);
    let saved = &h.state().settings.sessions;
    assert_eq!(saved.len(), 1);
    assert_eq!(
        (
            saved[0].name.as_str(),
            saved[0].host.as_str(),
            saved[0].port
        ),
        ("Web æøå", "web.lan", 22)
    );
    assert!(
        h.query_by_label("Web æøå").is_some(),
        "listed on the start screen"
    );
}

#[test]
fn edit_and_delete_change_the_saved_sessions_r_ssh_11() {
    let mut h = app(Settings {
        sessions: vec![prod()],
        ..Settings::default()
    });
    h.run_steps(2);
    h.get_by_label("Edit…").click();
    h.run_steps(2);
    h.get_by_label("Delete").click();
    h.run_steps(2);
    h.get_by_label("Delete").click();
    h.run_steps(2);
    assert_eq!(
        h.state().settings.sessions,
        [] as [sessions::SavedSession; 0]
    );
}

#[test]
fn the_defaults_are_edited_as_default_settings_r_ssh_11() {
    let mut h = app(Settings::default());
    h.run_steps(2);
    h.get_by_label("Default Settings…").click();
    h.run_steps(2);
    type_in_dialog(&mut h, "User name", "deploy");
    h.run_steps(2);
    h.get_by_label("Save").click();
    h.run_steps(2);
    assert_eq!(h.state().settings.defaults.user, "deploy");
    assert_eq!(
        h.state().settings.defaults.name,
        "",
        "the defaults have no name of their own"
    );
}

#[test]
fn the_command_line_names_a_saved_session_or_a_host_r_ssh_14() {
    let settings = Settings {
        sessions: vec![prod()],
        defaults: SavedSession {
            user: "deploy".into(),
            key_files: vec!["~/.ssh/k".into()],
            ..SavedSession::default()
        },
        ..Settings::default()
    };
    let app = ParleySshUi::new(settings, known());
    let load = StartRequest {
        load: Some("Prod æøå".into()),
        agent_forwarding: Some(true),
        ..StartRequest::default()
    };
    let (source, saved) = app.source_for(load).expect("found").expect("named");
    assert!(saved);
    assert_eq!(
        (source.host.as_str(), source.agent_forwarding),
        ("box.lan", true)
    );

    let host = StartRequest {
        host: Some("web:2200".into()),
        key_files: vec!["/k2".into()],
        ..StartRequest::default()
    };
    let (source, saved) = app.source_for(host).expect("valid").expect("named");
    assert!(!saved);
    assert_eq!(
        (source.user.as_str(), source.port),
        ("deploy", 2200),
        "the defaults' user"
    );
    assert_eq!(source.key_files, ["~/.ssh/k", "/k2"]);

    let missing = StartRequest {
        load: Some("Nope".into()),
        ..StartRequest::default()
    };
    assert_eq!(
        app.source_for(missing),
        Err("There is no saved session called \"Nope\".".to_owned())
    );
    assert_eq!(app.source_for(StartRequest::default()), Ok(None));
}

#[test]
fn new_and_duplicate_open_other_windows_with_the_session_r_ssh_15() {
    let asked: Arc<Mutex<Vec<Vec<String>>>> = Arc::default();
    let mut app = ParleySshUi::new(
        Settings {
            sessions: vec![prod()],
            ..Settings::default()
        },
        known(),
    );
    let record = Arc::clone(&asked);
    app.set_spawner(Arc::new(move |args: &[String]| {
        record.lock().expect("not poisoned").push(args.to_vec());
        Ok(())
    }));
    let ctx = egui::Context::default();
    app.carry_out(&ctx, Action::NewSession);
    app.open(&ctx, prod(), true);
    app.carry_out(&ctx, Action::Duplicate);
    app.session = None;
    let asked = asked.lock().expect("not poisoned").clone();
    assert_eq!(
        asked,
        [
            Vec::<String>::new(),
            vec!["-load".to_owned(), "Prod æøå".to_owned()]
        ]
    );
}

#[test]
fn without_a_spawner_new_session_says_why_not() {
    let mut app = ParleySshUi::new(Settings::default(), known());
    app.carry_out(&egui::Context::default(), Action::NewSession);
    assert!(app.note.is_some());
}
