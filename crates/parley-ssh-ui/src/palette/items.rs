//! What the palette lists (R-SSH-48) and doing what was chosen in it.

use super::{Choice, Closed, Item};
use crate::ParleySshUi;
use crate::bars::Action;
use crate::sessions::SavedSession;

/// Every menu action with its name and shortcut (Ctrl+Shift, or Cmd on macOS, as R-SSH-7).
fn actions(mac: bool) -> Vec<(Action, &'static str, String)> {
    let ours = |key: &str| {
        if mac {
            format!("Cmd+{key}")
        } else {
            format!("Ctrl+Shift+{key}")
        }
    };
    let size = |key: &str| {
        if mac {
            format!("Cmd+{key}")
        } else {
            format!("Ctrl+{key}")
        }
    };
    vec![
        (Action::NewTab, "New tab", ours("T")),
        (
            Action::NewWindow,
            "New window",
            if mac {
                "Cmd+Alt+N".into()
            } else {
                "Ctrl+Shift+Alt+N".into()
            },
        ),
        (Action::Duplicate, "Duplicate session", ours("D")),
        (Action::SplitRight, "Split right", "Alt+Shift+=".into()),
        (Action::SplitDown, "Split down", "Alt+Shift+-".into()),
        (Action::Restart, "Restart session", ours("R")),
        (Action::Disconnect, "Disconnect", String::new()),
        (Action::Close, "Close pane or tab", ours("W")),
        (Action::EventLog, "Event log", String::new()),
        (
            Action::ToggleLog,
            "Start or stop the session log",
            String::new(),
        ),
        (
            Action::BroadcastTab,
            "Broadcast to this tab, or stop",
            ours("B"),
        ),
        (
            Action::BroadcastAll,
            "Broadcast to every tab, or stop",
            String::new(),
        ),
        (Action::Copy, "Copy", ours("C")),
        (Action::Paste, "Paste", ours("V")),
        (Action::CopyAll, "Copy all", String::new()),
        (Action::Find, "Find in the scrollback", ours("F")),
        (Action::ClearScrollback, "Clear scrollback", String::new()),
        (Action::ResetTerminal, "Reset terminal", String::new()),
        (Action::TextLarger, "Larger text", size("=")),
        (Action::TextSmaller, "Smaller text", size("-")),
        (Action::TextActualSize, "Actual text size", size("0")),
        (Action::PickSnippet, "Send a snippet…", ours("S")),
        (Action::EditSnippets, "Edit snippets…", String::new()),
        (Action::Shortcuts, "Keyboard shortcuts", String::new()),
        (Action::Settings, "Settings", String::new()),
        (Action::Exit, "Exit", String::new()),
    ]
}

impl ParleySshUi {
    /// Everything the palette lists, in its order when nothing is typed.
    pub(crate) fn palette_items(&self, mac: bool) -> Vec<Item> {
        let mut items = Vec::new();
        for (i, tab) in self.tabs.iter().enumerate() {
            items.push(Item {
                key: format!("tab:{}", tab.active),
                kind: "Tab",
                label: tab.title(),
                detail: format!("tab {}", i + 1),
                shortcut: if i < 9 {
                    if mac {
                        format!("Cmd+{}", i + 1)
                    } else {
                        format!("Ctrl+Alt+{}", i + 1)
                    }
                } else {
                    String::new()
                },
                choice: Choice::Tab(i),
            });
        }
        // The favourites first (R-SSH-63), each group in the settings' order.
        let mut sessions: Vec<(usize, &SavedSession)> =
            self.settings.sessions.iter().enumerate().collect();
        sessions.sort_by_key(|(_, s)| !s.favourite);
        for (i, s) in sessions {
            items.push(Item {
                key: format!("session:{}", s.name),
                kind: "Session",
                label: s.name.clone(),
                detail: s.address(),
                shortcut: String::new(),
                choice: Choice::Session(i),
            });
        }
        for (i, host) in self.openssh.hosts().iter().enumerate() {
            items.push(Item {
                key: format!("config:{}", host.alias),
                kind: "~/.ssh/config",
                label: host.alias.clone(),
                detail: host.host_name.clone(),
                shortcut: String::new(),
                choice: Choice::ConfigHost(i),
            });
        }
        for (i, s) in self.settings.recent.iter().enumerate() {
            items.push(Item {
                key: format!("recent:{}", s.address()),
                kind: "Recent",
                label: s.address(),
                detail: String::new(),
                shortcut: String::new(),
                choice: Choice::Recent(i),
            });
        }
        for (i, s) in self.settings.snippets.iter().enumerate() {
            items.push(Item {
                key: format!("snippet:{}", s.name),
                kind: "Snippet",
                label: s.name.clone(),
                detail: s.text.lines().next().unwrap_or_default().to_owned(),
                shortcut: s.key.clone(),
                choice: Choice::Action(Action::SendSnippet(i)),
            });
        }
        for (action, label, shortcut) in actions(mac) {
            items.push(Item {
                key: format!("action:{label}"),
                kind: "Action",
                label: label.to_owned(),
                detail: String::new(),
                shortcut,
                choice: Choice::Action(action),
            });
        }
        items
    }

