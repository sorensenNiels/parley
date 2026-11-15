//! The IBM i Services the side panel's System node reads (R-SQL-78..80, D32): the signed-on
//! user's spooled files, message queue and active jobs, a spooled file's text and a job's job
//! log. Read-only, on the second connection (D21). The window and `sqlprobe --tools` run the
//! same text.

use super::catalog::literal;
use super::protocol::Row;

/// At most this many entries are listed in a view.
pub const MAX_ENTRIES: u32 = 1000;
/// At most this many lines of a spooled file or job log are read; under the second
/// connection's row limit, so no cursor stays open.
pub const MAX_LINES: usize = 9999;

/// A list under the System node (R-SQL-78..80).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum View {
    SpooledFiles,
    MessageQueue,
    Jobs,
}

/// A view's column: its heading, and whether it is a number (right-aligned).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewColumn {
    pub heading: &'static str,
    pub numeric: bool,
}

const fn text(heading: &'static str) -> ViewColumn {
    ViewColumn {
        heading,
        numeric: false,
    }
}

const fn number(heading: &'static str) -> ViewColumn {
    ViewColumn {
        heading,
        numeric: true,
    }
}

/// Spooled files: the columns of [`View::SpooledFiles`]'s SQL, in order.
pub mod spooled {
    pub const NAME: usize = 0;
    pub const JOB: usize = 2;
    pub const NUMBER: usize = 3;
}

/// The message queue: the columns of [`View::MessageQueue`]'s SQL, in order.
pub mod messages {
    pub const TIME: usize = 0;
    pub const HELP: usize = 5;
}

/// Jobs: the columns of [`View::Jobs`]'s SQL, in order.
pub mod jobs {
    pub const NAME: usize = 0;
}

impl View {
    pub const ALL: [Self; 3] = [Self::SpooledFiles, Self::MessageQueue, Self::Jobs];

    /// Its name in the tree and on its tab.
    pub const fn title(self) -> &'static str {
        match self {
            Self::SpooledFiles => "Spooled files",
            Self::MessageQueue => "Message queue",
            Self::Jobs => "Jobs",
        }
    }

    /// Whether an entry opens in a viewer: a spooled file's text, a job's job log.
    pub const fn opens(self) -> bool {
        match self {
            Self::SpooledFiles | Self::Jobs => true,
            Self::MessageQueue => false,
        }
    }

    /// The SQL that lists it, newest first, for the signed-on user (`SESSION_USER`).
    pub fn sql(self) -> String {
        match self {
            // R-SQL-78
            Self::SpooledFiles => format!(
                "SELECT SPOOLED_FILE_NAME, USER_DATA, JOB_NAME, FILE_NUMBER, TOTAL_PAGES, STATUS, \
                 CREATE_TIMESTAMP FROM QSYS2.OUTPUT_QUEUE_ENTRIES WHERE USER_NAME = SESSION_USER \
                 ORDER BY CREATE_TIMESTAMP DESC FETCH FIRST {MAX_ENTRIES} ROWS ONLY"
            ),
            // R-SQL-79: the user's own message queue is QUSRSYS/user.
            Self::MessageQueue => format!(
                "SELECT MESSAGE_TIMESTAMP, MESSAGE_ID, MESSAGE_TYPE, MESSAGE_TEXT, FROM_USER, \
                 MESSAGE_SECOND_LEVEL_TEXT FROM QSYS2.MESSAGE_QUEUE_INFO \
                 WHERE MESSAGE_QUEUE_LIBRARY = 'QUSRSYS' AND MESSAGE_QUEUE_NAME = SESSION_USER \
                 ORDER BY MESSAGE_TIMESTAMP DESC FETCH FIRST {MAX_ENTRIES} ROWS ONLY"
            ),
            // R-SQL-80: jobs that run as the user, the database server jobs among them.
            Self::Jobs => format!(
                "SELECT JOB_NAME, SUBSYSTEM, JOB_TYPE, JOB_STATUS, CPU_TIME, FUNCTION \
                 FROM TABLE(QSYS2.ACTIVE_JOB_INFO()) X WHERE AUTHORIZATION_NAME = SESSION_USER \
                 ORDER BY JOB_NAME FETCH FIRST {MAX_ENTRIES} ROWS ONLY"
            ),
        }
    }

    /// Its columns, as its SQL selects them.
    pub const fn columns(self) -> &'static [ViewColumn] {
        match self {
            Self::SpooledFiles => &SPOOLED_COLUMNS,
            Self::MessageQueue => &MESSAGE_COLUMNS,
            Self::Jobs => &JOB_COLUMNS,
        }
    }
}

const SPOOLED_COLUMNS: [ViewColumn; 7] = [
    text("Name"),
    text("User data"),
    text("Job"),
    number("Number"),
    number("Pages"),
    text("Status"),
    text("Created"),
];
const MESSAGE_COLUMNS: [ViewColumn; 6] = [
    text("Time"),
    text("Message ID"),
    text("Type"),
    text("Message"),
    text("From"),
    text("Help"),
];
const JOB_COLUMNS: [ViewColumn; 6] = [
    text("Job"),
    text("Subsystem"),
    text("Type"),
    text("Status"),
    number("CPU (ms)"),
    text("Function"),
];

