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
