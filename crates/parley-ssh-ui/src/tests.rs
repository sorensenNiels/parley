//! The window as a whole: the start screen, saved sessions (R-SSH-11), the command line's start (R-SSH-14), other
//! windows (R-SSH-15) and the View menu.

use std::sync::Mutex;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

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

/// R-SSH-59: a host without a user, as Parley 5250's Tools ▸ SSH… sends it, waits in the quick connect.
#[test]
fn a_host_without_a_user_waits_in_quick_connect_r_ssh_59() {
    let mut h = app(Settings::default());
    h.run_steps(2);
    let request = StartRequest {
        host: Some("ibmi.æøå.lan".into()),
        port: Some(2222),
        ..StartRequest::default()
    };
    let ctx = h.ctx.clone();
    h.state_mut().start(&ctx, request);
    h.run_steps(3);
    assert_eq!(
        h.get_by_label("Host").value().as_deref(),
        Some("ibmi.æøå.lan")
    );
    assert_eq!(h.get_by_label("Port").value().as_deref(), Some("2222"));
    h.get_by_label("Fill in the user name to sign on as.");
    assert!(h.state().active_session().is_none(), "nothing is opened");
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
fn view_sizes_the_screens_text_within_its_limits_r_ssh_20() {
    let mut app = ParleySshUi::new(Settings::default(), known());
    let ctx = egui::Context::default();
    app.carry_out(&ctx, Action::TextLarger);
    assert_eq!(app.settings.font_size, 15);
    app.settings.font_size = 32;
    app.carry_out(&ctx, Action::TextLarger);
    assert_eq!(app.settings.font_size, 32);
    app.settings.font_size = 8;
    app.carry_out(&ctx, Action::TextSmaller);
    assert_eq!(app.settings.font_size, 8);
    app.carry_out(&ctx, Action::TextActualSize);
    assert_eq!(app.settings.font_size, 14);
    assert_eq!(
        app.settings.text_size, 100,
        "the window's own text size stays"
    );
}

#[test]
fn the_appearance_page_chooses_themes_and_text_and_its_buttons_stay_put_r_ssh_20() {
    let mut h = app(Settings::default());
    h.run_steps(2);
    h.state_mut()
        .carry_out(&egui::Context::default(), Action::Settings);
    h.run_steps(3);
    let before = h.get_by_label("Save").rect();
    h.get_by_label("When dark").click();
    h.run_steps(2);
    h.get_by_label("Nord").click();
    h.run_steps(3);
    assert_eq!(
        h.state().settings.dark_theme,
        crate::screen::themes::TerminalTheme::Nord
    );
    // The page scrolls inside the dialog's fixed frame.
    h.get_by_label("Underline").scroll_to_me();
    h.run_steps(3);
    h.get_by_label("Underline").click();
    h.run_steps(3);
    assert_eq!(h.state().settings.cursor, settings::CursorStyle::Underline);
    assert_eq!(
        h.get_by_label("Save").rect(),
        before,
        "the dialog keeps its size"
    );
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
        "listed: All sessions is open with nothing above it (R-SSH-65, D64)"
    );
    // A favourite above it: All sessions folds as the user left it.
    h.state_mut().settings.sessions[0].favourite = true;
    h.run_steps(2);
    assert_eq!(
        h.get_all_by_label("Web æøå").count(),
        1,
        "under Favourites only"
    );
    pointer_click(&mut h, "ALL SESSIONS (1)", 0);
    assert_eq!(h.get_all_by_label("Web æøå").count(), 2);
    assert!(
        h.state().settings.all_sessions_open,
        "kept in the settings for the next start"
    );
    pointer_click(&mut h, "ALL SESSIONS (1)", 0);
    assert!(!h.state().settings.all_sessions_open, "and folded again");
}

