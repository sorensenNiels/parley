//! The history view (R-UI-38, D61): a kept screen in the live one's place, read-only, under a bar that says which
//! it is and when it came.

use std::time::SystemTime;

use egui::{Ui, WidgetInfo, accesskit::Role};
use parley_core::Session;
use parley_core::screen::Screen;
use parley_core::session::{History, KeptScreen};

use crate::{icons, theme};

/// Which kept screen is shown: counted as the history counts them ([`History::added`]), so it stays the same
/// screen while the host's new ones are kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Browse {
    serial: u64,
    /// How many screens had been kept when the view opened.
    opened_at: u64,
}

impl Browse {
    /// The serial number of the oldest screen still kept.
    fn first(history: &History) -> u64 {
        (history.added() + 1).saturating_sub(history.len() as u64)
    }

    /// Opens on the newest kept screen; when it is what the live screen shows, on the one before it.
    pub(super) fn open(history: &History, live: &Screen) -> Option<Self> {
        let newest = history.screens().back()?;
        let skip = history.len() > 1 && newest.screen.buffer() == live.buffer();
        Some(Self {
            serial: history.added() - u64::from(skip),
            opened_at: history.added(),
        })
    }

    /// Its place in the history, from 0; the oldest when it has been dropped since.
    fn index(&self, history: &History) -> Option<usize> {
        let last = history.len().checked_sub(1)?;
        let from_first = self.serial.saturating_sub(Self::first(history));
        Some(usize::try_from(from_first).map_or(last, |i| i.min(last)))
    }

    pub(super) fn shown<'a>(&self, history: &'a History) -> Option<&'a KeptScreen> {
        history.screens().get(self.index(history)?)
    }

    /// One screen further back, staying at the oldest.
    pub(super) fn back(&mut self, history: &History) {
        self.serial = self.serial.saturating_sub(1).max(Self::first(history));
    }

    /// One screen forward; `false` past the newest, where the live screen comes back.
    pub(super) fn forward(&mut self, history: &History) -> bool {
        self.serial = self.serial.max(Self::first(history));
        if self.serial >= history.added() {
            return false;
        }
        self.serial += 1;
        true
    }

    /// What the bar says about it.
    pub(super) fn place(&self, history: &History) -> Option<Place> {
        let index = self.index(history)?;
        Some(Place {
            number: index + 1,
            of: history.len(),
            came: history.screens().get(index)?.came,
            new: history.added().saturating_sub(self.opened_at),
        })
    }
}

/// Where the shown screen is in the history: its number, how many there are, when it came, and how many came
/// since the view opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    pub number: usize,
    pub of: usize,
    pub came: SystemTime,
    pub new: u64,
}

impl Place {
    /// When the screen came, as the local time of day: "10:42:13".
    pub fn time(&self) -> String {
        chrono::DateTime::<chrono::Local>::from(self.came)
            .format("%H:%M:%S")
            .to_string()
    }

    /// "Screen 12 of 50 · 10:42:13", and how many came since.
    pub fn text(&self) -> String {
        let time = self.time();
        let since = match self.new {
            0 => String::new(),
            1 => " · 1 new screen".to_owned(),
            n => format!(" · {n} new screens"),
        };
        format!("Screen {} of {} · {time}{since}", self.number, self.of)
    }
}

/// What the bar asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryAction {
    Back,
    Forward,
    Live,
    /// Save the screen shown (R-UI-39).
    Save,
}

/// The bar over the history view.
pub fn bar(ui: &mut Ui, place: &Place) -> Option<HistoryAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        let back = icons::button(icons::CARET_LEFT, "").small();
        if icons::add_named(ui, place.number > 1, back, "Earlier screen").clicked() {
            action = Some(HistoryAction::Back);
        }
        let forward = icons::button(icons::CARET_RIGHT, "").small();
        if icons::add_named(ui, true, forward, "Later screen").clicked() {
            action = Some(HistoryAction::Forward);
        }
        let text = place.text();
        let label = ui.label(&text);
        label.widget_info(|| WidgetInfo::labeled(Role::Label, true, &text));
        theme::button_row(ui, |ui| {
            if ui.add(theme::ghost_button("Back to live")).clicked() {
                action = Some(HistoryAction::Live);
            }
            if ui.add(theme::ghost_button("Save…")).clicked() {
                action = Some(HistoryAction::Save);
            }
        });
    });
    action
}