    /// Does what was chosen in the palette, and remembers it.
    pub(crate) fn palette_chosen(&mut self, ctx: &egui::Context, closed: Closed) {
        let Closed::Chosen(key, choice) = closed else {
            if let Some(session) = self.active_session_mut() {
                session.focus_screen();
            }
            return;
        };
        super::remember(&mut self.settings.palette_recent, &key);
        match choice {
            Choice::Action(action) => self.carry_out(ctx, action),
            Choice::Tab(i) => self.select(i),
            Choice::Session(i) => {
                if let Some(source) = self.settings.sessions.get(i).cloned() {
                    self.open_source(ctx, source, true);
                }
            }
            Choice::ConfigHost(i) => {
                if let Some(host) = self.openssh.hosts().get(i) {
                    let source = crate::openssh::session(&self.settings.defaults, host);
                    self.open_source(ctx, source, false);
                }
            }
            Choice::Recent(i) => {
                if let Some((source, saved)) =
                    recent_source(&self.settings.sessions, &self.settings.recent, i)
                {
                    self.open_source(ctx, source, saved);
                }
            }
        }
        if let Some(session) = self.active_session_mut() {
            session.focus_screen();
        }
    }

    /// Connects `source` in the pane in front when it shows a start screen, else in a new tab.
    fn open_source(&mut self, ctx: &egui::Context, source: SavedSession, saved: bool) {
        if !self
            .tabs
            .get(self.current)
            .is_some_and(crate::tabs::Tab::shows_start)
        {
            self.new_tab(ctx, None);
        }
        let pane = self.tabs[self.current].active;
        self.connect_pane(ctx, pane, source, saved);
    }
}

/// Keeps `source` first among the recent connections, at most ten, each address once (R-SSH-48); a saved session
/// keeps its name, to be found again (R-SSH-64), any other is kept without one.
pub fn remember_connection(recent: &mut Vec<SavedSession>, source: &SavedSession, saved: bool) {
    let address = source.address();
    recent.retain(|s| s.address() != address);
    let name = if saved {
        source.name.clone()
    } else {
        String::new()
    };
    recent.insert(
        0,
        SavedSession {
            name,
            ..source.clone()
        },
    );
    recent.truncate(10);
}

/// The index of the saved session the recent connection `recent` was made with, while one is saved under its name;
/// for one kept without a name (made otherwise, or before names were kept), the saved session at its address
/// (R-SSH-64).
pub fn recent_saved(sessions: &[SavedSession], recent: &SavedSession) -> Option<usize> {
    if recent.name.is_empty() {
        let address = recent.address();
        return sessions.iter().position(|s| s.address() == address);
    }
    sessions.iter().position(|s| s.name == recent.name)
}

/// What recent connection `i` connects to, and whether that is a saved session: the saved one as it is now, else the
/// connection as it was made, without a name (R-SSH-64).
pub fn recent_source(
    sessions: &[SavedSession],
    recent: &[SavedSession],
    i: usize,
) -> Option<(SavedSession, bool)> {
    let made = recent.get(i)?;
    Some(match recent_saved(sessions, made) {
        Some(saved) => (sessions[saved].clone(), true),
        None => (
            SavedSession {
                name: String::new(),
                ..made.clone()
            },
            false,
        ),
    })
}
