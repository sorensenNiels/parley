//! Settings ▸ SQL assistant (R-SQL-48, D27), in both programs: the switch that turns the
//! assistant on (R-ADD-3), the provider, its model and address, and its API key, which goes
//! into the operating system's keychain at once and never into the settings.

use std::sync::Arc;

use egui::{ComboBox, TextEdit, Ui};
use parley_core::sql::assistant::{AssistantConfig, Provider};

use super::{Settings, group};
use crate::addons::AddOn;
use crate::sql::assistant::{ApiKeys, KeySource, disclosure, find_key};
use crate::theme;

/// The page's own state: the key being typed and where the provider's key comes from.
#[derive(Default)]
pub(super) struct Page {
    keys: Option<Arc<dyn ApiKeys>>,
    /// A key typed and not yet saved.
    draft: String,
    /// Where the key of the provider shown comes from, looked up once per provider.
    found: Option<(Provider, Result<KeySource, String>)>,
    /// What the last Save key or Remove did: whether it failed, and the text.
    message: Option<(bool, String)>,
}

impl std::fmt::Debug for Page {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Page")
            .field("keys", &self.keys)
            .field("draft", &if self.draft.is_empty() { "" } else { "hidden" })
            .field("found", &self.found)
            .finish_non_exhaustive()
    }
}

/// The settings this page restores: off, and the provider's defaults. A saved key stays.
pub(super) fn restore_defaults(settings: &mut Settings) {
    settings
        .addons
        .set_enabled(AddOn::Assistant, AddOn::Assistant.on_by_default());
    settings.assistant = AssistantConfig::default();
}

impl Page {
    pub(super) fn new(keys: Option<Arc<dyn ApiKeys>>) -> Self {
        Self {
            keys,
            ..Self::default()
        }
    }

    /// The page; `needs_sql` when Parley's SQL add-on, which the assistant lives in, is off.
    pub(super) fn show(&mut self, ui: &mut Ui, settings: &mut Settings, needs_sql: bool) {
        let note = (
            "Ask for SQL in plain words in the SQL window.",
            "The model looks up the tables and columns it needs through the catalog; its SQL \
             opens in a new script tab and nothing runs by itself.",
        );
        group(ui, "SQL assistant", Some(note), |rows| {
            let addon = AddOn::Assistant;
            let mut on = settings.addons.enabled(addon);
            rows.switch(addon.name(), Some(addon.about()), &mut on);
            settings.addons.set_enabled(addon, on);
            if needs_sql {
                rows.full_row(|ui| {
                    theme::warning(ui, "Turn on the SQL add-on in Add-ons too.");
                });
            }
        });
        let config = &mut settings.assistant;
        let before = config.provider;
        group(ui, "Provider", None, |rows| {
            rows.row("Provider", None, |ui, name_id| {
                ComboBox::from_id_salt("assistant-provider")
                    .selected_text(config.provider.label())
                    .width(260.0)
                    .show_ui(ui, |ui| {
                        for provider in Provider::ALL {
                            ui.selectable_value(&mut config.provider, provider, provider.label());
                        }
                    })
                    .response
                    .labelled_by(name_id);
            });
            let model_hint = match config.provider.default_model() {
                "" => "The server's own",
                model => model,
            };
            rows.row("Model", Some("Empty: the default."), |ui, name_id| {
                let edit = TextEdit::singleline(&mut config.model).hint_text(model_hint);
                theme::sized_edit(ui, 260.0, edit).labelled_by(name_id);
            });
            if config.provider.has_url() {
                let hint = match config.provider.default_url() {
                    "" => "https://host/v1",
                    url => url,
                };
                rows.row(
                    "Address",
                    Some("The server's address, up to /v1."),
                    |ui, name_id| {
                        let edit = TextEdit::singleline(&mut config.url).hint_text(hint);
                        theme::sized_edit(ui, 260.0, edit).labelled_by(name_id);
                    },
                );
            }
            if config.provider == Provider::LlamaCpp {
                rows.full_row(|ui| {
                    theme::hint(
                        ui,
                        "Start llama-server with --jinja so the model can look up tables, \
                         e.g. llama-server -m model.gguf --jinja --port 8080.",
                    );
                });
            }
        });
        if config.provider != before {
            self.draft.clear();
            self.message = None;
        }
        self.key_group(ui, config);
    }