#[test]
fn edit_and_delete_change_the_saved_sessions_r_ssh_11() {
    let mut h = app(Settings {
        sessions: vec![prod()],
        all_sessions_open: true,
        ..Settings::default()
    });
    h.run_steps(2);
    h.get_by_label("Edit…").click();
    // The editor's page area learns its height in the first frames (`fitting_scroll_area`).
    h.run_steps(4);
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

/// A session that fails at once: nothing listens on the port any more.
fn refused() -> SavedSession {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("address").port();
    drop(listener);
    SavedSession {
        name: "Refused".into(),
        host: "127.0.0.1".into(),
        port,
        user: "me".into(),
        ..SavedSession::default()
    }
}

#[test]
fn a_split_pane_whose_session_fails_to_start_is_taken_out_r_ssh_58() {
    let mut h = app(Settings::default());
    h.run_steps(1);
    let ctx = h.ctx.clone();
    let first = h.state().tabs[0].active;
    h.state_mut().connect_pane(&ctx, first, refused(), true);
    press(&mut h, egui::Key::Equals, ALT_SHIFT);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    // Both connections fail on their own threads, in either order: wait for both.
    let settled = |app: &ParleySshUi| {
        app.tabs[0].panes.len() == 1 && app.active_session().is_some_and(Session::failed_to_start)
    };
    while !settled(h.state()) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.run_steps(1);
    }
    let tab = &h.state().tabs[0];
    assert_eq!(
        (tab.panes.len(), tab.active),
        (1, first),
        "the failed pane is gone and the first is in front again"
    );
    assert!(
        h.state()
            .note
            .as_deref()
            .is_some_and(|n| n.starts_with("The new pane was closed. Could not connect")),
        "the status bar says why: {:?}",
        h.state().note
    );
    let ended = h.state().active_session().map(Session::failed_to_start);
    assert_eq!(ended, Some(true), "a tab's only pane stays, to show why");
}

