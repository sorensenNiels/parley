//! The SQL window's history (R-SQL-89..91, D35): every run, with when it ran, how long it took
//! and how it ended, the last [`LIMIT`] per saved connection; never results (R-SQL-7). The
//! outcome is the one the editor's gutter shows too (R-SQL-62).

use std::time::Duration;

use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

/// Runs kept per saved connection (R-SQL-89).
pub const LIMIT: usize = 500;
/// An error is kept as its first line, at most this many characters.
const ERROR_CHARS: usize = 300;

/// How a run ended (R-SQL-62, R-SQL-89).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Rows read, and whether more were left to fetch.
    Rows { count: usize, more: bool },
    /// Rows changed.
    Changed(i64),
    /// A statement or CL command that returns nothing.
    Done,
    /// The error, its first line.
    Failed(String),
}

impl Outcome {
    /// What it says in the history: "128 rows, more to fetch", "1 row changed", "done", or
    /// the error.
    pub fn text(&self) -> String {
        let rows = |n: i64| if n == 1 { "row" } else { "rows" };
        match self {
            Self::Rows { count, more } => {
                let n = i64::try_from(*count).unwrap_or(i64::MAX);
                let more = if *more { ", more to fetch" } else { "" };
                format!("{count} {}{more}", rows(n))
            }
            Self::Changed(count) => format!("{count} {} changed", rows(*count)),
            Self::Done => "done".to_owned(),
            Self::Failed(message) => message.clone(),
        }
    }

    pub const fn failed(&self) -> bool {
        match self {
            Self::Failed(_) => true,
            Self::Rows { .. } | Self::Changed(_) | Self::Done => false,
        }
    }
}

/// One run (R-SQL-89). Entries kept before Phase 19 have only the statement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Stored")]
pub struct Entry {
    pub statement: String,
    /// When it ran: seconds since 1970, UTC.
    pub at: Option<i64>,
    /// How long it took, in milliseconds.
    pub took_ms: Option<u64>,
    pub outcome: Option<Outcome>,
}

/// An entry as the settings may hold it: a statement alone (before Phase 19), or a run.
#[derive(Deserialize)]
#[serde(untagged)]
enum Stored {
    Statement(String),
    Run(Run),
}

#[derive(Deserialize)]
struct Run {
    statement: String,
    #[serde(default)]
    at: Option<i64>,
    #[serde(default)]
    took_ms: Option<u64>,
    #[serde(default)]
    outcome: Option<Outcome>,
}

impl From<Stored> for Entry {
    fn from(stored: Stored) -> Self {
        match stored {
            Stored::Statement(statement) => Self {
                statement,
                at: None,
                took_ms: None,
                outcome: None,
            },
            Stored::Run(run) => Self {
                statement: run.statement,
                at: run.at,
                took_ms: run.took_ms,
                outcome: run.outcome,
            },
        }
    }
}

impl Entry {
    /// `statement`, run now, which took `took` and ended as `outcome`; an error is kept as its
    /// first line.
    pub fn now(statement: &str, took: Duration, outcome: Outcome) -> Self {
        let outcome = match outcome {
            Outcome::Failed(message) => Outcome::Failed(first_line(&message)),
            other => other,
        };
        Self {
            statement: statement.to_owned(),
            at: Some(chrono::Utc::now().timestamp()),
            took_ms: Some(u64::try_from(took.as_millis()).unwrap_or(u64::MAX)),
            outcome: Some(outcome),
        }
    }

    /// Whether it succeeded; `None` for an entry kept without its outcome.
    pub fn succeeded(&self) -> Option<bool> {
        self.outcome.as_ref().map(|o| !o.failed())
    }

    /// The line under the statement: time, how long it took, and the outcome, as far as
    /// known; "10:42 · 0.31 s · 128 rows".
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(time) = self.at.and_then(local) {
            parts.push(time.format("%H:%M").to_string());
        }
        if let Some(ms) = self.took_ms {
            parts.push(seconds(ms));
        }
        if let Some(outcome) = &self.outcome {
            parts.push(outcome.text());
        }
        parts.join(" · ")
    }

    /// The local date it ran, for its day heading.
    pub fn date(&self) -> Option<NaiveDate> {
        self.at.and_then(local).map(|t| t.date_naive())
    }

    /// Whether it is shown for the search `query` (its statement or error, ignoring case;
    /// lower case already) and `show`.
    pub fn matches(&self, query: &str, show: Show) -> bool {
        let shown = match show {
            Show::All => true,
            Show::Succeeded => self.succeeded() == Some(true),
            Show::Failed => self.succeeded() == Some(false),
        };
        shown
            && (query.is_empty()
                || self.statement.to_lowercase().contains(query)
                || self
                    .outcome
                    .as_ref()
                    .is_some_and(|o| o.failed() && o.text().to_lowercase().contains(query)))
    }
}

/// Which entries the list shows (R-SQL-90).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Show {
    #[default]
    All,
    Succeeded,
    Failed,
}

/// Adds `ran` to `history`, keeping the last [`LIMIT`] (R-SQL-89).
pub fn add(history: &mut Vec<Entry>, ran: Vec<Entry>) {
    history.extend(ran);
    let excess = history.len().saturating_sub(LIMIT);
    history.drain(..excess);
}

/// The heading of the day `date`, seen on `today`: Today, Yesterday, else the date; Earlier
/// for entries without a time (R-SQL-90).
pub fn day_heading(date: Option<NaiveDate>, today: NaiveDate) -> String {
    match date {
        None => "Earlier".to_owned(),
        Some(d) if d == today => "Today".to_owned(),
        Some(d) if today.pred_opt() == Some(d) => "Yesterday".to_owned(),
        Some(d) => d.format("%a %-d %b %Y").to_string(),
    }
}