    /// The API key: where it comes from, a field to save one in the keychain, and Remove.
    fn key_group(&mut self, ui: &mut Ui, config: &AssistantConfig) {
        let provider = config.provider;
        let found = self.found(provider).clone();
        let keys = self.keys.clone();
        let mut changed = false;
        group(ui, "API key", None, |rows| {
            rows.fact("Key", |ui| key_fact(ui, provider, &found, keys.is_some()));
            let Some(keys) = keys else {
                return;
            };
            rows.row(
                "Save a key",
                Some("Kept in this computer's keychain, not in Parley's settings."),
                |ui, name_id| {
                    let edit = TextEdit::singleline(&mut self.draft)
                        .password(true)
                        .hint_text("Paste the key");
                    theme::sized_edit(ui, 260.0, edit).labelled_by(name_id);
                    ui.horizontal(|ui| {
                        let typed = !self.draft.trim().is_empty();
                        if ui
                            .add_enabled(typed, egui::Button::new("Save key"))
                            .clicked()
                        {
                            self.message = Some(match keys.set(provider, self.draft.trim()) {
                                Ok(()) => (false, "Saved in the keychain.".to_owned()),
                                Err(e) => (true, format!("Could not save the key: {e}")),
                            });
                            self.draft.clear();
                            changed = true;
                        }
                        let saved = matches!(found, Ok(KeySource::Keychain));
                        if ui.add_enabled(saved, egui::Button::new("Remove")).clicked() {
                            self.message = Some(match keys.delete(provider) {
                                Ok(()) => (false, "Removed from the keychain.".to_owned()),
                                Err(e) => (true, format!("Could not remove the key: {e}")),
                            });
                            changed = true;
                        }
                    });
                    match &self.message {
                        Some((true, text)) => theme::error(ui, text),
                        Some((false, text)) => theme::hint(ui, text),
                        None => {}
                    }
                },
            );
            rows.fact("Sends", |ui| {
                ui.label(disclosure(config));
            });
        });
        if changed {
            self.found = None;
        }
    }

    /// Where `provider`'s key comes from, looked up when the provider changes.
    fn found(&mut self, provider: Provider) -> &Result<KeySource, String> {
        if self.found.as_ref().is_none_or(|(p, _)| *p != provider) {
            let found = find_key(provider, self.keys.as_deref()).map(|(source, _)| source);
            self.found = Some((provider, found));
        }
        &self.found.as_ref().expect("set just above").1
    }
}

