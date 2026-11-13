//! The SQL window (R-SQL-6..9, D20): a window of its own beside the main one, where the operator
//! signs on to the session's host, writes SQL, runs it through the SQL helper and reads the
//! results. The helper runs on a thread of its own (`parley_core::sql::runner`), so the window
//! never waits for the host.

pub mod editor;
pub mod grid;
pub mod results;

use std::collections::VecDeque;
use std::path::PathBuf;

use egui::{
    CentralPanel, Key, Modifiers, Panel, RichText, ScrollArea, TextEdit, Ui, ViewportBuilder,
    ViewportCommand, ViewportId,
};
use parley_core::SessionConfig;
use parley_core::password::Password;
use parley_core::sql::helper::HelperFiles;
use parley_core::sql::protocol::{
    self, Connect, Connected, Done, Hello, MoreRows, Outcome, Request,
};
use parley_core::sql::runner::{Event, Runner};
use parley_core::sql::text::{self, Risk};

use grid::ResultView;

use crate::{dialog, icons, theme};

/// Files the SQL window reads and writes through the system's file window, in the app
/// (R-SQL-8, R-SQL-21). `Ok(None)` is the operator cancelling.
pub trait SqlFiles: std::fmt::Debug {
    /// Saves an export where the operator chooses; the path saved to.
    fn export_csv(&self, suggested: &str, bytes: &[u8]) -> Result<Option<String>, String>;
    /// A `.sql` file the operator picks: its path and text.
    fn open_script(&self) -> Result<Option<(String, String)>, String>;
    /// Saves `text` to `path`, or where the operator chooses when `path` is `None`; the path.
    fn save_script(
        &self,
        path: Option<&str>,
        suggested: &str,
        text: &str,
    ) -> Result<Option<String>, String>;
}

/// How far the window is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    SignIn,
    Connecting,
    Ready,
}

/// What the request in flight was.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Waiting {
    Hello,
    Connect,
    Run(String),
    /// More rows from this cursor.
    Fetch(u64),
    Autocommit(bool),
    Commit,
    Rollback,
}

/// Results or messages under the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Results,
    Messages,
}

/// The answer to the question asked on closing with uncommitted changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Closing {
    Open,
    /// Asking Commit, Roll back or Cancel.
    Asking,
    /// Waiting for a commit or rollback before closing.
    AfterCommit,
    Closed,
}

/// The SQL window for one session.
#[derive(Debug)]
pub struct SqlWindow {
    session: String,
    config: SessionConfig,
    java: PathBuf,
    files: HelperFiles,
    stage: Stage,
    pub user: String,
    password: String,
    runner: Option<Runner>,
    connected: Option<Connected>,
    /// The scripts in the editor (R-SQL-20).
    scripts: editor::Scripts,
    /// Statements still to run (Run all).
    queue: VecDeque<String>,
    waiting: Option<(u64, Waiting)>,
    results: results::Results,
    tab: Tab,
    messages: Vec<(bool, String)>,
    error_line: Option<String>,
    autocommit: bool,
    uncommitted: bool,
    confirm: Option<(String, Risk)>,
    closing: Closing,
    /// Statements run since the last [`SqlWindow::take_ran`], for the history.
    ran: Vec<String>,
    /// The sign-on, sent once the helper's hello is read; it holds the password until then.
    pending_connect: Option<Request>,
    /// Rows fetched at a time, and seconds before a statement is stopped (R-SQL-18).
    rows: u32,
    timeout: u32,
}

impl SqlWindow {
    /// The window for `session` on `config`'s host, run with `java` and the helper in `files`;
    /// `user` is the one typed last for this session in this run of Parley.
    pub fn open(
        session: &str,
        config: &SessionConfig,
        java: PathBuf,
        files: HelperFiles,
        user: &str,
        scripts: &[editor::SavedScript],
    ) -> Self {
        Self {
            session: session.to_owned(),
            config: config.clone(),
            java,
            files,
            stage: Stage::SignIn,
            user: user.to_owned(),
            password: String::new(),
            runner: None,
            connected: None,
            scripts: editor::Scripts::restore(scripts),
            queue: VecDeque::new(),
            waiting: None,
            results: results::Results::default(),
            tab: Tab::Results,
            messages: Vec::new(),
            error_line: None,
            autocommit: true,
            uncommitted: false,
            confirm: None,
            closing: Closing::Open,
            ran: Vec::new(),
            pending_connect: None,
            rows: protocol::FIRST_ROWS,
            timeout: 0,
        }
    }

