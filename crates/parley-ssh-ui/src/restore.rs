//! Restore (R-SSH-52, D50): the tabs, their split panes and sizes and each pane's session are kept in the settings
//! when Parley SSH saves them, and opened again on the next start without a host on the command line; a pane
//! connects when its tab is first shown. No output and never a password is kept.

use serde::{Deserialize, Serialize};

use super::ParleySshUi;
use crate::panes::{Layout, PaneId};
use crate::sessions::SavedSession;
use crate::settings::Settings;
use crate::tabs::{Pane, Tab};

/// One tab as kept.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedTab {
    pub layout: Layout,
    pub panes: Vec<(PaneId, SavedPane)>,
    pub active: PaneId,
}

/// One pane as kept: a start screen, or what its session was opened from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SavedPane {
    Start,
    Session {
        source: Box<SavedSession>,
        saved: bool,
    },
}

impl SavedTab {
    fn of(tab: &Tab) -> Self {
        let panes = tab
            .panes
            .iter()
            .map(|(id, pane)| {
                let kept = match pane {
                    Pane::Start(_) => SavedPane::Start,
                    Pane::Session(s) => SavedPane::Session {
                        source: Box::new(s.source.clone()),
                        saved: s.saved,
                    },
                };
                (*id, kept)
            })
            .collect();
        Self {
            layout: tab.layout.clone(),
            panes,
            active: tab.active,
        }
    }
}

impl ParleySshUi {
    /// The settings to keep: with the tabs open now when restore is on, else with none.
    pub fn settings_to_save(&self) -> Settings {
        let mut settings = self.settings.clone();
        settings.last_tabs = if settings.restore_tabs {
            self.tabs.iter().map(SavedTab::of).collect()
        } else {
            Vec::new()
        };
        settings
    }

    /// Opens the tabs of last time in place of the start screen, when restore is on and there are any.
    pub(super) fn restore_last_tabs(&mut self) {
        if !self.settings.restore_tabs || self.settings.last_tabs.is_empty() {
            return;
        }
        let kept = std::mem::take(&mut self.settings.last_tabs);
        let mut tabs = Vec::new();
        for saved in kept {
            if let Some(tab) = self.restored(saved) {
                tabs.push(tab);
            }
        }
        if !tabs.is_empty() {
            self.tabs = tabs;
            self.current = 0;
        }
    }

    /// A tab from what was kept, its panes numbered anew; `None` when its layout and panes do not agree.
    fn restored(&mut self, saved: SavedTab) -> Option<Tab> {
        let ids = saved.layout.panes();
        if ids.is_empty()
            || ids
                .iter()
                .any(|id| !saved.panes.iter().any(|(p, _)| p == id))
        {
            return None;
        }
        let mut layout = saved.layout;
        let mut first = None;
        let mut tab: Option<Tab> = None;
        let mut restore = Vec::new();
        let mut renumbered = Vec::new();
        for (old, kept) in saved.panes {
            if !ids.contains(&old) {
                continue;
            }
            self.next_pane += 1;
            let id = self.next_pane;
            renumbered.push((old, id));
            if let SavedPane::Session { source, saved } = kept {
                // A saved session as it is now; one since deleted opens as it was.
                let current = saved
                    .then(|| {
                        self.settings
                            .sessions
                            .iter()
                            .find(|s| s.name == source.name)
                    })
                    .flatten()
                    .cloned();
                let found = current.is_some();
                restore.push((id, current.unwrap_or(*source), saved && found));
            }
            let pane = Pane::Start(crate::start::Form::from_settings(&self.settings));
            match &mut tab {
                None => {
                    tab = Some(Tab::new(id, pane));
                    first = Some(id);
                }
                Some(t) => t.panes.push((id, pane)),
            }
        }
        let mut tab = tab?;
        layout.renumber(&renumbered);
        tab.layout = layout;
        tab.active = renumbered
            .iter()
            .find(|(old, _)| *old == saved.active)
            .map(|(_, new)| *new)
            .or(first)?;
        tab.restore = restore;
        Some(tab)
    }

    /// Connects the restored panes of the tab in front, the first time it is shown.
    pub(super) fn connect_restored(&mut self, ctx: &egui::Context) {
        let Some(tab) = self.tabs.get_mut(self.current) else {
            return;
        };
        for (id, source, saved) in std::mem::take(&mut tab.restore) {
            self.connect_pane(ctx, id, source, saved);
        }
    }
}
