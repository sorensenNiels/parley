//! The About page both programs show (R-UI-11, R-UI-12): what this build is, looking for a new
//! version, and what Parley includes. As Tortuga's About page.

use egui::{RichText, Ui};

use super::layout::group;
use crate::menu::Program;
use crate::theme;
use crate::updates::UpdateStatus;

/// Where the source and the issues are.
const REPOSITORY: &str = "https://github.com/sorensenNiels/parley";

/// Something a program ships beside itself, for the About page's Includes (R-SSH-66): its name with its version, a
/// line about it (what it is for, its licence), and where its source is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Included {
    pub name: String,
    pub about: String,
    pub source: String,
}

/// The About page's own state.
#[derive(Debug, Default)]
pub struct About {
    /// "Check now" was clicked; the program starts the check.
    check_requested: bool,
    /// What was last copied, to say so.
    copied: Option<String>,
}

impl About {
    /// True once after "Check now" was clicked.
    pub fn take_check_request(&mut self) -> bool {
        std::mem::take(&mut self.check_requested)
    }

    /// The page for `program`, with its switch for looking for new versions.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        program: Program,
        check_for_updates: &mut bool,
        updates: &UpdateStatus,
    ) {
        self.show_with(ui, program, (check_for_updates, updates), &[]);
    }

    /// The page, with what `program` ships beside itself among the Includes.
    pub fn show_with(
        &mut self,
        ui: &mut Ui,
        program: Program,
        (check_for_updates, updates): (&mut bool, &UpdateStatus),
        included: &[Included],
    ) {
        let p = theme::palette(ui);
        let name = program.name();
        ui.label(theme::semibold(name).size(18.0));
        ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(p.muted));
        ui.add_space(14.0);
        group(ui, "About", None, |rows| {
            rows.fact("Built for", |ui| {
                ui.label(format!(
                    "{} {}",
                    std::env::consts::OS,
                    std::env::consts::ARCH
                ));
            });
            rows.fact("Licence", |ui| {
                ui.label(env!("CARGO_PKG_LICENSE").replace(" OR ", " or "));
            });
            rows.fact("Source", |ui| {
                ui.hyperlink(REPOSITORY);
            });
            rows.full_row(|ui| {
                if ui
                    .button("Copy version")
                    .on_hover_text(version_info())
                    .clicked()
                {
                    ui.ctx().copy_text(version_info());
                    self.copied = Some("The version is copied.".to_owned());
                }
            });
        });
        if *updates == UpdateStatus::Unsupported {
            theme::hint(ui, "This build doesn't look for new versions.");
        } else {
            group(ui, "Updates", None, |rows| {
                rows.switch(
                    &format!("Look for a new version when {name} starts"),
                    Some("Asks GitHub for the latest release; sends nothing else."),
                    check_for_updates,
                );
                rows.full_row(|ui| self.update_row(ui, updates));
            });
        }
        if let Some(copied) = &self.copied {
            theme::hint(ui, copied);
        }
        // R-SQL-13, D20: the IBM Public License asks for IBM's copyright in plain view. Both
        // programs ship in one installer, with the SQL helper.
        group(ui, "Includes", None, |rows| {
            rows.fact("JTOpen 21.0.7", |ui| {
                ui.label(
                    "For the SQL add-on. Copyright © 1997-2026 International Business Machines \
                     Corporation and others. All Rights Reserved. IBM Public License 1.0.",
                );
                ui.hyperlink_to("Its source", "https://github.com/IBM/JTOpen");
            });
            for item in included {
                rows.fact(&item.name, |ui| {
                    ui.label(&item.about);
                    ui.hyperlink_to("Its source", &item.source);
                });
            }
        });
    }

    /// "Check now" and what the last check found.
    fn update_row(&mut self, ui: &mut Ui, updates: &UpdateStatus) {
        ui.horizontal(|ui| {
            let checking = *updates == UpdateStatus::Checking;
            if ui
                .add_enabled(!checking, egui::Button::new("Check now"))
                .clicked()
            {
                self.check_requested = true;
            }
            match updates {
                UpdateStatus::Unsupported | UpdateStatus::NotChecked | UpdateStatus::Failed(_) => {}
                UpdateStatus::Checking => {
                    ui.spinner();
                    ui.label("Looking for a new version…");
                }
                UpdateStatus::UpToDate => {
                    ui.label("Parley is up to date.");
                }
                UpdateStatus::Available(release) => {
                    ui.hyperlink_to(
                        format!("Parley {} is available", release.version),
                        &release.page,
                    )
                    .on_hover_text("Opens the release page, with its notes and installers.");
                    let packages = if cfg!(target_os = "linux") {
                        release.linux_packages()
                    } else {
                        Vec::new()
                    };
                    if let Some(command) = download_command_menu(ui, &packages) {
                        ui.ctx().copy_text(command.clone());
                        self.copied = Some(format!("Copied: {command}"));
                    }
                }
            }
        });
        if let UpdateStatus::Failed(e) = updates {
            theme::error(ui, e);
        }
    }
}

/// The app's version and platform, for bug reports.
fn version_info() -> String {
    format!(
        "Parley {} ({} {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

/// "Copy download command", listing `packages` by file name; returns the command picked.
/// Nothing when there are none (R-UI-12).
fn download_command_menu(
    ui: &mut Ui,
    packages: &[&parley_core::update::Download],
) -> Option<String> {
    if packages.is_empty() {
        return None;
    }
    ui.menu_button("Copy download command", |ui| {
        let mut picked = None;
        for package in packages {
            let command = package.curl_command();
            if ui
                .button(&package.name)
                .on_hover_text(command.as_str())
                .clicked()
            {
                picked = Some(command);
            }
        }
        picked
    })
    .inner
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn a_program_lists_what_it_ships_beside_itself_r_ssh_66() {
        let included = vec![Included {
            name: "rsync 3.4.1".into(),
            about: "For transfers. GNU GPL version 3; æøå.".into(),
            source: "https://rsync.samba.org".into(),
        }];
        let mut about = About::default();
        let mut check = true;
        let mut harness = Harness::new_ui(|ui| {
            crate::theme::install(ui.ctx(), crate::fonts::DEFAULT_TERMINAL_FONT);
            about.show_with(
                ui,
                Program::ParleySsh,
                (&mut check, &UpdateStatus::Unsupported),
                &included,
            );
        });
        harness.run();
        harness.get_by_label("rsync 3.4.1");
        harness.get_by_label("For transfers. GNU GPL version 3; æøå.");
        harness.get_by_label("JTOpen 21.0.7");
    }
}
