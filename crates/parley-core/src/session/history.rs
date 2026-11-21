//! Screens kept (R-UI-37, D61): each time the host has written and unlocks the keyboard, the session keeps a copy
//! of the screen for the history view. Non-display text is blanked in the copy, so a password never reaches it.

use std::collections::VecDeque;
use std::time::SystemTime;

use super::Session;
use crate::CodePage;
use crate::screen::{Screen, is_attribute};

/// How many screens are kept unless the settings say otherwise (R-UI-39).
pub const SCREENS_KEPT: usize = 50;

/// The fewest and most screens the settings may keep (R-UI-39).
pub const SCREENS_KEPT_RANGE: std::ops::RangeInclusive<usize> = 10..=500;

/// A screen as it was when the keyboard unlocked: its text, attributes, extended attributes, GUI windows and
/// cursor, with non-display text blanked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptScreen {
    pub screen: Screen,
    /// When it came.
    pub came: SystemTime,
}

/// The kept screens, oldest first, at most `limit` of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    kept: VecDeque<KeptScreen>,
    limit: usize,
    /// How many screens have been kept in all, dropped ones too: the view counts new ones by it.
    added: u64,
}

impl Default for History {
    fn default() -> Self {
        Self {
            kept: VecDeque::new(),
            limit: SCREENS_KEPT,
            added: 0,
        }
    }
}

impl History {
    /// The kept screens, oldest first.
    pub fn screens(&self) -> &VecDeque<KeptScreen> {
        &self.kept
    }

    pub fn len(&self) -> usize {
        self.kept.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kept.is_empty()
    }

    /// How many screens have been kept since the history began, those dropped since too.
    pub const fn added(&self) -> u64 {
        self.added
    }

    pub const fn limit(&self) -> usize {
        self.limit
    }

    /// Keeps at most `limit` screens (within [`SCREENS_KEPT_RANGE`]); the oldest go at once.
    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit.clamp(*SCREENS_KEPT_RANGE.start(), *SCREENS_KEPT_RANGE.end());
        self.trim();
    }

    /// Keeps `screen`, which came at `came`, with its non-display text blanked; the same screen as the last one
    /// kept is not kept again.
    pub fn keep(&mut self, screen: &Screen, code_page: CodePage, came: SystemTime) {
        let screen = blanked(screen, code_page);
        if self.kept.back().is_some_and(|last| last.screen == screen) {
            return;
        }
        self.kept.push_back(KeptScreen { screen, came });
        self.added = self.added.wrapping_add(1);
        self.trim();
    }

    fn trim(&mut self) {
        while self.kept.len() > self.limit {
            self.kept.pop_front();
        }
    }
}

/// `screen` with every non-display position blank: what the screen shows hidden (a non-display field, or text
/// after a non-display attribute or extended attribute) is gone from the copy, not just from the drawing.
fn blanked(screen: &Screen, code_page: CodePage) -> Screen {
    let mut copy = screen.clone();
    for (address, cell) in screen.cells(code_page).iter().enumerate() {
        let byte = screen.get(address);
        if cell.style.hidden && !is_attribute(byte) && byte != BLANK {
            copy.put(address, BLANK);
        }
    }
    copy
}

/// A blank in every EBCDIC code page.
const BLANK: u8 = 0x40;

impl Session {
    /// The screens kept so far (R-UI-37).
    pub const fn history(&self) -> &History {
        &self.history
    }

    /// How many screens to keep (R-UI-39).
    pub fn set_screens_kept(&mut self, limit: usize) {
        self.history.set_limit(limit);
    }

    /// Takes the history, for the session that replaces this one on a reconnect.
    pub fn take_history(&mut self) -> History {
        let empty = History {
            limit: self.history.limit,
            ..History::default()
        };
        std::mem::replace(&mut self.history, empty)
    }

    /// Goes on with `history`, taken from the session this one replaces.
    pub fn adopt_history(&mut self, history: History) {
        self.history = history;
    }

    /// Keeps the screen if a host write unlocked the keyboard since the last time (R-UI-37).
    pub(super) fn keep_unlocked_screen(&mut self) {
        if std::mem::take(&mut self.unlocked_since_kept) && !self.locked {
            self.history
                .keep(&self.screen, self.config.code_page, SystemTime::now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::ScreenSize;

    fn screen_saying(text: &str) -> Screen {
        let mut s = Screen::new(ScreenSize::Standard);
        for (i, c) in text.chars().enumerate() {
            s.put(i, CodePage::Ccsid37.to_byte(c).expect("CCSID 37 character"));
        }
        s
    }

    #[test]
    fn the_same_screen_twice_is_kept_once_and_the_oldest_go_past_the_limit_r_ui_37() {
        let mut h = History::default();
        let now = SystemTime::now();
        h.keep(&screen_saying("æøå"), CodePage::Ccsid37, now);
        h.keep(&screen_saying("æøå"), CodePage::Ccsid37, now);
        assert_eq!((h.len(), h.added()), (1, 1));

        h.set_limit(1);
        assert_eq!(h.limit(), 10, "at least ten are kept");
        for n in 0..12 {
            h.keep(
                &screen_saying(&format!("screen {n}")),
                CodePage::Ccsid37,
                now,
            );
        }
        assert_eq!((h.len(), h.added()), (10, 13));
        let first = &h.screens()[0].screen;
        assert!(first.text(CodePage::Ccsid37).starts_with("screen 2"));

        h.set_limit(10_000);
        assert_eq!(h.limit(), 500);
    }

    #[test]
    fn text_after_a_non_display_attribute_is_blanked_in_the_copy_r_ui_37() {
        let mut s = screen_saying("xsecret");
        s.put(0, 0x27);
        s.put(7, 0x20);
        let mut h = History::default();
        h.keep(&s, CodePage::Ccsid37, SystemTime::now());
        let kept = &h.screens()[0].screen;
        assert_eq!(
            &kept.buffer()[..8],
            &[0x27, 0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x20]
        );
        assert_eq!(
            kept.text(CodePage::Ccsid37),
            s.text(CodePage::Ccsid37),
            "it looks the same"
        );
    }
}
