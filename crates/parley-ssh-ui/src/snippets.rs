//! Snippets (R-SSH-49, D50): commands kept by name in the settings, edited on Settings ▸ Snippets, exported and
//! imported as one JSON file, sent from the Snippets menu, the picker (Ctrl+Shift+S) or a key of their own, with
//! `{{name}}` placeholders asked for first (`parley_ssh_core::snippet`).

use egui::{Context, Key, RichText, Ui};
use parley_ssh_core::snippet::{self, Placeholder};
use parley_ui_egui::keymap::Chord;
use parley_ui_egui::settings::key_capture::{self, Pressed, Slot};
use parley_ui_egui::{dialog, theme};
use serde::{Deserialize, Serialize};

/// One snippet.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Snippet {
    pub name: String,
    /// What is sent: one or more lines, with placeholders.
    pub text: String,
    /// Press Enter after it.
    pub enter: bool,
    /// Its own key as `Chord::text` writes it; empty for none.
    pub key: String,
}

impl Snippet {
    /// Its own key, if it has one that can be read.
    pub fn chord(&self) -> Option<Chord> {
        Chord::parse_exact(&self.key)
    }
}

/// Saving and opening the export file through the system's file window, which only the launcher can show.
pub trait SnippetFiles {
    /// Saves `json`; the path saved to, or `None` when the user cancelled.
    ///
    /// # Errors
    /// The file could not be written, said for the user.
    fn export(&self, json: &str) -> Result<Option<String>, String>;

    /// The text of the file the user chose, or `None` when they cancelled.
    ///
    /// # Errors
    /// The file could not be read, said for the user.
    fn import(&self) -> Result<Option<String>, String>;
}

/// The export file: a list of snippets with a version, so later formats can tell.
#[derive(Debug, Serialize, Deserialize)]
struct Exported {
    parley_ssh_snippets: u32,
    snippets: Vec<Snippet>,
}

pub fn to_json(snippets: &[Snippet]) -> String {
    let exported = Exported {
        parley_ssh_snippets: 1,
        snippets: snippets.to_vec(),
    };
    serde_json::to_string_pretty(&exported).unwrap_or_default()
}

/// Adds the snippets of an export file to `snippets`: one with a name already there replaces it. Returns how
/// many were read.
///
/// # Errors
/// The text is not an export file, said for the user.
pub fn merge(snippets: &mut Vec<Snippet>, json: &str) -> Result<usize, String> {
    let read: Exported = serde_json::from_str(json)
        .map_err(|e| format!("That file is not a Parley SSH snippets file: {e}"))?;
    let count = read.snippets.len();
    for new in read.snippets {
        match snippets.iter_mut().find(|s| s.name == new.name) {
            Some(old) => *old = new,
            None => snippets.push(new),
        }
    }
    Ok(count)
}

/// The snippets whose name or text holds `query`, ignoring case; all of them for an empty query.
pub fn matching<'a>(snippets: &'a [Snippet], query: &str) -> Vec<(usize, &'a Snippet)> {
    let query = query.trim().to_lowercase();
    snippets
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            query.is_empty()
                || s.name.to_lowercase().contains(&query)
                || s.text.to_lowercase().contains(&query)
        })
        .collect()
}

/// Settings ▸ Snippets while open: the snippet being edited, and a note after export or import.
#[derive(Debug, Default)]
pub struct Page {
    editing: Option<Editing>,
    note: Option<String>,
    /// The index whose Delete was clicked once.
    confirm_delete: Option<usize>,
}

#[derive(Debug)]
struct Editing {
    /// `None` for a new one.
    index: Option<usize>,
    snippet: Snippet,
    waiting_for_key: bool,
    error: Option<String>,
}