#[test]
fn a_waiting_split_pane_stays_and_each_pane_has_a_close_button_r_ssh_58() {
    let mut h = app(Settings::default());
    h.run_steps(1);
    let ctx = h.ctx.clone();
    let first = h.state().tabs[0].active;
    h.state_mut().connect_pane(&ctx, first, waiting(), true);
    h.run_steps(1);
    assert!(
        h.query_by_label("Close pane").is_none(),
        "no pane button without a split"
    );
    press(&mut h, egui::Key::Equals, ALT_SHIFT);
    h.run_steps(2);
    assert_eq!(h.state().tabs[0].panes.len(), 2, "still connecting: kept");
    assert_eq!(h.get_all_by_label("Close pane").count(), 2);
    h.state_mut().settings.confirm_close = false;
    h.get_all_by_label("Close pane")
        .next()
        .expect("the first pane's button")
        .click();
    h.run_steps(2);
    let tab = &h.state().tabs[0];
    assert_eq!(tab.panes.len(), 1, "the button closed its pane");
    assert_ne!(tab.active, first, "the first pane was the one closed");
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

/// A home folder of the test's own whose `~/.ssh/config` holds `text`, read as OpenSSH's config (R-SSH-42).
fn openssh_config(
    name: &str,
    text: &str,
) -> (std::path::PathBuf, parley_ssh_core::openssh_config::Config) {
    let home = std::env::temp_dir().join(format!(
        "parley-ssh-ui-config-{name}-{}",
        std::process::id()
    ));
    let ssh = home.join(".ssh");
    std::fs::create_dir_all(&ssh).expect("the temp folder is writable");
    std::fs::write(ssh.join("config"), text).expect("the temp folder is writable");
    let config = parley_ssh_core::openssh_config::Config::read(&ssh.join("config"), &home);
    (home, config)
}

const CONFIG: &str = "Host web\n  HostName web1.lan\n  User cfg\n  Port 2200\n  IdentityFile ~/.ssh/web\n  \
                      ForwardAgent yes\nHost *.lan\n  User lanuser\nMatch all\n";

#[test]
fn the_command_line_reads_the_config_as_ssh_does_r_ssh_42() {
    let (home, config) = openssh_config("cli", CONFIG);
    let mut app = ParleySshUi::new(Settings::default(), known());
    app.set_openssh_config(config);
    let request = |host: &str| StartRequest {
        host: Some(host.into()),
        ..StartRequest::default()
    };
    let (web, saved) = app
        .source_for(request("web"))
        .expect("valid")
        .expect("named");
    assert!(!saved);
    assert_eq!(
        (
            web.host.as_str(),
            web.user.as_str(),
            web.port,
            web.agent_forwarding
        ),
        ("web1.lan", "cfg", 2200, true)
    );
    assert_eq!(web.key_files, [home.join(".ssh/web").to_string_lossy()]);

    let typed = StartRequest {
        port: Some(22),
        agent_forwarding: Some(false),
        ..request("root@web")
    };
    let (typed, _) = app.source_for(typed).expect("valid").expect("named");
    assert_eq!(
        (typed.user.as_str(), typed.port, typed.agent_forwarding),
        ("root", 22, false),
        "what is typed wins"
    );
    let (lan, _) = app
        .source_for(request("box.LAN"))
        .expect("valid")
        .expect("named");
    assert_eq!(
        lan.user, "lanuser",
        "a pattern applies to a host not listed"
    );
    assert!(
        app.source_for(request("other")).is_err(),
        "no user anywhere: quick connect asks (R-SSH-59)"
    );
}

#[test]
fn config_hosts_are_listed_and_saved_as_sessions_r_ssh_42() {
    let (_home, config) = openssh_config("start", CONFIG);
    let mut app = ParleySshUi::new(Settings::default(), known());
    app.set_openssh_config(config);
    let mut h = Harness::builder()
        .with_size(egui::vec2(900.0, 900.0))
        .build_ui_state(|ui, app: &mut ParleySshUi| app.ui(ui), app);
    h.run_steps(2);
    h.get_by_label("FROM ~/.SSH/CONFIG (1)");
    h.get_by_label("cfg@web1.lan:2200");
    assert!(
        h.query_by_label_contains("Match blocks are not used")
            .is_some(),
        "the skipped line is said"
    );
    h.get_by_label("Save as session…").click();
    h.run_steps(4);
    h.get_by_label("Save").click();
    h.run_steps(3);
    let saved = &h.state().settings.sessions;
    assert_eq!(saved.len(), 1);
    assert_eq!(
        (
            saved[0].name.as_str(),
            saved[0].host.as_str(),
            saved[0].port,
            saved[0].user.as_str()
        ),
        ("web", "web1.lan", 2200, "cfg")
    );
}

/// A real click with the pointer at the centre of the widget `label` (`n`-th of that name): it goes through egui's
/// hit test, as a mouse click does, where kittest's `click()` does not.
fn pointer_click(h: &mut Harness<'static, ParleySshUi>, label: &str, n: usize) {
    let at = h
        .get_all_by_label(label)
        .nth(n)
        .expect("the widget")
        .rect()
        .center();
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(1);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(1);
    }
    h.run_steps(2);
}

#[test]
fn the_close_button_on_a_tab_closes_it_and_asks_first_while_connected_r_ssh_39() {
    let mut h = app(Settings::default());
    h.run_steps(2);
    press(&mut h, egui::Key::T, CTRL_SHIFT);
    pointer_click(&mut h, "Close tab", 0);
    assert_eq!(h.state().tabs.len(), 1, "the start screen's tab closed");

    let ctx = h.ctx.clone();
    h.state_mut().new_tab(&ctx, None);
    let pane = h.state().tabs[1].active;
    h.state_mut().connect_pane(&ctx, pane, waiting(), false);
    h.run_steps(2);
    pointer_click(&mut h, "Close tab", 1);
    assert!(
        h.state().closing.is_some(),
        "a connected tab asks before it closes"
    );
    assert_eq!(h.state().tabs.len(), 2);
}

fn key(h: &mut Harness<'static, ParleySshUi>, key: egui::Key, modifiers: egui::Modifiers) {
    for pressed in [true, false] {
        h.event(egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        });
    }
    h.run_steps(2);
}

