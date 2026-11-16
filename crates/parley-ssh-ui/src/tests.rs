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
    assert!(h.state().active_session().is_none(), "nothing is opened");
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

/// A session that stays connecting for the test: an address that does not answer.
fn waiting() -> SavedSession {
    SavedSession {
        name: "Wait".into(),
        host: "10.255.255.1".into(),
        user: "me".into(),
        ..SavedSession::default()
    }
}

fn press(h: &mut Harness<'static, ParleySshUi>, key: egui::Key, modifiers: egui::Modifiers) {
    h.event(egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    });
    h.run_steps(2);
}

const CTRL_SHIFT: egui::Modifiers = egui::Modifiers {
    ctrl: true,
    shift: true,
    command: true,
    ..egui::Modifiers::NONE
};
const ALT_SHIFT: egui::Modifiers = egui::Modifiers {
    alt: true,
    shift: true,
    ..egui::Modifiers::NONE
};
const ALT: egui::Modifiers = egui::Modifiers {
    alt: true,
    ..egui::Modifiers::NONE
};

#[test]
fn new_tabs_show_the_start_screen_and_the_bar_shows_from_two_r_ssh_39() {
    let mut h = app(Settings::default());
    h.run_steps(2);
    assert!(
        h.query_by_label("New tab").is_none(),
        "no tab bar for one tab"
    );
    press(&mut h, egui::Key::T, CTRL_SHIFT);
    assert_eq!((h.state().tabs.len(), h.state().current), (2, 1));
    assert_eq!(
        h.get_all_by_label("Close tab").count(),
        2,
        "the bar, a close button per tab"
    );
    press(
        &mut h,
        egui::Key::Tab,
        egui::Modifiers {
            ctrl: true,
            command: true,
            ..egui::Modifiers::NONE
        },
    );
    assert_eq!(h.state().current, 0, "Ctrl+Tab wraps to the first");
    press(
        &mut h,
        egui::Key::Num2,
        egui::Modifiers {
            ctrl: true,
            alt: true,
            command: true,
            ..egui::Modifiers::NONE
        },
    );
    assert_eq!(h.state().current, 1);
    press(&mut h, egui::Key::W, CTRL_SHIFT);
    assert_eq!(
        h.state().tabs.len(),
        1,
        "a start screen closes without asking"
    );
    press(&mut h, egui::Key::W, CTRL_SHIFT);
    assert_eq!(
        h.state().tabs.len(),
        1,
        "closing the last tab leaves a start screen"
    );
}

#[test]
fn panes_split_move_and_close_with_the_keys_r_ssh_40() {
    let mut h = app(Settings::default());
    h.run_steps(2);
    press(&mut h, egui::Key::Equals, ALT_SHIFT);
    press(&mut h, egui::Key::Minus, ALT_SHIFT);
    let tab = &h.state().tabs[0];
    assert_eq!(tab.panes.len(), 3);
    let newest = tab.active;
    press(&mut h, egui::Key::ArrowLeft, ALT);
    assert_ne!(
        h.state().tabs[0].active,
        newest,
        "Alt+Left moved to the pane on the left"
    );
    press(&mut h, egui::Key::W, CTRL_SHIFT);
    assert_eq!(
        h.state().tabs[0].panes.len(),
        2,
        "Close takes the pane in front of a split tab"
    );
}

#[test]
fn closing_a_connected_tab_asks_first_r_ssh_39() {
    let mut h = app(Settings::default());
    h.run_steps(1);
    let ctx = h.ctx.clone();
    let pane = h.state().tabs[0].active;
    h.state_mut().connect_pane(&ctx, pane, waiting(), true);
    h.run_steps(2);
    press(&mut h, egui::Key::W, CTRL_SHIFT);
    assert_eq!(h.state().closing, Some(Closing::Tab(0)));
    h.get_by_label("Cancel").click();
    h.run_steps(2);
    assert_eq!(
        (h.state().closing, h.state().tabs[0].panes.len()),
        (None, 1),
        "kept"
    );
    assert!(h.state().active_session().is_some());
}

#[test]
fn duplicate_opens_the_session_again_in_a_tab_and_new_window_asks_the_launcher_r_ssh_41() {
    let asked: Arc<Mutex<Vec<Vec<String>>>> = Arc::default();
    let mut app = ParleySshUi::new(Settings::default(), known());
    let record = Arc::clone(&asked);
    app.set_spawner(Arc::new(move |args: &[String]| {
        record.lock().expect("not poisoned").push(args.to_vec());
        Ok(())
    }));
    let ctx = egui::Context::default();
    app.carry_out(&ctx, Action::NewWindow);
    assert_eq!(*asked.lock().expect("not poisoned"), [Vec::<String>::new()]);
    let pane = app.tabs[0].active;
    app.connect_pane(&ctx, pane, waiting(), true);
    app.carry_out(&ctx, Action::Duplicate);
    assert_eq!((app.tabs.len(), app.current), (2, 1));
    assert_eq!(
        app.active_session().map(|s| s.source.name.as_str()),
        Some("Wait")
    );
}

#[test]
fn without_a_spawner_a_new_window_says_why_not() {
    let mut app = ParleySshUi::new(Settings::default(), known());
    app.carry_out(&egui::Context::default(), Action::NewWindow);
    assert!(app.note.is_some());
}
