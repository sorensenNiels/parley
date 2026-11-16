//! Settings saved by Parley 0.3.1, when both programs shared one settings type, load into each
//! program's own settings now (D36). eframe keeps them as RON, so they are read as RON here.

/// One program's settings as 0.3.1 saved them: the terminal's fields and the SQL window's.
const SAVED_BY_0_3_1: &str = r#"(theme:Dark,text_size:125,density:Compact,terminal_font:"IBM Plex Mono",check_for_updates:false,addons:(enabled:["sql","sql-assistant"],disabled:["macros"]),keyboard_maps:[],java_path:"/opt/java/bin/java",sql_rows:500,sql_timeout:30,sql_header:Names,sql_results:beside,sql_close_pairs:false,sql_format:(case:Lower,indent:4,commas_first:true),sql_keys:{},sql_snippets_shared:"/srv/snippets",sql_history:{},sql_scripts:{},sql_pinned:{"Prod":["QGPL","ÆØÅLIB"]},sql_groups:{},assistant:(provider:anthropic,model:"",url:""))"#;

#[test]
fn parleys_settings_saved_by_0_3_1_still_load() {
    let settings: parley_ui_egui::Settings = ron::from_str(SAVED_BY_0_3_1).expect("loads");
    assert_eq!(settings.theme, parley_ui_egui::Theme::Dark);
    assert_eq!(settings.text_size, 125);
    assert_eq!(settings.terminal_font, "IBM Plex Mono");
    assert!(!settings.check_for_updates);
    assert!(settings.addons.enabled(parley_ui_egui::AddOn::Sql));
    assert!(!settings.addons.enabled(parley_ui_egui::AddOn::Macros));
}

#[test]
fn parley_sqls_settings_saved_by_0_3_1_still_load() {
    let settings: parley_sql_ui::Settings = ron::from_str(SAVED_BY_0_3_1).expect("loads");
    assert_eq!(settings.theme, parley_ui_egui::Theme::Dark);
    assert_eq!(settings.text_size, 125);
    assert!(!settings.check_for_updates);
    assert_eq!(settings.java_path, "/opt/java/bin/java");
    assert_eq!(settings.sql_rows, 500);
    assert_eq!(settings.sql_timeout, 30);
    assert!(!settings.sql_close_pairs);
    assert_eq!(settings.sql_snippets_shared, "/srv/snippets");
    assert_eq!(settings.sql_pinned["Prod"], ["QGPL", "ÆØÅLIB"]);
    assert_eq!(settings.sql_format.indent, 4);
    assert!(
        settings.addons.enabled(parley_sql_ui::Assistant),
        "the assistant keeps its id"
    );
}

#[test]
fn each_programs_settings_round_trip_through_storage() {
    let parley = parley_ui_egui::Settings::default();
    let text = ron::to_string(&parley).expect("saves");
    assert_eq!(
        ron::from_str::<parley_ui_egui::Settings>(&text).expect("loads"),
        parley
    );
    let sql = parley_sql_ui::Settings::default();
    let text = ron::to_string(&sql).expect("saves");
    assert_eq!(
        ron::from_str::<parley_sql_ui::Settings>(&text).expect("loads"),
        sql
    );
}

/// Parley SSH's settings as 0.8 saved them, before saved sessions (Phase 26).
const SSH_SAVED_BY_0_8: &str = r#"(theme:Light,text_size:110,density:Comfortable,check_for_updates:true,confirm_close:false,confirm_paste:true,last_host:"box.lan",last_port:2222,last_user:"æøå")"#;

#[test]
fn parley_sshs_settings_saved_before_saved_sessions_still_load_r_ssh_11() {
    let settings: parley_ssh_ui::Settings = ron::from_str(SSH_SAVED_BY_0_8).expect("loads");
    assert_eq!(settings.text_size, 110);
    assert!(!settings.confirm_close);
    assert_eq!(
        (settings.last_host.as_str(), settings.last_port),
        ("box.lan", 2222)
    );
    assert_eq!(settings.sessions, []);
    assert!(settings.defaults.use_agent, "the defaults' own default");
}

#[test]
fn parley_sshs_saved_sessions_are_read_back_from_storage_r_ssh_11() {
    let session = parley_ssh_ui::sessions::SavedSession {
        name: "Prod æøå".into(),
        host: "box.lan".into(),
        port: 2222,
        user: "me".into(),
        key_files: vec!["C:\\Users\\me\\key.ppk".into()],
        agent_key: "SHA256:abc".into(),
        agent_forwarding: true,
        ..parley_ssh_ui::sessions::SavedSession::default()
    };
    let settings = parley_ssh_ui::Settings {
        sessions: vec![session],
        ..parley_ssh_ui::Settings::default()
    };
    let text = ron::to_string(&settings).expect("saves");
    assert!(!text.contains("password"), "no password field: {text}");
    assert_eq!(
        ron::from_str::<parley_ssh_ui::Settings>(&text).expect("loads"),
        settings
    );
}
