//! What the editor gets from around it: the assistant's answers (R-SQL-44), names to complete
//! and describe from the schema browser (R-SQL-36, R-SQL-37, R-SQL-65), picks from the browser,
//! snippets (R-SQL-25) and the history (R-SQL-89..91).

use egui::Ui;
use parley_sql_core::text;

use super::{
    SqlWindow, Tab, WAIT, assistant, browser, download, editor, history, snippets, system,
};

impl SqlWindow {
    /// The Ask row and the question's progress; its SQL opens in a new tab (R-SQL-44).
    pub(crate) fn assistant_row(&mut self, ui: &mut Ui) {
        if self.assistant.row(ui) {
            let release = self
                .connected
                .as_ref()
                .map_or("", |c| c.release.as_str())
                .to_owned();
            let (schema, libraries) = self.job.as_ref().map_or_else(Default::default, |j| {
                (
                    j.schema.clone(),
                    j.libraries
                        .iter()
                        .map(|(l, _)| l.clone())
                        .collect::<Vec<_>>(),
                )
            });
            self.assistant.ask(ui.ctx(), &release, &schema, &libraries);
        }
        if let Some(answer) = self
            .assistant
            .step(ui.ctx(), &mut self.browser, &mut self.database)
        {
            self.scripts
                .open_new("Assistant", &assistant::script(&answer));
        }
        ui.add_space(4.0);
    }

    /// Offers names for what is being typed, from the schema browser (R-SQL-36).
    pub(crate) fn complete(&mut self) {
        let Some((typed, statement)) = self.scripts.completing() else {
            return;
        };
        let (schema, libraries) = self
            .job
            .as_ref()
            .map(|j| (Some(j.schema.clone()), j.libraries.clone()))
            .unwrap_or_default();
        let found = self.browser.candidates(
            &typed,
            &statement,
            schema.as_deref(),
            &libraries,
            &mut self.database,
        );
        // Snippets by name, but not for a column after `alias.` (R-SQL-64).
        let snippets = if typed.qualifier.is_none() {
            self.snippets.completions(&typed.prefix)
        } else {
            Vec::new()
        };
        self.scripts.offer(editor::arrange(found, snippets));
    }

    /// The parameters of the routine called at the cursor, from the catalog (R-SQL-65).
    pub(crate) fn signatures(&mut self, ctx: &egui::Context) {
        let found = self.scripts.calling(ctx).and_then(|call| {
            let schema = self.job.as_ref().map(|j| j.schema.clone());
            let signatures = self
                .browser
                .signatures(
                    call.qualifier.as_deref(),
                    &call.name,
                    schema.as_deref(),
                    &mut self.database,
                )?
                .to_vec();
            Some((call, signatures))
        });
        self.scripts.set_signatures(found);
    }

    /// What hovering a name in the editor shows, from the schema browser (R-SQL-37).
    pub(crate) fn describe(&mut self) {
        let Some(((qualifier, name), statement)) = self.scripts.pointed() else {
            return;
        };
        let schema = self.job.as_ref().map(|j| j.schema.clone());
        let text = self.browser.describe(
            qualifier.as_deref(),
            &name,
            &statement,
            schema.as_deref(),
            &mut self.database,
        );
        self.scripts.describe((qualifier, name), text);
    }

    /// What was picked in the schema browser (R-SQL-34, R-SQL-35).
    pub(crate) fn use_pick(&mut self, pick: browser::Pick) {
        match pick {
            browser::Pick::Insert(text) => self.scripts.insert(&text),
            browser::Pick::Download { library, name } => {
                self.transfer.ask(download::Source::Table { library, name });
            }
            browser::Pick::Upload { library, name } => self.upload.ask(&library, &name),
            browser::Pick::System(system::SystemPick::Show(view)) => {
                self.system.open_tab(view);
                self.tab = Tab::View(view);
                if !self.system.is_read() {
                    self.system.read(&mut self.database);
                }
            }
            browser::Pick::System(system::SystemPick::Refresh) => {
                self.system.read(&mut self.database);
            }
            browser::Pick::Backup(backup) => self.start_backup(backup),
            browser::Pick::Failed(why) => self.fail(why),
            browser::Pick::Script { title, text, run } => {
                self.scripts.open_new(&title, &text);
                if run {
                    self.run_now(&text);
                }
            }
        }
    }

    /// Offers the values form the libraries or tables its field wants (R-SQL-115).
    pub(crate) fn offer_placeholder_names(&mut self) {
        let Some(wanted) = self.snippets.names_wanted() else {
            return;
        };
        let (schema, libraries) = self
            .job
            .as_ref()
            .map(|j| (Some(j.schema.clone()), j.libraries.clone()))
            .unwrap_or_default();
        let names = match wanted {
            snippets::Wanted::Libraries { prefix } => {
                self.browser.library_names(&prefix, &libraries)
            }
            snippets::Wanted::Tables { library, prefix } => library
                .or(schema)
                .map(|l| self.browser.table_names(&l, &prefix, &mut self.database))
                .unwrap_or_default(),
        };
        self.snippets.offer_names(names);
    }

    /// A snippet into the editor at the cursor, or run (R-SQL-25).
    pub(crate) fn use_snippet(&mut self, used: snippets::Use) {
        let report = self.snippets.take_report();
        match used {
            snippets::Use::Insert(sql) => self.scripts.insert(&sql),
            snippets::Use::Run(sql) => {
                let idle = !self.busy();
                self.run_now(&sql);
                // R-SQL-117: its results are named after the snippet.
                if idle && let Some(report) = report {
                    let statements = text::statements(&sql)
                        .into_iter()
                        .filter_map(|r| sql.get(r).map(str::to_owned))
                        .collect();
                    self.snippet_run = Some((statements, report));
                }
            }
        }
    }

    /// Runs the statements of `sql` as Run all does, unless a statement is running.
    pub(crate) fn run_now(&mut self, sql: &str) {
        if self.busy() {
            self.error_line = Some(WAIT.to_owned());
            return;
        }
        self.snippet_run = None;
        self.queue.extend(
            text::statements(sql)
                .into_iter()
                .filter_map(|r| sql.get(r).map(str::to_owned)),
        );
        self.next_statement();
    }

    /// The History tab (R-SQL-89..91).
    pub(crate) fn history(&mut self, ui: &mut Ui, entries: &[parley_sql_core::history::Entry]) {
        for action in self.history_list.show(ui, entries) {
            match action {
                history::HistoryAction::Append(sql) => self.scripts.append(&sql),
                history::HistoryAction::Insert(sql) => self.scripts.insert(&sql),
                history::HistoryAction::NewTab(sql) => self.scripts.open_new("History", &sql),
                history::HistoryAction::Run(sql) => self.run_now(&sql),
                history::HistoryAction::SaveSnippet(sql) => {
                    self.snippets.save_as(&sql);
                    self.show_snippets();
                }
                history::HistoryAction::Remove(_) | history::HistoryAction::Clear => {
                    self.history_edits.push(action);
                }
            }
        }
    }
}