impl Page {
    /// The page: the list with its buttons, the editor under it, export and import.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        snippets: &mut Vec<Snippet>,
        files: Option<&dyn SnippetFiles>,
    ) {
        theme::hint(
            ui,
            "Sent from the Snippets menu, with Ctrl+Shift+S, or with a key of their own. \
             {{name}} and {{name:default}} are asked for when one is sent.",
        );
        ui.add_space(8.0);
        if snippets.is_empty() {
            theme::hint(ui, "No snippets yet.");
        }
        self.list(ui, snippets);
        self.buttons(ui, snippets, files);
        if let Some(note) = &self.note {
            theme::hint(ui, note);
        }
        if let Some(editing) = &mut self.editing {
            ui.add_space(10.0);
            match edit(ui, editing, snippets) {
                Some(true) => {
                    let new = std::mem::take(&mut editing.snippet);
                    match editing.index {
                        Some(i) if i < snippets.len() => snippets[i] = new,
                        _ => snippets.push(new),
                    }
                    self.editing = None;
                }
                Some(false) => self.editing = None,
                None => {}
            }
        }
    }

    /// Each snippet with its key and its buttons: edit, move, delete (asked twice).
    fn list(&mut self, ui: &mut Ui, snippets: &mut Vec<Snippet>) {
        let mut moved = None;
        let mut deleted = None;
        for (i, s) in snippets.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(300.0);
                    ui.label(theme::semibold(&s.name));
                    let first = s.text.lines().next().unwrap_or_default();
                    ui.label(RichText::new(first).monospace().small())
                        .on_hover_text(&s.text);
                });
                if !s.key.is_empty() {
                    theme::kbd(ui, &s.key);
                }
                theme::button_row(ui, |ui| {
                    let delete = if self.confirm_delete == Some(i) {
                        "Delete?"
                    } else {
                        "Delete"
                    };
                    if ui.add(theme::danger_button(ui, delete)).clicked() {
                        if self.confirm_delete == Some(i) {
                            deleted = Some(i);
                        } else {
                            self.confirm_delete = Some(i);
                        }
                    }
                    if parley_ui_egui::icons::add_named(
                        ui,
                        i + 1 < snippets.len(),
                        parley_ui_egui::icons::button(parley_ui_egui::icons::ARROW_DOWN, "")
                            .small(),
                        "Move down",
                    )
                    .clicked()
                    {
                        moved = Some((i, i + 1));
                    }
                    if parley_ui_egui::icons::add_named(
                        ui,
                        i > 0,
                        parley_ui_egui::icons::button(parley_ui_egui::icons::ARROW_UP, "").small(),
                        "Move up",
                    )
                    .clicked()
                    {
                        moved = Some((i, i - 1));
                    }
                    if parley_ui_egui::icons::add_named(
                        ui,
                        true,
                        parley_ui_egui::icons::button(parley_ui_egui::icons::PENCIL, "").small(),
                        &format!("Edit {}", s.name),
                    )
                    .clicked()
                    {
                        self.editing = Some(Editing {
                            index: Some(i),
                            snippet: s.clone(),
                            waiting_for_key: false,
                            error: None,
                        });
                    }
                });
            });
            ui.separator();
        }
        if let Some((a, b)) = moved {
            snippets.swap(a, b);
        }
        if let Some(i) = deleted {
            snippets.remove(i);
            self.confirm_delete = None;
        }
    }

    /// Add, and export and import when the launcher can show the file window.
    fn buttons(
        &mut self,
        ui: &mut Ui,
        snippets: &mut Vec<Snippet>,
        files: Option<&dyn SnippetFiles>,
    ) {
        ui.horizontal(|ui| {
            if ui.button("Add snippet").clicked() {
                self.editing = Some(Editing {
                    index: None,
                    snippet: Snippet::default(),
                    waiting_for_key: false,
                    error: None,
                });
            }
            if let Some(files) = files {
                if ui.button("Export…").clicked() {
                    self.note = Some(match files.export(&to_json(snippets)) {
                        Ok(Some(path)) => format!("Saved {} snippets to {path}.", snippets.len()),
                        Ok(None) => return,
                        Err(e) => e,
                    });
                }
                if ui.button("Import…").clicked() {
                    self.note = match files.import() {
                        Ok(Some(json)) => Some(match merge(snippets, &json) {
                            Ok(n) => format!(
                                "Imported {n} snippets; one with a name already here replaced it."
                            ),
                            Err(e) => e,
                        }),
                        Ok(None) => None,
                        Err(e) => Some(e),
                    };
                }
            }
        });
    }
}