#[test]
fn ctrl_shift_b_broadcasts_to_the_tab_and_escape_twice_stops_it_r_ssh_50() {
    let mut h = app(Settings::default());
    h.run_steps(2);
    key(&mut h, egui::Key::B, CTRL_SHIFT);
    assert_eq!(h.state().broadcast_scope(), broadcast::Scope::Tab);
    // Two quick presses (half a second apart here).
    press(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    press(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!(h.state().broadcast_scope(), broadcast::Scope::Off);
    key(&mut h, egui::Key::B, CTRL_SHIFT);
    h.get_by_label("Session").click();
    h.run_steps(2);
    assert!(h.query_by_label("Stop broadcasting").is_some());
    h.get_by_label("Stop broadcasting").click();
    h.run_steps(2);
    assert_eq!(h.state().broadcast_scope(), broadcast::Scope::Off);
    let ctx = egui::Context::default();
    h.state_mut().carry_out(&ctx, Action::BroadcastAll);
    assert_eq!(h.state().broadcast_scope(), broadcast::Scope::All);
    h.state_mut().carry_out(&ctx, Action::BroadcastTab);
    assert_eq!(
        h.state().broadcast_scope(),
        broadcast::Scope::Off,
        "the menu stops it too"
    );
}

fn with_snippets() -> Settings {
    Settings {
        snippets: vec![
            snippets::Snippet {
                name: "uptime".into(),
                text: "uptime".into(),
                enter: true,
                key: "Ctrl+Alt+U".into(),
            },
            snippets::Snippet {
                name: "logs".into(),
                text: "tail -f {{file:/var/log/syslog}}".into(),
                ..snippets::Snippet::default()
            },
        ],
        ..Settings::default()
    }
}

#[test]
fn a_snippets_own_key_sends_it_and_without_a_session_says_to_connect_r_ssh_49() {
    let mut h = app(with_snippets());
    h.run_steps(2);
    let ctrl_alt = egui::Modifiers {
        ctrl: true,
        alt: true,
        command: true,
        ..egui::Modifiers::NONE
    };
    key(&mut h, egui::Key::U, ctrl_alt);
    assert_eq!(
        h.state().note.as_deref(),
        Some("A snippet goes to a connected session: connect first.")
    );
}

#[test]
fn a_snippet_with_placeholders_asks_first_from_the_picker_r_ssh_49() {
    let mut h = app(with_snippets());
    h.run_steps(2);
    key(&mut h, egui::Key::S, CTRL_SHIFT);
    assert!(h.state().picker.is_some(), "Ctrl+Shift+S opens the picker");
    h.get_by_label("Find").type_text("LOG");
    h.run_steps(2);
    assert!(
        h.query_by_label_contains("uptime").is_none(),
        "found by name"
    );
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(h.state().fill.is_some(), "the placeholder is asked for");
    h.get_by_label("file");
    h.get_by_label("Send").click();
    h.run_steps(2);
    assert!(h.state().fill.is_none());
}

#[test]
fn the_snippets_menu_lists_them_and_opens_their_settings_page_r_ssh_49() {
    let mut h = app(with_snippets());
    h.run_steps(2);
    h.get_by_label("Snippets").click();
    h.run_steps(2);
    assert!(h.query_by_label("uptime").is_some());
    h.get_by_label("Edit snippets…").click();
    h.run_steps(3);
    let before = h.get_by_label("Save").rect();
    h.get_by_label("Add snippet").click();
    h.run_steps(3);
    h.get_by_label("Save snippet").scroll_to_me();
    h.run_steps(2);
    h.get_by_label("Save snippet").click();
    h.run_steps(2);
    h.get_by_label("Give the snippet a name.");
    assert_eq!(
        h.get_by_label("Save").rect(),
        before,
        "the dialog keeps its size"
    );
}

#[test]
fn the_tabs_of_last_time_open_again_with_their_split_and_sessions_r_ssh_52() {
    let settings = Settings {
        restore_tabs: true,
        sessions: vec![waiting()],
        ..Settings::default()
    };
    let mut h = app(settings);
    h.run_steps(2);
    let ctx = h.ctx.clone();
    {
        let app = h.state_mut();
        let first = app.tabs[0].active;
        app.connect_pane(&ctx, first, waiting(), true);
        app.split(&ctx, panes::Axis::Row);
        app.new_tab(&ctx, None);
    }
    h.run_steps(2);
    let kept = h.state().settings_to_save();
    assert_eq!(kept.last_tabs.len(), 2);
    assert!(
        h.state().settings().last_tabs.is_empty(),
        "only what is saved holds them"
    );

    let mut again = app(kept.clone());
    again.state_mut().start(&ctx, StartRequest::default());
    again.run_steps(2);
    let app = again.state();
    assert_eq!(app.tabs.len(), 2);
    assert!(app.tabs[0].is_split());
    let sessions = app.tabs[0]
        .panes
        .iter()
        .filter(|(_, p)| p.session().is_some())
        .count();
    assert_eq!(sessions, 2, "both panes connect when the tab is shown");
    assert!(app.tabs[1].shows_start());
    assert_eq!(app.tabs[1].restore, []);

    let mut off = app_with_host(Settings {
        restore_tabs: false,
        ..kept
    });
    off.run_steps(2);
    assert_eq!(off.state().tabs.len(), 1, "restore off");
    assert_eq!(off.state().settings_to_save().last_tabs, []);
}

/// The app started with a host on the command line ignores the tabs of last time.
fn app_with_host(settings: Settings) -> Harness<'static, ParleySshUi> {
    let mut h = app(settings);
    let ctx = h.ctx.clone();
    h.state_mut().start(
        &ctx,
        StartRequest {
            host: Some("me@10.255.255.1".into()),
            ..StartRequest::default()
        },
    );
    h
}

#[test]
fn a_host_on_the_command_line_wins_over_the_tabs_of_last_time_r_ssh_52() {
    let mut h = app(Settings {
        restore_tabs: true,
        ..Settings::default()
    });
    h.run_steps(2);
    let ctx = h.ctx.clone();
    h.state_mut().new_tab(&ctx, None);
    let kept = h.state().settings_to_save();
    let mut started = app_with_host(kept);
    started.run_steps(2);
    assert_eq!(started.state().tabs.len(), 1);
}

#[test]
fn the_terminal_page_offers_reconnect_and_restore_r_ssh_51_r_ssh_52() {
    let mut h = app(Settings::default());
    h.run_steps(2);
    h.state_mut()
        .carry_out(&egui::Context::default(), Action::Settings);
    h.run_steps(2);
    h.get_by_label("Terminal").click();
    h.run_steps(2);
    h.get_by_label("Keep trying for");
    h.get_by_label("Open the tabs of last time").scroll_to_me();
    h.run_steps(2);
    h.get_by_label("Reconnect a lost connection").scroll_to_me();
    h.run_steps(2);
    h.get_by_label("Reconnect a lost connection").click();
    h.run_steps(2);
    assert!(!h.state().settings.reconnect);
    assert!(h.query_by_label("Keep trying for").is_none());
}

#[test]
fn the_palette_finds_an_action_and_runs_it_and_lists_it_first_next_time_r_ssh_48() {
    let mut h = app(with_snippets());
    h.run_steps(2);
    key(&mut h, egui::Key::P, CTRL_SHIFT);
    assert!(h.state().palette.is_some(), "Ctrl+Shift+P opens it");
    // A typo forgiven: Enter runs the first match.
    h.get_by_label("Find").type_text("setings");
    h.run_steps(2);
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(h.state().palette.is_none());
    assert!(h.state().settings_dialog.is_some(), "Settings opened");
    assert_eq!(h.state().settings.palette_recent, ["action:Settings"]);
    let items = h.state().palette_items(false);
    let ranked = palette::ranked(&items, "", &h.state().settings.palette_recent);
    assert_eq!(ranked[0].label, "Settings", "recent first");
    assert!(
        items
            .iter()
            .any(|i| i.kind == "Snippet" && i.label == "uptime")
    );
    assert!(
        items
            .iter()
            .any(|i| i.label == "New tab" && i.shortcut == "Ctrl+Shift+T")
    );
}

#[test]
fn a_session_from_the_palette_connects_in_the_start_screen_or_a_new_tab_and_is_recent_r_ssh_48() {
    let mut h = app(Settings {
        sessions: vec![waiting()],
        ..Settings::default()
    });
    h.run_steps(2);
    let ctx = h.ctx.clone();
    let chosen = palette::Closed::Chosen("session:Wait".into(), palette::Choice::Session(0));
    h.state_mut().palette_chosen(&ctx, chosen.clone());
    assert_eq!(h.state().tabs.len(), 1, "the start screen took it");
    assert!(h.state().active_session().is_some());
    h.state_mut().palette_chosen(&ctx, chosen);
    assert_eq!(
        h.state().tabs.len(),
        2,
        "a new tab, the first being connected"
    );
    assert_eq!(h.state().settings.recent.len(), 1, "one address once");
    assert_eq!(
        h.state().settings.recent[0].name,
        "Wait",
        "a saved session is kept with its name (R-SSH-64)"
    );
}

#[test]
fn a_click_on_a_tab_brings_it_to_the_front_with_and_without_a_colour_r_ssh_39_r_ssh_61() {
    for color in [
        parley_ui_egui::SessionColor::None,
        parley_ui_egui::SessionColor::Teal,
    ] {
        let mut h = app(Settings::default());
        h.run_steps(2);
        let ctx = h.ctx.clone();
        let session = SavedSession { color, ..waiting() };
        let first = h.state().tabs[0].active;
        h.state_mut()
            .connect_pane(&ctx, first, session.clone(), false);
        h.state_mut().new_tab(&ctx, None);
        let second = h.state().tabs[1].active;
        h.state_mut().connect_pane(
            &ctx,
            second,
            SavedSession {
                name: "Other".into(),
                host: "10.255.255.2".into(),
                ..session
            },
            false,
        );
        h.run_steps(3);
        assert_eq!(h.state().current, 1);
        let title = h.state().tabs[0].title();
        pointer_click(&mut h, &title, 0);
        assert_eq!(
            h.state().current,
            0,
            "{color:?}: the first tab came to the front"
        );
    }
}

/// Sessions named `names`, each at an address that does not answer.
fn many(names: &[&str]) -> Vec<SavedSession> {
    names
        .iter()
        .map(|name| SavedSession {
            name: (*name).into(),
            ..waiting()
        })
        .collect()
}

#[test]
fn the_star_makes_a_session_a_favourite_listed_first_and_back_r_ssh_63() {
    let mut h = app(Settings {
        sessions: many(&["Alpha", "Beta æøå"]),
        all_sessions_open: true,
        ..Settings::default()
    });
    h.run_steps(2);
    assert!(h.query_by_label("FAVOURITES").is_none());
    h.get_all_by_label("Add to favourites")
        .nth(1)
        .expect("Beta's star")
        .click();
    h.run_steps(2);
    let favourites: Vec<bool> = h
        .state()
        .settings
        .sessions
        .iter()
        .map(|s| s.favourite)
        .collect();
    assert_eq!(favourites, [false, true]);
    h.get_by_label("FAVOURITES");
    h.get_by_label("ALL SESSIONS (2)");
    let palette: Vec<String> = h
        .state()
        .palette_items(false)
        .into_iter()
        .filter(|i| i.kind == "Session")
        .map(|i| i.label)
        .collect();
    assert_eq!(
        palette,
        ["Beta æøå", "Alpha"],
        "the palette lists favourites first"
    );
    assert_eq!(
        h.get_all_by_label("Beta æøå").count(),
        2,
        "a favourite is listed under All sessions too"
    );
    h.get_all_by_label("Remove from favourites")
        .next()
        .expect("Beta's star")
        .click();
    h.run_steps(2);
    assert!(!h.state().settings.sessions[1].favourite);
    assert!(h.query_by_label("FAVOURITES").is_none());
}

#[test]
fn recent_connections_are_listed_and_a_saved_one_connects_as_it_is_now_r_ssh_64() {
    let quick = SavedSession {
        name: String::new(),
        host: "quick.lan".into(),
        ..waiting()
    };
    let mut h = app(Settings {
        sessions: many(&["Wait"]),
        recent: vec![quick, waiting()],
        ..Settings::default()
    });
    h.run_steps(2);
    h.get_by_label("RECENT");
    h.get_by_label("me@quick.lan");
    h.get_by_label("Not saved");
    assert_eq!(
        h.get_all_by_label("Wait").count(),
        1,
        "in Recent; All sessions is folded"
    );
    h.get_by_label("Save as session…");

    h.state_mut().settings.sessions[0].port = 2200;
    let ctx = h.ctx.clone();
    let pane = h.state().tabs[0].active;
    h.state_mut().chosen(&ctx, pane, start::Choice::Recent(1));
    let session = h.state().active_session().expect("connecting");
    assert!(session.saved);
    assert_eq!(session.source.port, 2200, "the saved session as it is now");
}

#[test]
fn the_search_has_the_keyboard_and_up_down_and_enter_connect_a_match_r_ssh_65() {
    let mut h = app(Settings {
        sessions: many(&["Web one", "Database", "Web two", "Mail"]),
        ..Settings::default()
    });
    h.run_steps(3);
    h.event(egui::Event::Text("web".into()));
    h.run_steps(2);
    assert_eq!(h.state().active_session().map(|s| s.saved), None);
    assert!(h.query_by_label("Database").is_none(), "only the matches");
    press(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    press(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    h.run_steps(2);
    let session = h.state().active_session().expect("connecting");
    assert_eq!(session.source.name, "Web two");
}

#[test]
fn escape_empties_the_search_and_a_long_list_folds_under_the_favourites_r_ssh_65() {
    let names: Vec<String> = (1..=12).map(|n| format!("Host {n:02}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut sessions = many(&names);
    sessions[0].favourite = true;
    let mut h = app(Settings {
        sessions,
        ..Settings::default()
    });
    h.run_steps(3);
    h.get_by_label("ALL SESSIONS (12)");
    assert!(
        h.query_by_label("Host 05").is_none(),
        "more than ten under a favourite start folded"
    );
    h.event(egui::Event::Text("05".into()));
    h.run_steps(2);
    h.get_by_label("Host 05");
    press(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    h.run_steps(2);
    let tab = &h.state().tabs[0];
    let Some(Pane::Start(form)) = tab.pane(tab.active) else {
        panic!("the start screen");
    };
    assert_eq!(form.search, "");
    h.get_by_label("ALL SESSIONS (12)");
}

#[test]
fn ctrl_shift_y_opens_the_transfer_panel_and_close_closes_it_r_ssh_56() {
    let mut h = app(Settings::default());
    h.run_steps(2);
    assert!(h.query_by_label("Copy…").is_none());
    press(&mut h, egui::Key::Y, CTRL_SHIFT);
    let copy = h.get_by_label("Copy…");
    assert!(
        copy.accesskit_node().is_disabled(),
        "nothing to copy between without two sessions"
    );
    assert!(h.state().transfer.is_some());
    h.get_by_label("Close").click();
    h.run_steps(2);
    assert!(h.state().transfer.is_none());
    assert!(h.query_by_label("Copy…").is_none());
}

#[test]
fn a_saved_transfer_is_run_from_the_palette_and_a_session_no_longer_saved_is_said_r_ssh_56() {
    let mut settings = Settings::default();
    settings.transfers.push(crate::transfer::SavedTransfer {
        name: "Nightly æøå".into(),
        source: "gone".into(),
        target: "also gone".into(),
        ..crate::transfer::SavedTransfer::default()
    });
    let mut h = app(settings);
    h.run_steps(2);
    let listed: Vec<(String, String)> = h
        .state()
        .palette_items(false)
        .into_iter()
        .filter(|i| i.kind == "Transfer")
        .map(|i| (i.label, i.detail))
        .collect();
    assert_eq!(listed, [("Nightly æøå".into(), "gone to also gone".into())]);
    let ctx = h.ctx.clone();
    h.state_mut().run_saved_transfer(&ctx, 0);
    h.run_steps(2);
    let panel = h.state().transfer.as_ref().expect("the panel is open");
    assert!(!panel.waiting, "nothing to wait for");
    assert!(
        matches!(&panel.outcome, Some(crate::transfer::Outcome::Failed(why)) if why == "No session is saved as gone any more."),
        "{:?}",
        panel.outcome
    );
    assert_eq!(panel.name, "Nightly æøå");
}

#[test]
fn the_transfers_page_names_another_rsync_and_its_buttons_stay_put_r_ssh_66() {
    let mut h = app(Settings::default());
    h.run_steps(2);
    h.state_mut()
        .carry_out(&egui::Context::default(), Action::Settings);
    h.run_steps(3);
    let before = h.get_by_label("Save").rect();
    h.get_by_label("Transfers").click();
    h.run_steps(2);
    h.get_by_label("rsync").click();
    h.run_steps(1);
    h.get_by_label("rsync").type_text("/opt/æøå/rsync");
    h.run_steps(3);
    assert_eq!(h.state().settings.transfer_rsync, "/opt/æøå/rsync");
    assert_eq!(
        h.get_by_label("Save").rect(),
        before,
        "the dialog keeps its size"
    );
}

/// Chooses files for the panel without a window: always the same folder.
struct ChosenFolder;

impl crate::transfer::DesktopFiles for ChosenFolder {
    fn choose(&self, folder: bool) -> Option<String> {
        folder.then(|| "/home/niels/æøå".to_owned())
    }
}

#[test]
fn this_computer_as_an_end_takes_a_path_chosen_with_the_file_window_r_ssh_55() {
    let mut h = app(Settings::default());
    h.run_steps(2);
    h.state_mut().set_desktop_files(Box::new(ChosenFolder));
    press(&mut h, egui::Key::Y, CTRL_SHIFT);
    if let Some(panel) = &mut h.state_mut().transfer {
        panel.target = Some(crate::transfer::DESKTOP);
    }
    h.run_steps(2);
    assert!(
        h.query_by_label("Folder…").is_some(),
        "the file window for this computer's path"
    );
    h.get_by_label("Folder…").click();
    h.run_steps(2);
    let panel = h.state().transfer.as_ref().expect("the panel");
    assert_eq!(
        panel.target_path, "/home/niels/æøå/",
        "a folder's contents go into it"
    );
    assert_eq!(
        h.query_all_by_label("Folder…").count(),
        1,
        "only this computer's end has it"
    );
}

#[test]
fn not_for_this_session_and_a_key_set_up_are_kept_in_the_saved_session_r_ssh_67() {
    use crate::key_setup::{Choice, Order};
    let mut settings = Settings::default();
    settings.sessions.push(waiting());
    let mut h = app(settings);
    h.run_steps(1);
    let ctx = h.ctx.clone();
    let pane = h.state().tabs[0].active;
    h.state_mut().connect_pane(&ctx, pane, waiting(), true);
    h.state_mut().no_key_offer(pane);
    assert!(h.state().settings.sessions[0].key_offer_off);

    let key = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAV5CINKVHRsJdOO4vr5nwVr0ASkuG8DODBufVj2pCQ9";
    let line = parley_ssh_core::key_setup::PublicLine::parse(key).expect("a key line");
    h.state_mut().carry_key_orders(
        pane,
        vec![Order::Remember {
            session: "Wait".into(),
            key: Choice::File("/home/me/.ssh/id_æøå".into(), Ok(line)),
        }],
    );
    assert_eq!(
        h.state().settings.sessions[0].key_files,
        ["/home/me/.ssh/id_æøå"]
    );
    let restarted = h
        .state()
        .active_session()
        .map(|s| s.source.key_files.clone());
    assert_eq!(
        restarted,
        Some(vec!["/home/me/.ssh/id_æøå".to_owned()]),
        "a restart uses it too"
    );
}