/// Today's local date.
pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

fn local(at: i64) -> Option<DateTime<Local>> {
    DateTime::from_timestamp(at, 0).map(|t| t.with_timezone(&Local))
}

/// Milliseconds as seconds with two decimals, as the gutter says them.
fn seconds(ms: u64) -> String {
    format!("{}.{:02} s", ms / 1000, (ms % 1000) / 10)
}

fn first_line(message: &str) -> String {
    let line = message.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    line.trim().chars().take(ERROR_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(statement: &str, outcome: Option<Outcome>) -> Entry {
        Entry {
            statement: statement.to_owned(),
            at: None,
            took_ms: Some(310),
            outcome,
        }
    }

    /// R-SQL-89: the history kept before, statements only, reads as entries without a time;
    /// a run is kept and read back whole.
    #[test]
    fn old_history_and_runs_are_both_read_r_sql_89() {
        let old: Vec<Entry> = serde_json::from_str(r#"["VALUES 'æøå'"]"#).expect("reads");
        assert_eq!(
            old,
            [Entry::from(Stored::Statement("VALUES 'æøå'".to_owned()))]
        );
        assert_eq!(old[0].at, None);
        assert_eq!(old[0].succeeded(), None);
        let run = Entry::now(
            "SELECT 1",
            Duration::from_millis(1234),
            Outcome::Rows {
                count: 3,
                more: true,
            },
        );
        let json = serde_json::to_string(&run).expect("serializes");
        let back: Entry = serde_json::from_str(&json).expect("reads");
        assert_eq!(back, run);
        assert_eq!(back.took_ms, Some(1234));
    }

    /// R-SQL-89: every run is kept, also of the same statement, and only the last 500.
    #[test]
    fn every_run_is_kept_up_to_the_limit_r_sql_89() {
        let mut history = vec![entry("A", None)];
        add(&mut history, vec![entry("A", None), entry("B", None)]);
        assert_eq!(history.len(), 3, "a run again is an entry of its own");
        add(
            &mut history,
            (0..600).map(|n| entry(&format!("S{n}"), None)).collect(),
        );
        assert_eq!(history.len(), LIMIT);
        assert_eq!(history.last().map(|e| e.statement.as_str()), Some("S599"));
    }

    /// R-SQL-89: an error is kept as its first line, cut short.
    #[test]
    fn an_error_is_kept_as_its_first_line_r_sql_89() {
        let message = format!(
            "\nSQL0204: KUNDER i *LIBL findes ikke.\nCause . . . {}",
            "x".repeat(50)
        );
        let e = Entry::now(
            "SELECT * FROM KUNDER",
            Duration::ZERO,
            Outcome::Failed(message),
        );
        assert_eq!(
            e.outcome,
            Some(Outcome::Failed(
                "SQL0204: KUNDER i *LIBL findes ikke.".to_owned()
            ))
        );
        let long = Entry::now("X", Duration::ZERO, Outcome::Failed("æ".repeat(400)));
        assert!(matches!(long.outcome, Some(Outcome::Failed(m)) if m.chars().count() == 300));
    }

    /// R-SQL-90: what the line under the statement says.
    #[test]
    fn the_summary_says_how_long_and_how_it_ended_r_sql_90() {
        let rows = |count, more| Some(Outcome::Rows { count, more });
        assert_eq!(
            entry("S", rows(128, true)).summary(),
            "0.31 s · 128 rows, more to fetch"
        );
        assert_eq!(entry("S", rows(1, false)).summary(), "0.31 s · 1 row");
        assert_eq!(
            entry("S", Some(Outcome::Changed(1))).summary(),
            "0.31 s · 1 row changed"
        );
        assert_eq!(entry("S", Some(Outcome::Done)).summary(), "0.31 s · done");
        let old = Entry {
            took_ms: None,
            ..entry("S", None)
        };
        assert_eq!(old.summary(), "");
        let timed = Entry {
            at: Some(0),
            ..entry("S", None)
        };
        assert!(timed.summary().contains(':'), "{}", timed.summary());
    }

    /// R-SQL-90: the search finds the statement or the error, ignoring case; the filter
    /// keeps the succeeded or the failed, and old entries only under All.
    #[test]
    fn search_and_filter_r_sql_90() {
        let ok = entry("SELECT * FROM Kunder", Some(Outcome::Done));
        let bad = entry("SELECT 1", Some(Outcome::Failed("SQL0204 ÆBLE".to_owned())));
        let old = entry("VALUES 2", None);
        assert!(ok.matches("kunder", Show::All));
        assert!(bad.matches("æble", Show::All), "the error is searched");
        assert!(!ok.matches("æble", Show::All));
        assert!(ok.matches("", Show::Succeeded) && !ok.matches("", Show::Failed));
        assert!(bad.matches("", Show::Failed) && !bad.matches("", Show::Succeeded));
        assert!(old.matches("", Show::All));
        assert!(!old.matches("", Show::Succeeded) && !old.matches("", Show::Failed));
    }

    /// R-SQL-90: day headings.
    #[test]
    fn days_are_today_yesterday_or_the_date_r_sql_90() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 15).expect("a date");
        let day = |d: u32| NaiveDate::from_ymd_opt(2026, 11, d);
        assert_eq!(day_heading(day(15), today), "Today");
        assert_eq!(day_heading(day(14), today), "Yesterday");
        assert_eq!(day_heading(day(12), today), "Thu 12 Nov 2026");
        assert_eq!(day_heading(None, today), "Earlier");
        assert_eq!(seconds(5), "0.00 s");
        assert_eq!(seconds(61_234), "61.23 s");
    }
}