    /// Fetches `rows` at a time and stops a statement after `timeout` seconds (0: never).
    pub const fn with_limits(mut self, rows: u32, timeout: u32) -> Self {
        self.rows = rows;
        self.timeout = timeout;
        self
    }

    /// The scripts as the settings keep them, and a count that changes with them (R-SQL-20).
    pub fn scripts(&self) -> (u64, Vec<editor::SavedScript>) {
        (self.scripts.generation(), self.scripts.to_saved())
    }

    /// Changes to the scripts so far: when it moves on, [`SqlWindow::scripts`] is worth keeping.
    pub const fn scripts_generation(&self) -> u64 {
        self.scripts.generation()
    }

    /// The statements run since last asked, for the history (R-SQL-7).
    pub fn take_ran(&mut self) -> Vec<String> {
        std::mem::take(&mut self.ran)
    }

    /// Closes the window, asking first if changes are not committed (R-SQL-5); the session
    /// disconnected, or the operator closed it.
    pub fn request_close(&mut self) {
        if self.closing == Closing::Open {
            self.closing = if self.uncommitted {
                Closing::Asking
            } else {
                Closing::Closed
            };
        }
    }

    /// Draws the window; false once it is closed.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        history: &[String],
        saver: Option<&dyn SqlFiles>,
    ) -> bool {
        self.read_events();
        if self.closing == Closing::Closed {
            self.finish();
            return false;
        }
        let title = format!("SQL – {}", self.session);
        let builder = ViewportBuilder::default()
            .with_title(&title)
            .with_inner_size([980.0, 660.0])
            .with_min_inner_size([560.0, 360.0]);
        ctx.show_viewport_immediate(
            ViewportId::from_hash_of("parley-sql"),
            builder,
            |ui, _class| {
                if ui.ctx().input(|i| i.viewport().close_requested())
                    && self.closing == Closing::Open
                {
                    if self.uncommitted {
                        ui.ctx().send_viewport_cmd(ViewportCommand::CancelClose);
                    }
                    self.request_close();
                }
                self.ui(ui, history, saver);
            },
        );
        if self.closing == Closing::Closed {
            self.finish();
            return false;
        }
        true
    }

    fn ui(&mut self, ui: &mut Ui, history: &[String], saver: Option<&dyn SqlFiles>) {
        let top = theme::bar(ui, egui::Margin::symmetric(12, 8));
        Panel::top("sql-top")
            .frame(top)
            .show(ui, |ui| self.top_bar(ui));
        match self.stage {
            Stage::SignIn | Stage::Connecting => {
                CentralPanel::default().show(ui, |ui| self.sign_in(ui));
            }
            Stage::Ready => {
                Panel::right("sql-history")
                    .resizable(true)
                    .default_size(230.0)
                    .show(ui, |ui| self.history(ui, history));
                CentralPanel::default().show(ui, |ui| self.work(ui, saver));
            }
        }
        self.confirm_dialog(ui.ctx());
        self.closing_dialog(ui.ctx());
    }

    // ---------------------------------------------------------------- the top bar

    fn top_bar(&mut self, ui: &mut Ui) {
        let p = theme::palette(ui);
        ui.horizontal(|ui| {
            ui.label(icons::icon(icons::DATABASE).color(p.muted));
            match &self.connected {
                Some(c) => {
                    ui.label(theme::semibold(&c.user));
                    ui.label(
                        RichText::new(format!(
                            "{} · {} · job {}",
                            self.config.host, c.release, c.job
                        ))
                        .color(p.muted),
                    );
                }
                None => {
                    ui.label(RichText::new(&self.config.host).color(p.muted));
                }
            }
            theme::button_row(ui, |ui| {
                if ui.button("Close").clicked() {
                    self.request_close();
                }
                if self.stage == Stage::Ready {
                    self.commit_controls(ui);
                }
            });
        });
        if !self.config.tls {
            theme::warning(
                ui,
                "Not encrypted: this host's connection has no TLS, so statements and results travel as plain text.",
            );
        }
    }

    /// Autocommit, and Commit and Rollback while it is off (R-SQL-5).
    fn commit_controls(&mut self, ui: &mut Ui) {
        let idle = self.waiting.is_none();
        if !self.autocommit {
            let commit = ui.add_enabled(idle && self.uncommitted, egui::Button::new("Commit"));
            if commit.clicked() {
                self.send(Request::Commit, Waiting::Commit);
            }
            if ui
                .add_enabled(idle && self.uncommitted, egui::Button::new("Rollback"))
                .clicked()
            {
                self.send(Request::Rollback, Waiting::Rollback);
            }
        }
        let mut on = self.autocommit;
        let switch = ui
            .add_enabled_ui(idle && !self.uncommitted, |ui| theme::switch(ui, &mut on))
            .inner;
        let label = ui.label("Autocommit");
        switch
            .labelled_by(label.id)
            .on_hover_text("Off: changes wait for Commit or Rollback");
        if on != self.autocommit {
            self.send(Request::Autocommit { on }, Waiting::Autocommit(on));
        }
    }

    // ---------------------------------------------------------------- signing in

    fn sign_in(&mut self, ui: &mut Ui) {
        let connecting = self.stage == Stage::Connecting;
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(380.0);
            theme::card(ui).show(ui, |ui| {
                ui.label(theme::semibold(format!("Sign on to {}", self.config.host)));
                theme::hint(
                    ui,
                    "Parley keeps no password: it is asked each time, and used once.",
                );
                ui.add_space(8.0);
                egui::Grid::new("sql-sign-in")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        let user = ui.label("User");
                        theme::sized_edit(ui, 220.0, TextEdit::singleline(&mut self.user))
                            .labelled_by(user.id);
                        ui.end_row();
                        let password = ui.label("Password");
                        let field = theme::sized_edit(
                            ui,
                            220.0,
                            TextEdit::singleline(&mut self.password).password(true),
                        )
                        .labelled_by(password.id);
                        ui.end_row();
                        if field.lost_focus()
                            && ui.input(|i| i.key_pressed(Key::Enter))
                            && !connecting
                        {
                            self.connect(ui.ctx());
                        }
                    });
                if let Some(error) = &self.error_line {
                    theme::error(ui, error);
                }
                ui.add_space(8.0);
                theme::button_row(ui, |ui| {
                    let ready =
                        !connecting && !self.user.trim().is_empty() && !self.password.is_empty();
                    let label = if connecting {
                        "Connecting…"
                    } else {
                        "Connect"
                    };
                    if ui
                        .add_enabled(ready, theme::primary_button(ui, label))
                        .clicked()
                    {
                        self.connect(ui.ctx());
                    }
                });
            });
        });
    }

    /// Starts the helper and signs on once; the password field is emptied at once.
    fn connect(&mut self, ctx: &egui::Context) {
        // Taken out of the field first: whatever happens next, it is not kept there.
        let connect = Request::Connect(Connect {
            host: self.config.host.clone(),
            tls: self.config.tls,
            user: self.user.trim().to_owned(),
            password: Password::new(std::mem::take(&mut self.password)),
            ca_file: self.config.ca_file.clone(),
        });
        let wake = ctx.clone();
        match Runner::start(&self.java, &self.files, move || wake.request_repaint()) {
            Ok(runner) => self.runner = Some(runner),
            Err(e) => {
                self.error_line = Some(e.to_string());
                return;
            }
        }
        self.error_line = None;
        self.stage = Stage::Connecting;
        self.send(Request::Hello, Waiting::Hello);
        // Sent once the hello shows the helper speaks this protocol.
        self.pending_connect = Some(connect);
    }

    // ---------------------------------------------------------------- the history

    fn history(&mut self, ui: &mut Ui, history: &[String]) {
        theme::section_label(ui, "History");
        if history.is_empty() {
            theme::hint(ui, "Statements you run are listed here.");
            return;
        }
        ScrollArea::vertical()
            .id_salt("sql-history")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for statement in history.iter().rev() {
                    let line = statement.split_whitespace().collect::<Vec<_>>().join(" ");
                    let item = egui::Button::new(RichText::new(line).monospace().small())
                        .truncate()
                        .frame_when_inactive(false);
                    if ui
                        .add_sized([ui.available_width(), 20.0], item)
                        .on_hover_text(statement)
                        .clicked()
                    {
                        self.scripts.append(statement);
                    }
                }
            });
    }

    // ---------------------------------------------------------------- writing and running

    fn work(&mut self, ui: &mut Ui, saver: Option<&dyn SqlFiles>) {
        self.keys(ui);
        let height = (ui.available_height() * 0.38).clamp(90.0, 320.0);
        self.scripts.show(ui, height, saver);
        ui.add_space(6.0);
        self.run_buttons(ui);
        if let Some(error) = &self.error_line {
            theme::error(ui, error);
        }
        ui.add_space(4.0);
        theme::segmented(
            ui,
            &mut self.tab,
            &[(Tab::Results, "Results"), (Tab::Messages, "Messages")],
        );
        ui.add_space(4.0);
        match self.tab {
            Tab::Results => self.results(ui, saver),
            Tab::Messages => self.messages(ui),
        }
    }

    /// Ctrl+Enter runs, Ctrl+Shift+Enter runs all, Esc cancels: taken before the editor sees them.
    fn keys(&mut self, ui: &Ui) {
        let (all, one, cancel) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Enter),
                i.consume_key(Modifiers::COMMAND, Key::Enter),
                // Esc closes the find bar first, when it is open.
                !self.scripts.finding() && i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if all {
            self.run_all();
        } else if one {
            self.run_current();
        }
        if cancel {
            self.cancel();
        }
    }

    fn run_buttons(&mut self, ui: &mut Ui) {
        let idle = self.waiting.is_none() && self.queue.is_empty();
        ui.horizontal(|ui| {
            let run = icons::button(icons::PLAY, "Run").shortcut_text("Ctrl+Enter");
            if icons::add_named(ui, idle, run, "Run").clicked() {
                self.run_current();
            }
            let all = egui::Button::new("Run all").shortcut_text("Ctrl+Shift+Enter");
            if icons::add_named(ui, idle, all, "Run all").clicked() {
                self.run_all();
            }
            let cancel = icons::button(icons::STOP, "Cancel").shortcut_text("Esc");
            if icons::add_named(ui, !idle, cancel, "Cancel").clicked() {
                self.cancel();
            }
            if !idle {
                ui.spinner();
                theme::hint(ui, "Running…");
            }
        });
    }

    /// The selection, or the statement under the cursor (R-SQL-7).
    fn run_current(&mut self) {
        if self.waiting.is_some() || !self.queue.is_empty() {
            return;
        }
        let (start, end) = self.scripts.selection();
        let script = self.scripts.text();
        let chosen = if start < end {
            script
                .get(start..end)
                .map(str::trim)
                .map(|s| s.trim_end_matches(';').trim().to_owned())
        } else {
            text::statement_at(script, start)
                .and_then(|r| script.get(r))
                .map(str::to_owned)
        };
        if let Some(statement) = chosen.filter(|s| !s.is_empty()) {
            self.queue.push_back(statement);
            self.next_statement();
        }
    }

    /// Every statement in the editor, in order, stopping at the first error.
    fn run_all(&mut self) {
        if self.waiting.is_some() || !self.queue.is_empty() {
            return;
        }
        let script = self.scripts.text();
        let statements: Vec<String> = text::statements(script)
            .into_iter()
            .filter_map(|r| script.get(r).map(str::to_owned))
            .collect();
        self.queue.extend(statements);
        self.next_statement();
    }

    /// Runs the next statement queued, after asking when it changes much (R-SQL-9).
    fn next_statement(&mut self) {
        if self.waiting.is_some() || self.confirm.is_some() {
            return;
        }
        let Some(statement) = self.queue.pop_front() else {
            return;
        };
        if let Some(risk) = text::risk(&statement) {
            self.confirm = Some((statement, risk));
            return;
        }
        self.start(statement);
    }

    fn start(&mut self, statement: String) {
        self.error_line = None;
        let run = self.run_request(&statement);
        self.send(run, Waiting::Run(statement));
    }

    /// The request that runs `statement`, with the rows at a time and the timeout (R-SQL-18).
    fn run_request(&self, statement: &str) -> Request {
        Request::Run {
            sql: statement.to_owned(),
            max_rows: self.rows,
            timeout: self.timeout,
        }
    }

    fn cancel(&mut self) {
        self.queue.clear();
        if let Some(runner) = &self.runner
            && self.waiting.is_some()
        {
            runner.cancel();
        }
    }

    fn send(&mut self, request: Request, waiting: Waiting) {
        if let Some(runner) = &mut self.runner {
            let tag = runner.send(request);
            self.waiting = Some((tag, waiting));
        }
    }

    // ---------------------------------------------------------------- results

    fn results(&mut self, ui: &mut Ui, saver: Option<&dyn SqlFiles>) {
        let idle = self.waiting.is_none();
        if let Some(results::Ask::Fetch(cursor)) = self.results.show(ui, idle, saver) {
            let fetch = Request::Fetch {
                cursor,
                max_rows: self.rows,
            };
            self.send(fetch, Waiting::Fetch(cursor));
        }
    }

    fn messages(&self, ui: &mut Ui) {
        if self.messages.is_empty() {
            theme::hint(ui, "Row counts, warnings and errors appear here.");
            return;
        }
        ScrollArea::vertical()
            .id_salt("sql-messages")
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for (error, text) in &self.messages {
                    if *error {
                        theme::error(ui, text);
                    } else {
                        ui.label(RichText::new(text).monospace().small());
                    }
                }
            });
    }

    // ---------------------------------------------------------------- answers

    /// Takes what the helper answered since the last frame.
    fn read_events(&mut self) {
        while let Some(event) = self.runner.as_ref().and_then(Runner::try_event) {
            self.handle(event);
        }
    }

    fn handle(&mut self, event: Event) {
        let Some((tag, waiting)) = self.waiting.take() else {
            return;
        };
        if event.tag != tag && event.tag != 0 {
            self.waiting = Some((tag, waiting));
            return;
        }
        match waiting {
            Waiting::Hello => self.on_hello(event),
            Waiting::Connect => self.on_connect(event),
            Waiting::Run(statement) => self.on_run(&statement, event),
            Waiting::Fetch(cursor) => match event
                .result
                .and_then(parley_core::sql::protocol::Answer::into_result::<MoreRows>)
            {
                Ok(more) => self.results.append(cursor, more.rows, more.more),
                Err(e) => self.fail(e.to_string()),
            },
            Waiting::Autocommit(on) => match event
                .result
                .and_then(parley_core::sql::protocol::Answer::into_result::<Done>)
            {
                Ok(_) => self.autocommit = on,
                Err(e) => self.fail(e.to_string()),
            },
            Waiting::Commit | Waiting::Rollback => {
                let done = if waiting == Waiting::Commit {
                    "Committed."
                } else {
                    "Rolled back."
                };
                match event
                    .result
                    .and_then(parley_core::sql::protocol::Answer::into_result::<Done>)
                {
                    Ok(_) => {
                        self.uncommitted = false;
                        self.messages.push((false, done.to_owned()));
                    }
                    Err(e) => self.fail(e.to_string()),
                }
                if self.closing == Closing::AfterCommit {
                    self.closing = if self.uncommitted {
                        Closing::Asking
                    } else {
                        Closing::Closed
                    };
                }
            }
        }
    }

    fn on_hello(&mut self, event: Event) {
        match event
            .result
            .and_then(parley_core::sql::protocol::Answer::into_result::<Hello>)
        {
            Ok(hello) if hello.protocol == protocol::PROTOCOL => {
                if let Some(connect) = self.pending_connect.take() {
                    self.send(connect, Waiting::Connect);
                }
            }
            Ok(hello) => self.back_to_sign_in(format!(
                "The SQL helper speaks protocol {}, Parley {}: the installation is mixed.",
                hello.protocol,
                protocol::PROTOCOL
            )),
            Err(e) => self.back_to_sign_in(e.to_string()),
        }
    }

    fn on_connect(&mut self, event: Event) {
        match event
            .result
            .and_then(parley_core::sql::protocol::Answer::into_result::<Connected>)
        {
            Ok(connected) => {
                self.messages.push((
                    false,
                    format!(
                        "Connected as {} to {} (job {}).",
                        connected.user, self.config.host, connected.job
                    ),
                ));
                self.connected = Some(connected);
                self.stage = Stage::Ready;
            }
            Err(e) => self.back_to_sign_in(format!(
                "{e} Not tried again: check the user and password first."
            )),
        }
    }

    fn on_run(&mut self, statement: &str, event: Event) {
        self.ran.push(statement.to_owned());
        let short = statement.split_whitespace().collect::<Vec<_>>().join(" ");
        match event
            .result
            .and_then(parley_core::sql::protocol::Answer::into_result::<Outcome>)
        {
            Ok(Outcome::Rows {
                cursor,
                columns,
                rows,
                more,
            }) => {
                self.messages.push((
                    false,
                    format!(
                        "{short}: {} rows{}",
                        rows.len(),
                        if more { ", more to fetch" } else { "" }
                    ),
                ));
                self.results
                    .add(statement, ResultView::new(columns, rows, more, cursor));
                self.tab = Tab::Results;
            }
            Ok(Outcome::Count { count }) => {
                self.messages
                    .push((false, format!("{short}: {count} rows changed")));
                if !self.autocommit {
                    self.uncommitted = true;
                }
                if self.queue.is_empty() {
                    self.tab = Tab::Messages;
                }
            }
            Err(e) => {
                self.queue.clear();
                self.fail(format!("{short}: {e}"));
                return;
            }
        }
        self.next_statement();
    }

    fn fail(&mut self, message: String) {
        self.messages.push((true, message.clone()));
        self.error_line = Some(message);
    }

    fn back_to_sign_in(&mut self, message: String) {
        self.runner = None;
        self.pending_connect = None;
        self.stage = Stage::SignIn;
        self.error_line = Some(message);
    }

    // ---------------------------------------------------------------- questions

    /// R-SQL-9: names the statement and what it does; Run goes on, Cancel stops the queue.
    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some((statement, risk)) = self.confirm.clone() else {
            return;
        };
        let (close, (cancel, run)) = dialog::dialog(
            ctx,
            "sql-confirm",
            "Run this statement?",
            460.0,
            |ui| {
                ui.label(RichText::new(&statement).monospace());
                ui.add_space(6.0);
                theme::warning(ui, format!("It {}.", risk.describe()));
                theme::hint(
                    ui,
                    "Parley only reads the statement's first words: this guards against slips, it does not check what the statement does.",
                );
                dialog::actions(ui, theme::danger_button(ui, "Run"), true)
            },
        );
        if run {
            self.confirm = None;
            self.start(statement);
        } else if close || cancel {
            self.confirm = None;
            self.queue.clear();
        }
    }

    /// R-SQL-5: closing with changes not committed.
    fn closing_dialog(&mut self, ctx: &egui::Context) {
        if self.closing != Closing::Asking {
            return;
        }
        let (close, choice) = dialog::dialog(
            ctx,
            "sql-closing",
            "Changes are not committed",
            420.0,
            |ui| {
                ui.label("Commit them, or roll them back, before the SQL window closes?");
                ui.add_space(12.0);
                theme::button_row(ui, |ui| {
                    if ui.add(theme::primary_button(ui, "Commit")).clicked() {
                        Some(Waiting::Commit)
                    } else if ui.button("Roll back").clicked() {
                        Some(Waiting::Rollback)
                    } else if ui.button("Cancel").clicked() {
                        Some(Waiting::Hello)
                    } else {
                        None
                    }
                })
            },
        );
        match choice {
            Some(Waiting::Commit) => {
                self.closing = Closing::AfterCommit;
                self.send(Request::Commit, Waiting::Commit);
            }
            Some(Waiting::Rollback) => {
                self.closing = Closing::AfterCommit;
                self.send(Request::Rollback, Waiting::Rollback);
            }
            Some(_) => self.closing = Closing::Open,
            None if close => self.closing = Closing::Open,
            None => {}
        }
    }

    /// Ends the helper: the connection closes with it.
    fn finish(&mut self) {
        if let Some(mut runner) = self.runner.take() {
            runner.send(Request::Close);
        }
    }
}