/// The text of the spooled file `name`, number `number`, of `job` (R-SQL-78), a line a row.
/// `Err` when the number is not one: it comes from the host.
pub fn spooled_file_text(job: &str, name: &str, number: &str) -> Result<String, String> {
    let number: u32 = number
        .trim()
        .parse()
        .map_err(|_| format!("\"{}\" is not a spooled file number.", number.trim()))?;
    Ok(format!(
        "SELECT SPOOLED_DATA FROM TABLE(SYSTOOLS.SPOOLED_FILE_DATA(JOB_NAME => {}, \
         SPOOLED_FILE_NAME => {}, SPOOLED_FILE_NUMBER => {number})) X \
         ORDER BY ORDINAL_POSITION FETCH FIRST {MAX_LINES} ROWS ONLY",
        literal(job.trim()),
        literal(name.trim()),
    ))
}

/// The job log of `job` (R-SQL-80): time, message ID and text, oldest first.
pub fn job_log(job: &str) -> String {
    format!(
        "SELECT MESSAGE_TIMESTAMP, MESSAGE_ID, MESSAGE_TEXT FROM TABLE(QSYS2.JOBLOG_INFO({})) X \
         ORDER BY ORDINAL_POSITION FETCH FIRST {MAX_LINES} ROWS ONLY",
        literal(job.trim())
    )
}

fn cell(row: &Row, n: usize) -> &str {
    row.get(n).and_then(Option::as_deref).unwrap_or_default()
}

/// A spooled file's rows as its text, a line a row, blanks at line ends trimmed.
pub fn spooled_lines(rows: &[Row]) -> String {
    rows.iter()
        .map(|r| cell(r, 0).trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// A job log's rows as its text: a message a line, with its time and ID.
pub fn job_log_lines(rows: &[Row]) -> String {
    rows.iter()
        .map(|r| {
            let id = cell(r, 1);
            let id = if id.is_empty() { "       " } else { id };
            format!("{}  {id}  {}", cell(r, 0), cell(r, 2).trim_end())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// How many messages are newer than `seen`, the time of the newest seen (R-SQL-79); all of
/// them when none was seen. Times are the host's timestamps, which sort as text.
pub fn newer_than(rows: &[Row], seen: Option<&str>) -> usize {
    rows.iter()
        .filter(|r| seen.is_none_or(|seen| cell(r, messages::TIME) > seen))
        .count()
}

/// The time of the newest message in `rows`.
pub fn newest(rows: &[Row]) -> Option<String> {
    rows.iter()
        .map(|r| cell(r, messages::TIME))
        .filter(|t| !t.is_empty())
        .max()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cells: &[Option<&str>]) -> Row {
        cells.iter().map(|c| c.map(str::to_owned)).collect()
    }

    /// R-SQL-78..80: each view lists the signed-on user's own, newest first, within limits,
    /// with a heading per column it selects.
    #[test]
    fn each_view_reads_the_users_own_within_a_limit_r_sql_78() {
        for view in View::ALL {
            let sql = view.sql();
            assert!(sql.contains("SESSION_USER"), "{sql}");
            assert!(sql.contains("FETCH FIRST 1000 ROWS ONLY"), "{sql}");
            let selected = sql
                .split(" FROM ")
                .next()
                .map_or(0, |s| s.matches(',').count() + 1);
            assert_eq!(selected, view.columns().len(), "{view:?}");
        }
        assert!(View::SpooledFiles.opens() && View::Jobs.opens());
        assert!(!View::MessageQueue.opens());
    }

    /// R-SQL-78: names from the host are quoted, and a number that is not one is an error.
    #[test]
    fn a_spooled_files_text_is_asked_by_quoted_names_r_sql_78() {
        let sql = spooled_file_text("123/O'BRIEN/QPADEV0001", "QSYSPRT", " 4 ").expect("sql");
        assert!(
            sql.contains("JOB_NAME => '123/O''BRIEN/QPADEV0001'"),
            "{sql}"
        );
        assert!(sql.contains("SPOOLED_FILE_NUMBER => 4)"), "{sql}");
        assert!(spooled_file_text("J", "N", "4; DROP").is_err());
        assert!(spooled_file_text("J", "N", "").is_err());
        assert!(job_log("1/A'B/C").contains("JOBLOG_INFO('1/A''B/C')"));
    }

    /// R-SQL-78, R-SQL-80: the rows become text, a line a row; empty IDs keep the columns.
    #[test]
    fn rows_become_the_viewers_text_r_sql_78_r_sql_80() {
        let lines = [row(&[Some("Side 1  ")]), row(&[None]), row(&[Some("æøå")])];
        assert_eq!(spooled_lines(&lines), "Side 1\n\næøå");
        assert_eq!(spooled_lines(&[]), "");
        let log = [
            row(&[
                Some("2026-11-15 10:00:00"),
                Some("CPF1124"),
                Some("Job started."),
            ]),
            row(&[Some("2026-11-15 10:00:01"), None, Some("Løst")]),
        ];
        assert_eq!(
            job_log_lines(&log),
            "2026-11-15 10:00:00  CPF1124  Job started.\n2026-11-15 10:00:01           Løst"
        );
    }

    /// R-SQL-79: new messages are those after the newest seen; none seen, all are new.
    #[test]
    fn messages_newer_than_the_newest_seen_are_counted_r_sql_79() {
        let rows = [
            row(&[Some("2026-11-15 10:00:02.000000")]),
            row(&[Some("2026-11-15 10:00:01.000000")]),
            row(&[None]),
        ];
        assert_eq!(newer_than(&rows, None), 3);
        assert_eq!(newer_than(&rows, Some("2026-11-15 10:00:01.000000")), 1);
        assert_eq!(newest(&rows).as_deref(), Some("2026-11-15 10:00:02.000000"));
        assert_eq!(newer_than(&rows, newest(&rows).as_deref()), 0);
        assert_eq!(newest(&[]), None);
    }
}
