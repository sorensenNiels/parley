//! Generated SQL from the browser (R-SQL-93..96): a library's SQL in a new script tab, a table
//! with its indexes and views, and a backup's SQL before its rows are read. Each is a batch of
//! catalog questions, put together when the last is answered.

use std::collections::BTreeSet;
use std::path::PathBuf;

use parley_sql_core::catalog::{self, Kind, Object};
use parley_sql_core::generate::{self, DataTable, Part};
use parley_sql_core::protocol::Row;

use super::{Browser, Pick};
use crate::database::{Ask, Database, Step};
use parley_ui_egui::{dialog, theme};

/// What a batch makes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Goal {
    /// A library's SQL in a new script tab (R-SQL-93).
    Script,
    /// A table with its indexes and views in a new script tab (R-SQL-94).
    Table,
    /// An SQL file: the library's SQL and every table's rows (R-SQL-96); with `only`, the rows
    /// of that table or view alone (R-SQL-95).
    Backup { path: PathBuf, only: Option<String> },
}

/// A backup's SQL, ready for its rows to be read into `path` (R-SQL-95, R-SQL-96).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backup {
    pub path: PathBuf,
    /// The system name of the library.
    pub library: String,
    /// Written before the rows: the header and the tables.
    pub before: String,
    /// Written after the rows: everything that needs the tables, or would stop the rows.
    pub after: String,
    pub tables: Vec<DataTable>,
    /// A whole library, not one table.
    pub whole: bool,
}

/// The dialog of Generate SQL… and Back up as SQL… on a library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asking {
    library: String,
    backup: bool,
    kinds: BTreeSet<Kind>,
}

/// What the dialog chose; a backup asks for its file before it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen {
    pub library: String,
    pub backup: bool,
    pub kinds: BTreeSet<Kind>,
}

/// A piece of the script: a part of a library, or one object of a table's; its SQL once read.
#[derive(Debug)]
struct Section {
    part: Option<Part>,
    label: String,
    text: Option<Result<String, String>>,
}

impl Section {
    fn text(&self) -> Result<String, String> {
        self.text
            .clone()
            .unwrap_or_else(|| Err("Not answered.".to_owned()))
    }
}

/// Generated SQL being read.
#[derive(Debug)]
pub(super) struct Batch {
    serial: u64,
    library: String,
    title: String,
    goal: Goal,
    kinds: BTreeSet<Kind>,
    /// Questions asked and not yet answered.
    waiting: usize,
    sections: Vec<Section>,
    tables: Vec<DataTable>,
    failed: Option<String>,
}

impl Batch {
    fn ask(&mut self, step: Step, sql: String, db: &mut Database) {
        self.waiting += 1;
        db.ask(Ask::Batch {
            batch: self.serial,
            step,
            sql,
        });
    }

    /// The library's contents came: a section for each part chosen that it has.
    fn contents(&mut self, rows: &[Row], db: &mut Database) {
        let contents = generate::contents_from(rows);
        for part in Part::ALL {
            if self.kinds.contains(&part.kind()) && contents.parts.contains(&part) {
                let n = self.sections.len();
                self.sections.push(Section {
                    part: Some(part),
                    label: part.heading().to_owned(),
                    text: None,
                });
                self.ask(Step::Section(n), generate::part(&self.library, part), db);
            }
        }
        self.tables = contents.tables;
    }

    /// A table's indexes and views came: a section for each.
    fn dependents(&mut self, rows: &[Row], db: &mut Database) {
        for found in generate::dependents_from(rows) {
            let n = self.sections.len();
            self.sections.push(Section {
                part: None,
                label: catalog::qualified(&found.library, &found.name),
                text: None,
            });
            let sql = catalog::ddl(&found.library, &found.name, found.kind);
            self.ask(Step::Section(n), sql, db);
        }
    }