#[cfg(test)]
mod tests {
    use parley_core::sql::protocol::Answer;

    use super::*;

    fn window() -> SqlWindow {
        let config = SessionConfig {
            host: "ibmi".to_owned(),
            ..SessionConfig::default()
        };
        let mut w = SqlWindow::open(
            "Prod",
            &config,
            PathBuf::from("java"),
            HelperFiles::in_folder(std::path::Path::new("/none")),
            "usr",
            &[],
        );
        w.stage = Stage::Ready;
        w
    }

    /// The window as if the helper answered `json` to what it waits for.
    fn answer(w: &mut SqlWindow, json: &str) {
        let tag = w.waiting.as_ref().map_or(0, |(tag, _)| *tag);
        w.handle(Event {
            tag,
            result: Answer::parse(json),
        });
    }

    /// The window waiting for `statement`, as if it had sent it.
    fn running(w: &mut SqlWindow, statement: &str) {
        w.waiting = Some((1, Waiting::Run(statement.to_owned())));
    }

    #[test]
    fn rows_fill_the_grid_and_go_into_the_history_r_sql_8() {
        let mut w = window();
        w.tab = Tab::Messages;
        running(&mut w, "SELECT NAME FROM T");
        answer(
            &mut w,
            r#"{"id":1,"ok":{"kind":"rows","cursor":3,"columns":[{"name":"NAME","type":"VARCHAR","length":10,"scale":0,"nullable":true,"numeric":false}],"rows":[["æøå"],[null]],"more":true}}"#,
        );
        let view = w.results.shown_view().expect("a grid");
        assert_eq!(view.rows.len(), 2);
        assert_eq!(view.cursor, Some(3), "more to fetch");
        assert_eq!(w.tab, Tab::Results);
        assert_eq!(w.take_ran(), ["SELECT NAME FROM T"]);
        assert!(w.waiting.is_none());
    }