/// The editor of one snippet: `Some(true)` when saved, `Some(false)` when cancelled.
fn edit(ui: &mut Ui, editing: &mut Editing, snippets: &[Snippet]) -> Option<bool> {
    theme::section_label(
        ui,
        if editing.index.is_some() {
            "Edit snippet"
        } else {
            "New snippet"
        },
    );
    egui::Grid::new("ssh-snippet-fields")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            let label = ui.label("Name");
            theme::sized_edit(
                ui,
                320.0,
                egui::TextEdit::singleline(&mut editing.snippet.name),
            )
            .labelled_by(label.id);
            ui.end_row();
            let label = ui.label("Text");
            let text = egui::TextEdit::multiline(&mut editing.snippet.text)
                .code_editor()
                .desired_rows(3)
                .hint_text("tail -f /var/log/{{service:syslog}}");
            theme::sized_edit(ui, 320.0, text).labelled_by(label.id);
            ui.end_row();
            ui.label("Key");
            ui.horizontal(|ui| {
                let shown = if editing.snippet.key.is_empty() {
                    "None"
                } else {
                    editing.snippet.key.as_str()
                };
                if key_capture::key_button(ui, editing.waiting_for_key, Slot::Add, shown) {
                    editing.waiting_for_key = true;
                }
                if editing.waiting_for_key {
                    let (pressed, _, _) = key_capture::take_key(ui, Chord::exact);
                    match pressed {
                        Some(Pressed::Chord(chord)) => {
                            editing.snippet.key = chord.text();
                            editing.waiting_for_key = false;
                        }
                        Some(Pressed::Clipboard) => editing.waiting_for_key = false,
                        None => {}
                    }
                }
                if !editing.snippet.key.is_empty() && ui.add(theme::ghost_button("Clear")).clicked()
                {
                    editing.snippet.key.clear();
                }
            });
            ui.end_row();
        });
    theme::switch_row(ui, "Press Enter after it", &mut editing.snippet.enter);
    if let Some(error) = &editing.error {
        theme::error(ui, error);
    }
    let (save, cancel) = theme::button_row(ui, |ui| {
        let save = ui.add(theme::primary_button(ui, "Save snippet")).clicked();
        let cancel = ui.button("Cancel").clicked();
        (save, cancel)
    });
    if cancel {
        return Some(false);
    }
    if save {
        editing.error = check(&editing.snippet, editing.index, snippets).err();
        return editing.error.is_none().then_some(true);
    }
    None
}

/// A snippet needs a name of its own and some text.
fn check(snippet: &Snippet, index: Option<usize>, snippets: &[Snippet]) -> Result<(), String> {
    let name = snippet.name.trim();
    if name.is_empty() {
        return Err("Give the snippet a name.".to_owned());
    }
    if snippet.text.is_empty() {
        return Err("Write what the snippet sends.".to_owned());
    }
    let taken = snippets
        .iter()
        .enumerate()
        .any(|(i, s)| Some(i) != index && s.name.trim() == name);
    if taken {
        return Err(format!("There is a snippet called \"{name}\" already."));
    }
    Ok(())
}

/// The picker (Ctrl+Shift+S): type to find, arrows to choose, Enter or a click to send.
#[derive(Debug, Default)]
pub struct Picker {
    query: String,
    selected: usize,
    focused: bool,
}

/// How a snippet dialog closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Picked {
    Cancel,
    /// The snippet with this index.
    Snippet(usize),
}