/// Says where the key comes from, or that one is missing.
fn key_fact(ui: &mut Ui, provider: Provider, found: &Result<KeySource, String>, keychain: bool) {
    match found {
        Ok(KeySource::Environment(variable)) => {
            ui.label(format!("From {variable}, which comes before the keychain"));
        }
        Ok(KeySource::Keychain) => {
            ui.label("Saved in the keychain");
        }
        Ok(KeySource::None) if !provider.needs_key() => {
            theme::hint(ui, "Not needed, unless your server asks for one.");
        }
        Ok(KeySource::None) => {
            let variable = provider.key_variable().unwrap_or_default();
            let text = if keychain {
                format!("None: save one below, or set {variable}.")
            } else {
                format!("None: set {variable} (no keychain on this system).")
            };
            theme::warning(ui, text);
        }
        Err(e) => theme::error(ui, format!("Could not read the keychain: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    use super::super::{Dialog, Page as SettingsPage};
    use super::*;
    use crate::test_support::{FakeKeys, install_fonts};
    use crate::updates::UpdateStatus;

    /// Settings open on the SQL assistant's page, with a keychain in memory.
    fn on_the_page(
        provider: Provider,
        keys: &Arc<FakeKeys>,
    ) -> Harness<'static, (Option<Dialog>, Settings)> {
        let mut settings = Settings::default();
        settings.addons.set_enabled(AddOn::Sql, true);
        settings.assistant.provider = provider;
        let keys: Arc<dyn ApiKeys> = Arc::clone(keys) as Arc<dyn ApiKeys>;
        Harness::builder()
            .with_size(egui::vec2(1100.0, 800.0))
            .build_ui_state(
                move |ui, (dialog, settings): &mut (Option<Dialog>, Settings)| {
                    install_fonts(ui);
                    let dialog = dialog.get_or_insert_with(|| {
                        let mut d =
                            Dialog::open(ui.ctx(), settings).with_api_keys(Some(Arc::clone(&keys)));
                        d.page = SettingsPage::Assistant;
                        d
                    });
                    dialog.show(ui.ctx(), settings, &UpdateStatus::Unsupported);
                },
                (None, settings),
            )
    }

    #[test]
    fn the_page_turns_the_assistant_on_and_keeps_its_place_r_sql_48() {
        let keys = Arc::new(FakeKeys::default());
        let mut harness = on_the_page(Provider::Anthropic, &keys);
        harness.run();
        for label in ["Provider", "Model", "Save a key", "Sends"] {
            harness.get_by_label(label);
        }
        assert!(
            harness.query_by_label("Address").is_none(),
            "fixed for Anthropic"
        );
        assert!(
            !harness.state().1.addons.enabled(AddOn::Assistant),
            "off at first"
        );
        harness
            .get_by_role_and_label(egui::accesskit::Role::CheckBox, "SQL assistant")
            .click();
        harness.run();
        assert!(harness.state().1.addons.enabled(AddOn::Assistant));
        let before = harness.get_by_label("Save").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Save").rect(), before, "D12");
    }

    #[test]
    fn a_key_goes_to_the_keychain_and_never_into_the_settings_r_sql_48() {
        if parley_core::sql::assistant::key_from_environment(Provider::Mistral).is_some() {
            return;
        }
        let keys = Arc::new(FakeKeys::default());
        let mut harness = on_the_page(Provider::Mistral, &keys);
        harness.run();
        harness.get_by_label_contains("None: save one below, or set MISTRAL_API_KEY");
        harness.get_by_label("Save a key").click();
        harness.run();
        harness
            .get_by_role(egui::accesskit::Role::PasswordInput)
            .type_text("mk-hemmelig");
        harness.run();
        harness.get_by_label("Save key").click();
        harness.run_steps(2);
        assert_eq!(
            keys.get(Provider::Mistral).expect("read").as_deref(),
            Some("mk-hemmelig")
        );
        harness.get_by_label("Saved in the keychain.");
        harness.get_by_label("Saved in the keychain");
        let json = serde_json::to_string(&harness.state().1).expect("settings serialize");
        assert!(!json.contains("hemmelig"), "{json}");
        assert!(!format!("{:?}", harness.state().0).contains("hemmelig"));
        harness.get_by_label("Remove").click();
        harness.run_steps(2);
        assert_eq!(keys.get(Provider::Mistral).expect("read"), None);
    }

    #[test]
    fn llama_cpp_takes_an_address_and_needs_no_key_r_sql_48() {
        let keys = Arc::new(FakeKeys::default());
        let mut harness = on_the_page(Provider::LlamaCpp, &keys);
        harness.run();
        harness.get_by_label("Address");
        harness.get_by_label_contains("--jinja");
        harness.get_by_label("Not needed, unless your server asks for one.");
        harness.get_by_label_contains("to your llama-server");
    }

    #[test]
    fn restore_defaults_turns_it_off_and_forgets_the_provider_r_sql_48() {
        let mut settings = Settings::default();
        settings.addons.set_enabled(AddOn::Assistant, true);
        settings.assistant.provider = Provider::Gemini;
        SettingsPage::Assistant.restore_defaults(&mut settings);
        assert!(!settings.addons.enabled(AddOn::Assistant));
        assert_eq!(settings.assistant, AssistantConfig::default());
        settings.addons.set_enabled(AddOn::Assistant, true);
        SettingsPage::AddOns.restore_defaults(&mut settings);
        assert!(
            settings.addons.enabled(AddOn::Assistant),
            "the Add-ons page no longer has it"
        );
    }
}