    #[test]
    fn a_change_without_autocommit_is_uncommitted_and_closing_asks_r_sql_5() {
        let mut w = window();
        w.autocommit = false;
        running(&mut w, "UPDATE T SET A = 1 WHERE B = 2");
        answer(&mut w, r#"{"id":1,"ok":{"kind":"count","count":3}}"#);
        assert!(w.uncommitted);
        assert!(w.messages.iter().any(|(_, m)| m.contains("3 rows changed")));
        w.request_close();
        assert_eq!(w.closing, Closing::Asking);
    }

    #[test]
    fn closing_with_everything_committed_closes_at_once_r_sql_5() {
        let mut w = window();
        w.request_close();
        assert_eq!(w.closing, Closing::Closed);
    }

    #[test]
    fn an_error_stops_run_all_and_shows_the_red_line_r_sql_7() {
        let mut w = window();
        w.queue
            .extend(["VALUES 2".to_owned(), "VALUES 3".to_owned()]);
        running(&mut w, "SELECT * FROM NOPE");
        answer(
            &mut w,
            r#"{"id":1,"error":{"message":"NOPE in *LIBL type *FILE not found.","sqlstate":"42704","sqlcode":-204}}"#,
        );
        assert!(w.queue.is_empty(), "the rest is not run");
        let line = w.error_line.as_deref().expect("an error line");
        assert!(line.contains("SQLSTATE 42704"), "{line}");
        assert_eq!(
            w.take_ran(),
            ["SELECT * FROM NOPE"],
            "failed statements are in the history too"
        );
    }

    #[test]
    fn a_bulk_change_waits_for_confirmation_r_sql_9() {
        let mut w = window();
        let script = "SELECT 1 FROM T; DELETE FROM T";
        w.scripts.set_text(script, script.len());
        w.run_current();
        let (statement, risk) = w.confirm.clone().expect("asked");
        assert_eq!(statement, "DELETE FROM T");
        assert_eq!(risk, Risk::DeleteWithoutWhere);
        assert!(w.waiting.is_none(), "nothing sent before the answer");
    }

    #[test]
    fn run_all_queues_every_statement_in_order_r_sql_7() {
        let mut w = window();
        w.scripts.set_text("VALUES 1;\nVALUES 'æøå';\n", 0);
        w.run_all();
        // Without a helper nothing is sent; the first statement left the queue to be run.
        assert_eq!(w.queue, ["VALUES 'æøå'"]);
    }

    #[test]
    fn a_refused_sign_on_goes_back_to_the_form_r_sql_6() {
        let mut w = window();
        w.stage = Stage::Connecting;
        w.waiting = Some((1, Waiting::Connect));
        answer(
            &mut w,
            r#"{"id":1,"error":{"message":"Password is not correct for user profile.","sqlstate":"08004","sqlcode":-99999}}"#,
        );
        assert_eq!(w.stage, Stage::SignIn);
        let line = w.error_line.as_deref().expect("why");
        assert!(line.contains("Not tried again"), "{line}");
    }

    #[test]
    fn the_password_leaves_its_field_even_when_the_helper_cannot_start_r_sql_6() {
        let config = SessionConfig::default();
        let files = HelperFiles::in_folder(std::path::Path::new("/none"));
        let mut w = SqlWindow::open(
            "Prod",
            &config,
            PathBuf::from("/no/such/java"),
            files,
            "usr",
            &[],
        );
        "TopSecret9".clone_into(&mut w.password);
        w.connect(&egui::Context::default());
        assert_eq!(w.password, "", "not kept in the field");
        assert_eq!(w.stage, Stage::SignIn);
        let line = w.error_line.as_deref().expect("why");
        assert!(!line.contains("TopSecret9"), "{line}");
    }

    #[test]
    fn character_positions_become_byte_offsets_r_sql_7() {
        assert_eq!(editor::byte_at("æøå;", 3), 6);
        assert_eq!(editor::byte_at("abc", 9), 3);
    }

    /// The window drawn alone, as in a test of the main window (embedded: kittest has one).
    fn drawn(w: SqlWindow, history: Vec<String>) -> egui_kittest::Harness<'static, SqlWindow> {
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(1100.0, 760.0))
            .with_max_steps(16)
            .build_ui_state(
                move |ui, w: &mut SqlWindow| {
                    crate::test_support::install_fonts(ui);
                    w.show(ui.ctx(), &history, None);
                },
                w,
            )
    }

    #[test]
    fn the_sign_in_form_asks_user_and_password_and_warns_without_tls_r_sql_6() {
        use egui_kittest::kittest::Queryable;
        let mut w = window();
        w.stage = Stage::SignIn;
        let mut harness = drawn(w, Vec::new());
        harness.get_by_label("User");
        harness.get_by_label("Password");
        harness.get_by_label_contains("Not encrypted");
        // Without a password Connect does nothing: no helper is started.
        harness.get_by_label("Connect").click();
        harness.run();
        assert_eq!(harness.state().stage, Stage::SignIn);
        assert!(harness.state().runner.is_none());
    }

    #[test]
    fn the_ready_window_draws_results_history_and_buttons_and_keeps_its_place_r_sql_8() {
        use egui_kittest::kittest::Queryable;
        let mut w = window();
        running(&mut w, "SELECT NAME FROM T");
        answer(
            &mut w,
            r#"{"id":1,"ok":{"kind":"rows","cursor":3,"columns":[{"name":"NAME","type":"VARCHAR","length":10,"scale":0,"nullable":true,"numeric":false},{"name":"AMOUNT","type":"DECIMAL","length":31,"scale":6,"nullable":true,"numeric":true}],"rows":[["æøå","12345678901234567890.123456"],[null,"-0.50"]],"more":true}}"#,
        );
        let mut harness = drawn(w, vec!["SELECT NAME FROM T".to_owned()]);
        for label in [
            "Run",
            "Run all",
            "Cancel",
            "Results",
            "Messages",
            "Fetch more",
            "Copy all",
            "HISTORY",
        ] {
            harness.get_by_label(label);
        }
        harness.get_by_label("æøå");
        harness.get_by_label("12345678901234567890.123456");
        let run = harness.get_by_label("Run").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Run").rect(), run);
    }

    #[test]
    fn the_rows_at_a_time_and_the_timeout_go_with_each_statement_r_sql_18() {
        let w = window().with_limits(250, 15);
        assert_eq!(
            w.run_request("VALUES 1"),
            Request::Run {
                sql: "VALUES 1".to_owned(),
                max_rows: 250,
                timeout: 15,
            }
        );
        let plain = window();
        assert_eq!(
            plain.run_request("VALUES 1"),
            Request::Run {
                sql: "VALUES 1".to_owned(),
                max_rows: protocol::FIRST_ROWS,
                timeout: 0,
            },
            "the defaults: 1000 rows, no timeout"
        );
    }

    #[test]
    fn enter_on_a_selected_row_shows_its_values_one_under_another_r_sql_15() {
        use egui_kittest::kittest::Queryable;
        let mut w = window();
        running(&mut w, "SELECT NAME FROM T");
        answer(
            &mut w,
            r#"{"id":1,"ok":{"kind":"rows","cursor":3,"columns":[{"name":"NAME","type":"VARCHAR","length":10,"scale":0,"nullable":true,"numeric":false},{"name":"NOTE","type":"VARCHAR","length":10,"scale":0,"nullable":true,"numeric":false}],"rows":[["æøå",null],["b","c"]],"more":false}}"#,
        );
        let mut harness = drawn(w, Vec::new());
        assert!(harness.query_by_label_contains("Row 1 of").is_none());
        harness.get_by_label("1").click();
        harness.run();
        let selected = harness
            .state()
            .results
            .shown_view()
            .map(ResultView::selected_count);
        assert_eq!(selected, Some(1), "a click on the row's number selects it");
        harness.key_press(egui::Key::Enter);
        harness.run();
        harness.get_by_label("Row 1 of 2");
        harness.get_by_label("Copy NAME");
        harness.get_by_label("Next row").click();
        harness.run();
        harness.get_by_label("Row 2 of 2");
    }

    #[test]
    fn ctrl_f_opens_the_find_bar_and_esc_closes_it_without_cancelling_r_sql_23() {
        use egui_kittest::kittest::Queryable;
        let mut harness = drawn(window(), Vec::new());
        assert!(harness.query_by_label("Match case").is_none());
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
        harness.run();
        harness.get_by_label("Match case");
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert!(harness.query_by_label("Match case").is_none());
        assert!(!harness.state().scripts.finding());
    }

    #[test]
    fn ctrl_t_adds_a_script_tab_r_sql_20() {
        use egui_kittest::kittest::Queryable;
        let mut harness = drawn(window(), Vec::new());
        harness.get_by_label("Script 1");
        assert!(harness.query_by_label("Script 2").is_none());
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::T);
        harness.run();
        harness.get_by_label("Script 2");
        let (_, saved) = harness.state().scripts();
        assert_eq!(saved.len(), 2, "both kept for the session");
    }
}