impl Picker {
    pub fn show(&mut self, ctx: &Context, snippets: &[Snippet]) -> Option<Picked> {
        let found = matching(snippets, &self.query);
        self.selected = self.selected.min(found.len().saturating_sub(1));
        let (down, up, enter) = ctx.input(|i| {
            (
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::Enter),
            )
        });
        if down && self.selected + 1 < found.len() {
            self.selected += 1;
        }
        if up {
            self.selected = self.selected.saturating_sub(1);
        }
        let (close, (cancel, clicked)) = dialog::dialog(
            ctx,
            "ssh-snippet-picker",
            "Send a snippet",
            460.0,
            |ui| {
                let search = ui
                    .horizontal(|ui| {
                        let label = ui.label("Find");
                        let edit =
                            egui::TextEdit::singleline(&mut self.query).hint_text("name or text");
                        theme::sized_edit(ui, 380.0, edit).labelled_by(label.id)
                    })
                    .inner;
                if !std::mem::replace(&mut self.focused, true) {
                    search.request_focus();
                }
                ui.add_space(6.0);
                let mut clicked = None;
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if found.is_empty() {
                            theme::hint(
                                ui,
                                if snippets.is_empty() {
                                    "No snippets yet: add them on the Snippets page of the settings."
                                } else {
                                    "No snippet matches."
                                },
                            );
                        }
                        for (row, (i, s)) in found.iter().enumerate() {
                            let text = format!(
                                "{}  —  {}",
                                s.name,
                                s.text.lines().next().unwrap_or_default()
                            );
                            if ui.selectable_label(row == self.selected, text).clicked() {
                                clicked = Some(*i);
                            }
                        }
                    });
                let cancel = theme::button_row(ui, |ui| ui.button("Cancel").clicked());
                (cancel, clicked)
            },
        );
        if close || cancel {
            return Some(Picked::Cancel);
        }
        if let Some(i) = clicked {
            return Some(Picked::Snippet(i));
        }
        if enter && let Some((i, _)) = found.get(self.selected) {
            return Some(Picked::Snippet(*i));
        }
        None
    }
}

/// The question for a snippet's placeholders, before it is sent.
#[derive(Debug)]
pub struct Fill {
    pub snippet: Snippet,
    fields: Vec<(Placeholder, String)>,
    focused: bool,
}

impl Fill {
    /// The question for `snippet`, or `None` when it has no placeholders.
    pub fn new(snippet: Snippet) -> Option<Self> {
        let fields: Vec<_> = snippet::placeholders(&snippet.text)
            .into_iter()
            .map(|p| {
                let value = p.default.clone().unwrap_or_default();
                (p, value)
            })
            .collect();
        (!fields.is_empty()).then_some(Self {
            snippet,
            fields,
            focused: false,
        })
    }