    /// The parts' text under their headings, of the parts `which` takes.
    fn sections(&self, which: impl Fn(Part) -> bool) -> String {
        self.sections
            .iter()
            .filter_map(|s| s.part.filter(|p| which(*p)).map(|p| (p, s.text())))
            .map(|(part, text)| generate::section(part, &text))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// What the batch made, once every question is answered.
    fn finish(self) -> Pick {
        if let Some(e) = &self.failed {
            return Pick::Failed(match &self.goal {
                Goal::Script | Goal::Table => format!("No SQL for {}: {e}", self.title),
                Goal::Backup { only: None, .. } => format!("Not backed up: {e}"),
                Goal::Backup { only: Some(_), .. } => format!("Not downloaded: {e}"),
            });
        }
        let when = generate::now();
        match &self.goal {
            Goal::Script if self.sections.is_empty() => Pick::Failed(format!(
                "No SQL for {}: it has none of the objects chosen.",
                self.title
            )),
            Goal::Script => Pick::Script {
                text: format!(
                    "{}{}",
                    generate::script_header(&self.library, &when),
                    self.sections(|_| true)
                ),
                title: self.title,
                run: false,
            },
            Goal::Table => {
                let pieces: Vec<String> = self
                    .sections
                    .iter()
                    .map(|s| generate::piece(&s.label, &s.text()))
                    .collect();
                Pick::Script {
                    text: pieces.join("\n"),
                    title: self.title,
                    run: false,
                }
            }
            Goal::Backup {
                only: Some(name), ..
            } if self.tables.is_empty() => Pick::Failed(format!(
                "Not downloaded: {} was not found.",
                catalog::qualified(&self.library, name)
            )),
            Goal::Backup { path, only } => {
                let header = match only {
                    Some(name) => {
                        generate::rows_header(&catalog::qualified(&self.library, name), &when)
                    }
                    None => generate::backup_header(&self.library, &when),
                };
                let tables = self.sections(|p| p == Part::Tables);
                let after = self.sections(|p| p != Part::Tables);
                Pick::Backup(Backup {
                    path: path.clone(),
                    before: format!("{header}{tables}"),
                    after: if after.is_empty() {
                        after
                    } else {
                        format!("\n{after}")
                    },
                    whole: only.is_none(),
                    library: self.library,
                    tables: self.tables,
                })
            }
        }
    }
}

impl Browser {
    /// Opens the dialog of Generate SQL… (`backup` false) or Back up as SQL… on `library`.
    pub(super) fn ask_generate(&mut self, library: &str, backup: bool) {
        self.generating = Some(Asking {
            library: library.to_owned(),
            backup,
            kinds: Kind::ALL.into_iter().collect(),
        });
    }

    /// What the dialog chose, once.
    pub fn take_chosen(&mut self) -> Option<Chosen> {
        self.chosen.take()
    }

    /// Starts reading `kinds` of objects in `library` (system name) for `goal`: a script, or a
    /// backup's SQL; its pick comes from [`Browser::receive`] when all is read.
    pub fn generate(
        &mut self,
        library: &str,
        kinds: BTreeSet<Kind>,
        goal: Goal,
        db: &mut Database,
    ) {
        let only = match &goal {
            Goal::Backup { only, .. } => only.clone(),
            Goal::Script | Goal::Table => None,
        };
        let mut batch = self.batch(library, catalog::identifier(library), goal, kinds);
        batch.ask(
            Step::Contents,
            generate::contents(library, only.as_deref()),
            db,
        );
        self.batches.push(batch);
    }

    /// Starts reading the SQL of the table `object` with its indexes and views (R-SQL-94).
    pub(super) fn generate_table(&mut self, library: &str, object: &Object, db: &mut Database) {
        let title = catalog::qualified(library, &object.name);
        let mut batch = self.batch(library, title.clone(), Goal::Table, BTreeSet::new());
        batch.sections.push(Section {
            part: None,
            label: title,
            text: None,
        });
        let table = catalog::ddl(library, &object.system_name, Kind::Table);
        batch.ask(Step::Section(0), table, db);
        let dependents = generate::dependents(library, &object.system_name);
        batch.ask(Step::Dependents, dependents, db);
        self.batches.push(batch);
    }