/// What the status bar says about the history (R-UI-38).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryStatus {
    /// A kept screen is shown.
    Shown,
    /// Ctrl+Page Up found no screen kept yet.
    NoneKept,
}

/// How long the status bar says no screen is kept, in seconds.
pub(super) const NONE_KEPT_FOR: f64 = 4.0;

/// The screen the history shows, or the live one.
pub(super) fn shown<'a>(browse: Option<&Browse>, session: &'a Session) -> &'a Screen {
    browse
        .and_then(|b| b.shown(session.history()))
        .map_or_else(|| session.screen(), |k| &k.screen)
}

#[cfg(test)]
mod tests {
    use super::*;
    use parley_core::CodePage;
    use parley_core::screen::ScreenSize;

    fn screen(byte: u8) -> Screen {
        let mut s = Screen::new(ScreenSize::Standard);
        s.put(0, byte);
        s
    }

    fn history_of(count: u8) -> History {
        let mut h = History::default();
        h.set_limit(10);
        for n in 0..count {
            h.keep(&screen(0xC1 + n), CodePage::Ccsid37, SystemTime::UNIX_EPOCH);
        }
        h
    }

    #[test]
    fn the_view_opens_before_the_live_screen_and_stays_put_as_new_screens_come_r_ui_38() {
        let mut h = history_of(3);
        let live = h.screens()[2].screen.clone();
        let mut b = Browse::open(&h, &live).expect("screens kept");
        assert_eq!(
            b.place(&h).map(|p| (p.number, p.of, p.new)),
            Some((2, 3, 0))
        );

        h.keep(&screen(0xF1), CodePage::Ccsid37, SystemTime::UNIX_EPOCH);
        assert_eq!(
            b.place(&h).map(|p| (p.number, p.of, p.new)),
            Some((2, 4, 1))
        );
        assert_eq!(b.shown(&h).map(|k| k.screen.get(0)), Some(0xC2));

        b.back(&h);
        b.back(&h);
        assert_eq!(
            b.place(&h).map(|p| p.number),
            Some(1),
            "the oldest is as far as it goes"
        );
        assert!(b.forward(&h) && b.forward(&h) && b.forward(&h));
        assert!(!b.forward(&h), "past the newest the live screen comes back");
    }

    #[test]
    fn a_shown_screen_dropped_from_the_history_becomes_the_oldest_r_ui_38() {
        let mut h = history_of(10);
        let mut b = Browse::open(&h, &screen(0x40)).expect("screens kept");
        b.back(&h);
        for _ in 0..9 {
            b.back(&h);
        }
        h.keep(&screen(0xF1), CodePage::Ccsid37, SystemTime::UNIX_EPOCH);
        assert_eq!(b.place(&h).map(|p| p.number), Some(1));
        assert_eq!(b.shown(&h).map(|k| k.screen.get(0)), Some(0xC2));
        assert!(Browse::open(&History::default(), &screen(0x40)).is_none());
    }

    #[test]
    fn the_bar_goes_back_forward_and_to_live_r_ui_38() {
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;
        let place = Place {
            number: 1,
            of: 2,
            came: SystemTime::now(),
            new: 0,
        };
        let mut harness = Harness::builder()
            .with_size(egui::vec2(700.0, 40.0))
            .build_ui_state(
                move |ui, asked: &mut Vec<HistoryAction>| {
                    crate::test_support::install_fonts(ui);
                    asked.extend(bar(ui, &place));
                },
                Vec::new(),
            );
        harness.run();
        harness.get_by_label_contains("Screen 1 of 2 · ");
        harness.get_by_label("Earlier screen").click();
        harness.run();
        harness.get_by_label("Later screen").click();
        harness.run();
        harness.get_by_label("Back to live").click();
        harness.run();
        harness.get_by_label("Save…").click();
        harness.run();
        assert_eq!(
            harness.state(),
            &[
                HistoryAction::Forward,
                HistoryAction::Live,
                HistoryAction::Save
            ],
            "the oldest screen has none earlier"
        );
    }

    #[test]
    fn the_bar_says_which_screen_when_it_came_and_how_many_came_since_r_ui_38() {
        let place = Place {
            number: 12,
            of: 50,
            came: SystemTime::now(),
            new: 3,
        };
        let text = place.text();
        assert!(text.starts_with("Screen 12 of 50 · "), "{text}");
        assert!(text.ends_with(" · 3 new screens"), "{text}");
        assert_eq!(text.matches(':').count(), 2, "{text}");
    }
}