    /// The text to send once answered; `Some(None)` when cancelled, `None` while open.
    pub fn show(&mut self, ctx: &Context) -> Option<Option<String>> {
        let title = format!("Send \"{}\"", self.snippet.name);
        let (close, (cancel, send)) =
            dialog::dialog(ctx, "ssh-snippet-fill", &title, 460.0, |ui| {
                let mut enter = false;
                egui::Grid::new("ssh-snippet-fill-fields")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        for (i, (p, value)) in self.fields.iter_mut().enumerate() {
                            let label = ui.label(&p.name);
                            let edit =
                                theme::sized_edit(ui, 300.0, egui::TextEdit::singleline(value))
                                    .labelled_by(label.id);
                            if i == 0 && !std::mem::replace(&mut self.focused, true) {
                                edit.request_focus();
                            }
                            enter |= edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                            ui.end_row();
                        }
                    });
                let (cancel, send) = dialog::actions(ui, theme::primary_button(ui, "Send"), true);
                (cancel, send || enter)
            });
        if close || cancel {
            return Some(None);
        }
        send.then(|| {
            Some(snippet::fill(&self.snippet.text, |name| {
                self.fields
                    .iter()
                    .find(|(p, _)| p.name == name)
                    .map(|(_, v)| v.clone())
            }))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snippet(name: &str, text: &str) -> Snippet {
        Snippet {
            name: name.into(),
            text: text.into(),
            ..Snippet::default()
        }
    }

    #[test]
    fn snippets_go_to_a_file_and_back_replacing_those_of_the_same_name_r_ssh_49() {
        let mine = vec![snippet("logs", "journalctl -f"), snippet("disk", "df -h")];
        let json = to_json(&[snippet("disk", "df -hT"), snippet("æøå", "echo æøå")]);
        let mut snippets = mine;
        assert_eq!(merge(&mut snippets, &json), Ok(2));
        let names: Vec<_> = snippets
            .iter()
            .map(|s| (s.name.as_str(), s.text.as_str()))
            .collect();
        assert_eq!(
            names,
            [
                ("logs", "journalctl -f"),
                ("disk", "df -hT"),
                ("æøå", "echo æøå")
            ]
        );
        let error = merge(&mut snippets, "{\"not\": 1}").expect_err("not a snippets file");
        assert!(error.starts_with("That file is not"), "{error}");
    }

    #[test]
    fn the_picker_finds_by_name_or_text_ignoring_case_r_ssh_49() {
        let snippets = [snippet("Logs", "journalctl -f"), snippet("disk", "df -h")];
        let names = |q| {
            matching(&snippets, q)
                .iter()
                .map(|(_, s)| s.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(names("LOG"), ["Logs"]);
        assert_eq!(names("df"), ["disk"]);
        assert_eq!(names(" "), ["Logs", "disk"]);
        assert_eq!(names("nothing"), Vec::<String>::new());
    }

    #[test]
    fn a_snippet_needs_a_name_of_its_own_and_text_r_ssh_49() {
        let snippets = [snippet("logs", "x")];
        assert!(check(&snippet("", "x"), None, &snippets).is_err());
        assert!(check(&snippet("new", ""), None, &snippets).is_err());
        assert!(
            check(&snippet("logs", "y"), None, &snippets).is_err(),
            "taken"
        );
        assert!(
            check(&snippet("logs", "y"), Some(0), &snippets).is_ok(),
            "itself"
        );
    }

    #[test]
    fn only_a_snippet_with_placeholders_asks_r_ssh_49() {
        assert!(Fill::new(snippet("plain", "uptime")).is_none());
        let fill = Fill::new(snippet("t", "tail {{file:/var/log/syslog}} {{n}}")).expect("asks");
        assert_eq!(fill.fields.len(), 2);
        assert_eq!(fill.fields[0].1, "/var/log/syslog");
    }

    fn harness_for<S: 'static>(
        state: S,
        mut show: impl FnMut(&Context, &mut S) + 'static,
    ) -> egui_kittest::Harness<'static, S> {
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(800.0, 600.0))
            .build_ui_state(
                move |ui, s: &mut S| {
                    parley_ui_egui::test_support::install_fonts(ui);
                    show(ui.ctx(), s);
                },
                state,
            )
    }

    #[test]
    fn the_placeholder_question_sends_the_answers_and_its_buttons_stay_put_r_ssh_49() {
        use egui_kittest::kittest::Queryable;
        let fill = Fill::new(snippet("ssh", "ssh {{host}} -p {{port:22}}")).expect("asks");
        let mut h = harness_for((fill, None::<Option<String>>), |ctx, (fill, out)| {
            if let Some(done) = fill.show(ctx) {
                *out = Some(done);
            }
        });
        h.run_steps(3);
        let before = h.get_by_label("Send").rect();
        h.get_by_label("host").type_text("bøx");
        h.run_steps(3);
        assert_eq!(h.get_by_label("Send").rect(), before);
        h.get_by_label("Send").click();
        h.run_steps(2);
        assert_eq!(h.state().1, Some(Some("ssh bøx -p 22".to_owned())));
    }
}