    fn batch(&mut self, library: &str, title: String, goal: Goal, kinds: BTreeSet<Kind>) -> Batch {
        self.serial += 1;
        Batch {
            serial: self.serial,
            library: library.to_owned(),
            title,
            goal,
            kinds,
            waiting: 0,
            sections: Vec::new(),
            tables: Vec::new(),
            failed: None,
        }
    }

    /// An answer to batch `serial`; its pick when it was the last.
    pub(super) fn receive_batch(
        &mut self,
        serial: u64,
        step: Step,
        rows: Result<Vec<Row>, String>,
        db: &mut Database,
    ) -> Option<Pick> {
        let i = self.batches.iter().position(|b| b.serial == serial)?;
        let batch = self.batches.get_mut(i)?;
        batch.waiting = batch.waiting.saturating_sub(1);
        match (step, rows) {
            (Step::Contents, Ok(rows)) => batch.contents(&rows, db),
            (Step::Dependents, Ok(rows)) => batch.dependents(&rows, db),
            (Step::Contents, Err(e)) => batch.failed = Some(e),
            (Step::Dependents, Err(e)) => batch.sections.push(Section {
                part: None,
                label: "Its indexes and views".to_owned(),
                text: Some(Err(e)),
            }),
            (Step::Section(n), rows) => {
                if let Some(section) = batch.sections.get_mut(n) {
                    section.text = Some(rows.map(|r| catalog::ddl_from(&r)));
                }
            }
        }
        if batch.waiting > 0 {
            return None;
        }
        Some(self.batches.remove(i).finish())
    }

    /// The dialog of Generate SQL… and Back up as SQL… while open.
    pub(super) fn generate_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut asking) = self.generating.take() else {
            return;
        };
        let (title, confirm, about) = if asking.backup {
            (
                "Back up as SQL",
                "Back up…",
                "One SQL file: the tables, every row as an INSERT statement, then the objects chosen here. The rows are written as they come.",
            )
        } else {
            (
                "Generate SQL",
                "Generate",
                "One script in a new tab, in an order that can be run: tables, constraints, indexes, functions, views, procedures, triggers.",
            )
        };
        let (close, (cancel, go)) = dialog::dialog(ctx, "sql-generate", title, 440.0, |ui| {
            ui.label(format!("Library {}", asking.library));
            theme::hint(ui, about);
            ui.add_space(10.0);
            theme::section_label(ui, "Objects");
            for kind in Kind::ALL {
                // A backup always has its tables: the rows go into them.
                if asking.backup && kind == Kind::Table {
                    continue;
                }
                let mut on = asking.kinds.contains(&kind);
                if theme::switch_row(ui, kind_label(kind), &mut on).changed() {
                    if on {
                        asking.kinds.insert(kind);
                    } else {
                        asking.kinds.remove(&kind);
                    }
                }
            }
            let ready = asking.backup || !asking.kinds.is_empty();
            dialog::actions(ui, theme::primary_button(ui, confirm), ready)
        });
        if close || cancel {
            return;
        }
        if go {
            let mut kinds = asking.kinds;
            if asking.backup {
                kinds.insert(Kind::Table);
            }
            self.chosen = Some(Chosen {
                library: asking.library,
                backup: asking.backup,
                kinds,
            });
            return;
        }
        self.generating = Some(asking);
    }
}

fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Table => "Tables, with their constraints and triggers",
        Kind::View => "Views",
        Kind::Index => "Indexes",
        Kind::Procedure => "Procedures",
        Kind::Function => "Functions",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::State;

    fn row(cells: &[&str]) -> Row {
        cells.iter().map(|c| Some((*c).to_owned())).collect()
    }

    /// One line of `QSYS2.GENERATE_SQL`'s answer.
    fn lines(text: &str) -> Vec<Row> {
        text.lines().map(|l| row(&["1", "0", l])).collect()
    }

    /// The batch questions waiting, as (serial, step, SQL).
    fn asked(db: &mut Database) -> Vec<(u64, Step, String)> {
        db.take_queued()
            .into_iter()
            .filter_map(|ask| match ask {
                Ask::Batch { batch, step, sql } => Some((batch, step, sql)),
                _ => None,
            })
            .collect()
    }

    /// Answers each question in `asks` (in reverse, as answers need not keep order) with
    /// `answer`'s rows; the picks made.
    fn answer_all(
        b: &mut Browser,
        db: &mut Database,
        asks: Vec<(u64, Step, String)>,
        answer: impl Fn(&str) -> Result<Vec<Row>, String>,
    ) -> Vec<Pick> {
        asks.into_iter()
            .rev()
            .filter_map(|(serial, step, sql)| b.receive_batch(serial, step, answer(&sql), db))
            .collect()
    }

    fn all_but_functions() -> BTreeSet<Kind> {
        [Kind::Table, Kind::Index, Kind::View, Kind::Procedure]
            .into_iter()
            .collect()
    }

    #[test]
    fn a_librarys_script_has_the_parts_it_holds_in_an_order_that_runs_r_sql_93() {
        let mut b = Browser::default();
        let mut db = Database::ready_for_tests("QZDASOINIT");
        b.generate("KUNDELIB", all_but_functions(), Goal::Script, &mut db);
        let first = asked(&mut db);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].1, Step::Contents);
        // It has tables, constraints, views, functions and triggers; no indexes or procedures.
        let contents: Vec<Row> = ["T", "C", "V", "F", "R"]
            .iter()
            .map(|code| row(&[code, "", ""]))
            .chain([row(&["D", "KUNDER", "N"])])
            .collect();
        assert_eq!(
            answer_all(&mut b, &mut db, first, |_| Ok(contents.clone())),
            []
        );
        let parts = asked(&mut db);
        let types: Vec<&str> = parts
            .iter()
            .map(|(_, _, sql)| {
                let at = sql.find("DATABASE_OBJECT_TYPE => '").expect("a type") + 25;
                &sql[at..at + sql[at..].find('\'').expect("its end")]
            })
            .collect();
        assert_eq!(
            types,
            ["TABLE", "CONSTRAINT", "VIEW", "TRIGGER"],
            "chosen and there, in order; functions were not chosen"
        );
        let picks = answer_all(&mut b, &mut db, parts, |sql| {
            if sql.contains("'VIEW'") {
                Err("SQL0204 æøå".to_owned())
            } else if sql.contains("'TABLE'") {
                Ok(lines("CREATE TABLE KUNDELIB.KUNDER (NAVN VARCHAR(50));"))
            } else {
                Ok(lines("ALTER TABLE X;"))
            }
        });
        let [Pick::Script { title, text, run }] = picks.as_slice() else {
            panic!("one script: {picks:?}");
        };
        assert_eq!(title, "KUNDELIB");
        assert!(!run);
        assert!(text.starts_with("-- The SQL of library KUNDELIB"), "{text}");
        let tables = text.find("-- Tables\n\nCREATE TABLE").expect("tables");
        let constraints = text.find("-- Constraints").expect("constraints");
        let views = text
            .find("-- Views: not generated: SQL0204 æøå")
            .expect("the failure, as a comment");
        let triggers = text.find("-- Triggers").expect("triggers");
        assert!(tables < constraints && constraints < views && views < triggers);
        assert!(b.batches.is_empty(), "done");
    }

    #[test]
    fn a_library_without_the_kinds_chosen_says_so_r_sql_93() {
        let mut b = Browser::default();
        let mut db = Database::ready_for_tests("J");
        let only_views = [Kind::View].into_iter().collect();
        b.generate("TOM", only_views, Goal::Script, &mut db);
        let asks = asked(&mut db);
        let picks = answer_all(&mut b, &mut db, asks, |_| Ok(vec![row(&["T", "", ""])]));
        assert!(
            matches!(picks.as_slice(), [Pick::Failed(why)] if why.contains("none of the objects")),
            "{picks:?}"
        );
    }

    #[test]
    fn a_failed_contents_question_fails_the_backup_r_sql_96() {
        let mut b = Browser::default();
        // Not signed on: the question is answered at once, with why.
        let mut db = Database::default();
        assert_eq!(db.state, State::Off);
        let goal = Goal::Backup {
            path: PathBuf::from("lib.sql"),
            only: None,
        };
        b.generate("LIB", all_but_functions(), goal, &mut db);
        let answers = db.take_answers();
        let picks = b.receive(answers, &mut db);
        assert!(
            matches!(picks.as_slice(), [Pick::Failed(why)] if why.starts_with("Not backed up: Not signed on")),
            "{picks:?}"
        );
    }

    #[test]
    fn a_table_comes_with_its_indexes_and_views_r_sql_94() {
        let mut b = Browser::default();
        let mut db = Database::ready_for_tests("J");
        let object = Object {
            kind: Kind::Table,
            name: "KUNDER_ÆØÅ".to_owned(),
            system_name: "KUNDE00001".to_owned(),
            text: String::new(),
        };
        b.generate_table("LIB", &object, &mut db);
        let first = asked(&mut db);
        assert_eq!(
            first.iter().map(|a| a.1).collect::<Vec<_>>(),
            [Step::Section(0), Step::Dependents]
        );
        assert!(first[0].2.contains("'KUNDE00001'"), "by system name");
        // The table's SQL comes first; the dependents after.
        let (table, dependents) = (first[0].clone(), first[1].clone());
        let ddl = |_: &str| Ok(lines("CREATE TABLE LIB.\"KUNDER_ÆØÅ\" (A INT);"));
        assert_eq!(answer_all(&mut b, &mut db, vec![table], ddl), []);
        let found = vec![
            row(&["I", "LIB", "KUNDER_IX"]),
            row(&["V", "ANDEN", "KUNDEV"]),
        ];
        assert_eq!(
            answer_all(&mut b, &mut db, vec![dependents], |_| Ok(found.clone())),
            []
        );
        let more = asked(&mut db);
        assert_eq!(more.len(), 2);
        assert!(more[0].2.contains("'INDEX'") && more[1].2.contains("'VIEW'"));
        let picks = answer_all(&mut b, &mut db, more, |sql| {
            if sql.contains("'INDEX'") {
                Ok(lines(
                    "CREATE INDEX LIB.KUNDER_IX ON LIB.\"KUNDER_ÆØÅ\" (A);",
                ))
            } else {
                Err("SQL0551".to_owned())
            }
        });
        let [Pick::Script { title, text, .. }] = picks.as_slice() else {
            panic!("one script: {picks:?}");
        };
        assert_eq!(title, "LIB.\"KUNDER_ÆØÅ\"");
        assert_eq!(
            text,
            "CREATE TABLE LIB.\"KUNDER_ÆØÅ\" (A INT);\n\n\
             CREATE INDEX LIB.KUNDER_IX ON LIB.\"KUNDER_ÆØÅ\" (A);\n\n\
             -- ANDEN.KUNDEV: not generated: SQL0551\n"
        );
    }

    #[test]
    fn a_backup_has_the_tables_before_the_rows_and_the_rest_after_r_sql_96() {
        let mut b = Browser::default();
        let mut db = Database::ready_for_tests("J");
        let goal = Goal::Backup {
            path: PathBuf::from("lib.sql"),
            only: None,
        };
        let tables_and_indexes = [Kind::Table, Kind::Index].into_iter().collect();
        b.generate("LIB", tables_and_indexes, goal, &mut db);
        let first = asked(&mut db);
        assert!(first[0].2.contains("TABLE_TYPE IN ('T', 'P')"));
        let contents = vec![
            row(&["T", "", ""]),
            row(&["I", "", ""]),
            row(&["D", "A", "N"]),
            row(&["D", "B", "Y"]),
        ];
        answer_all(&mut b, &mut db, first, |_| Ok(contents.clone()));
        let parts = asked(&mut db);
        let picks = answer_all(&mut b, &mut db, parts, |sql| {
            Ok(lines(if sql.contains("'TABLE'") {
                "CREATE TABLE LIB.A (N INT);"
            } else {
                "CREATE INDEX LIB.AX ON LIB.A (N);"
            }))
        });
        let [Pick::Backup(backup)] = picks.as_slice() else {
            panic!("a backup: {picks:?}");
        };
        assert!(backup.whole);
        assert_eq!(backup.library, "LIB");
        assert!(backup.before.starts_with("-- A backup of library LIB"));
        assert!(backup.before.contains("CREATE TABLE LIB.A"));
        assert!(!backup.before.contains("CREATE INDEX"));
        assert_eq!(
            backup.after,
            "\n-- Indexes\n\nCREATE INDEX LIB.AX ON LIB.A (N);\n"
        );
        assert_eq!(
            backup
                .tables
                .iter()
                .map(|t| (t.name.as_str(), t.overriding))
                .collect::<Vec<_>>(),
            [("A", false), ("B", true)]
        );
    }

    #[test]
    fn one_tables_rows_as_sql_need_it_found_r_sql_95() {
        let mut b = Browser::default();
        let mut db = Database::ready_for_tests("J");
        let goal = |only: &str| Goal::Backup {
            path: PathBuf::from("t.sql"),
            only: Some(only.to_owned()),
        };
        b.generate("LIB", BTreeSet::new(), goal("Ø"), &mut db);
        let first = asked(&mut db);
        assert!(first[0].2.contains("T.TABLE_NAME = 'Ø'"), "{}", first[0].2);
        let picks = answer_all(&mut b, &mut db, first, |_| {
            Ok(vec![row(&["T", "", ""]), row(&["D", "Ø", "Y"])])
        });
        let [Pick::Backup(backup)] = picks.as_slice() else {
            panic!("a backup: {picks:?}");
        };
        assert!(!backup.whole);
        assert!(backup.before.starts_with("-- The rows of LIB.\"Ø\""));
        assert_eq!(backup.after, "", "no SQL besides the rows");
        b.generate("LIB", BTreeSet::new(), goal("GONE"), &mut db);
        let asks = asked(&mut db);
        let picks = answer_all(&mut b, &mut db, asks, |_| Ok(Vec::new()));
        assert!(
            matches!(picks.as_slice(), [Pick::Failed(why)] if why.contains("LIB.GONE was not found")),
            "{picks:?}"
        );
    }

    /// The browser's dialogs drawn alone.
    fn dialogs(browser: Browser) -> egui_kittest::Harness<'static, Browser> {
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(900.0, 700.0))
            .with_max_steps(16)
            .build_ui_state(
                |ui, browser: &mut Browser| {
                    crate::test_support::install_fonts(ui);
                    browser.dialogs(ui.ctx());
                },
                browser,
            )
    }

    #[test]
    fn the_generate_dialog_keeps_its_buttons_in_place_and_takes_the_kinds_chosen_r_sql_93() {
        use egui_kittest::kittest::Queryable;
        let mut browser = Browser::default();
        browser.ask_generate("KUNDELIB", false);
        let mut harness = dialogs(browser);
        harness.get_by_label("Library KUNDELIB");
        crate::test_support::keeps_its_place(&mut harness, "Generate");
        harness.get_by_label("Views").click();
        harness.run();
        harness.get_by_label("Generate").click();
        harness.run();
        let chosen = harness.state_mut().take_chosen().expect("chosen");
        assert!(!chosen.backup);
        assert!(!chosen.kinds.contains(&Kind::View));
        assert!(chosen.kinds.contains(&Kind::Table));
    }

    #[test]
    fn the_backup_dialog_always_keeps_the_tables_r_sql_96() {
        use egui_kittest::kittest::Queryable;
        let mut browser = Browser::default();
        browser.ask_generate("LIB", true);
        let mut harness = dialogs(browser);
        crate::test_support::keeps_its_place(&mut harness, "Back up…");
        assert!(
            harness
                .query_by_label("Tables, with their constraints and triggers")
                .is_none(),
            "not a choice"
        );
        for kind in ["Views", "Indexes", "Procedures", "Functions"] {
            harness.get_by_label(kind).click();
            harness.run();
        }
        harness.get_by_label("Back up…").click();
        harness.run();
        let chosen = harness.state_mut().take_chosen().expect("chosen");
        assert!(chosen.backup);
        assert_eq!(chosen.kinds.into_iter().collect::<Vec<_>>(), [Kind::Table]);
    }
}
