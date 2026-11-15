//! Snippets (R-SQL-24..27): small SQL statements kept to reuse, each a `.sql` file whose first
//! comment lines say what it is, then the SQL:
//!
//! ```sql
//! -- title: Who has an object locked
//! -- tags: did you know, work management
//! -- about: The jobs holding locks on an object (QSYS2.OBJECT_LOCK_INFO).
//! -- needs: IBM i 7.2
//! SELECT … WHERE OBJECT_NAME = UPPER('${object}')
//! ```
//!
//! `${name}` and `${name:default}` are placeholders, asked for when the snippet is used.

use std::fmt::Write as _;

/// A snippet read from its text.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snippet {
    pub title: String,
    pub tags: Vec<String>,
    pub about: String,
    /// The IBM i release it needs, as written ("IBM i 7.3").
    pub needs: String,
    /// The SQL, without the header lines.
    pub sql: String,
}

impl Snippet {
    /// Reads `text`; a header line it does not know stays part of the SQL, and a snippet without
    /// a title is titled by `fallback` (its file name).
    pub fn parse(text: &str, fallback: &str) -> Self {
        let mut snippet = Self::default();
        let mut lines = text.lines().peekable();
        while let Some(line) = lines.peek() {
            let Some((key, value)) = line
                .trim()
                .strip_prefix("--")
                .and_then(|rest| rest.split_once(':'))
                .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
            else {
                break;
            };
            match key.as_str() {
                "title" => snippet.title = value,
                "about" => snippet.about = value,
                "needs" => snippet.needs = value,
                "tags" => {
                    snippet.tags = value
                        .split(',')
                        .map(str::trim)
                        .filter(|t| !t.is_empty())
                        .map(str::to_owned)
                        .collect();
                }
                _ => break,
            }
            lines.next();
        }
        lines
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .clone_into(&mut snippet.sql);
        if snippet.title.is_empty() {
            fallback.clone_into(&mut snippet.title);
        }
        snippet
    }

    /// The snippet as its file holds it.
    pub fn to_text(&self) -> String {
        let mut text = format!("-- title: {}\n", one_line(&self.title));
        if !self.tags.is_empty() {
            // Writing to a String cannot fail.
            let _ = writeln!(text, "-- tags: {}", self.tags.join(", "));
        }
        if !self.about.is_empty() {
            // Writing to a String cannot fail.
            let _ = writeln!(text, "-- about: {}", one_line(&self.about));
        }
        if !self.needs.is_empty() {
            // Writing to a String cannot fail.
            let _ = writeln!(text, "-- needs: {}", one_line(&self.needs));
        }
        text.push_str(self.sql.trim());
        text.push('\n');
        text
    }

    /// Whether `query` (lower case) is in its title, tags, about or SQL, ignoring case.
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || [&self.title, &self.about, &self.sql]
                .iter()
                .any(|s| s.to_lowercase().contains(&query))
            || self.tags.iter().any(|t| t.to_lowercase().contains(&query))
    }
}

/// A header value on one line: a line break would end the header early.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A placeholder in a snippet's SQL: its name and the default written with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placeholder {
    pub name: String,
    pub default: Option<String>,
}

/// The placeholders of `sql`, each name once, in the order they first appear.
pub fn placeholders(sql: &str) -> Vec<Placeholder> {
    let mut found: Vec<Placeholder> = Vec::new();
    for (_, inside) in placeholder_spans(sql) {
        let (name, default) = match inside.split_once(':') {
            Some((name, default)) => (name.trim(), Some(default.to_owned())),
            None => (inside.trim(), None),
        };
        if !name.is_empty() && !found.iter().any(|p| p.name == name) {
            found.push(Placeholder {
                name: name.to_owned(),
                default,
            });
        }
    }
    found
}

/// `sql` with each placeholder replaced by `value(name)`, or its default when that is `None`
/// (an empty text when it has none). Values go in as given: the snippet writes the quotes.
pub fn fill(sql: &str, value: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut at = 0;
    for (range, inside) in placeholder_spans(sql) {
        out.push_str(sql.get(at..range.start).unwrap_or_default());
        let (name, default) = match inside.split_once(':') {
            Some((name, default)) => (name.trim(), default),
            None => (inside.trim(), ""),
        };
        out.push_str(&value(name).unwrap_or_else(|| default.to_owned()));
        at = range.end;
    }
    out.push_str(sql.get(at..).unwrap_or_default());
    out
}

/// Each `${…}` in `sql`: its byte range and what is inside the braces.
fn placeholder_spans(sql: &str) -> Vec<(std::ops::Range<usize>, &str)> {
    let mut spans = Vec::new();
    let mut from = 0;
    while let Some(start) = sql
        .get(from..)
        .and_then(|rest| rest.find("${"))
        .map(|n| from + n)
    {
        let Some(end) = sql
            .get(start + 2..)
            .and_then(|rest| rest.find('}'))
            .map(|n| start + 2 + n)
        else {
            break;
        };
        spans.push((start..end + 1, sql.get(start + 2..end).unwrap_or_default()));
        from = end + 1;
    }
    spans
}

/// The snippets Parley comes with (R-SQL-27, R-SQL-81): what IBM i Services can answer, tagged
/// "did you know"; read-only. Each is (file name, text).
pub const STARTERS: [(&str, &str); 22] = [
    (
        "busiest-jobs",
        include_str!("../../assets/snippets/busiest-jobs.sql"),
    ),
    (
        "sql-job-log",
        include_str!("../../assets/snippets/sql-job-log.sql"),
    ),
    (
        "spooled-files",
        include_str!("../../assets/snippets/spooled-files.sql"),
    ),
    (
        "object-locks",
        include_str!("../../assets/snippets/object-locks.sql"),
    ),
    (
        "group-ptfs",
        include_str!("../../assets/snippets/group-ptfs.sql"),
    ),
    (
        "users-last-sign-on",
        include_str!("../../assets/snippets/users-last-sign-on.sql"),
    ),
    (
        "system-status",
        include_str!("../../assets/snippets/system-status.sql"),
    ),
    (
        "network-connections",
        include_str!("../../assets/snippets/network-connections.sql"),
    ),
    (
        "history-log",
        include_str!("../../assets/snippets/history-log.sql"),
    ),
    (
        "largest-objects",
        include_str!("../../assets/snippets/largest-objects.sql"),
    ),
    (
        "table-statistics",
        include_str!("../../assets/snippets/table-statistics.sql"),
    ),
    (
        "system-values",
        include_str!("../../assets/snippets/system-values.sql"),
    ),
    (
        "library-list",
        include_str!("../../assets/snippets/library-list.sql"),
    ),
    // R-SQL-81
    (
        "disk-use",
        include_str!("../../assets/snippets/disk-use.sql"),
    ),
    (
        "job-queues",
        include_str!("../../assets/snippets/job-queues.sql"),
    ),
    (
        "output-queues",
        include_str!("../../assets/snippets/output-queues.sql"),
    ),
    (
        "largest-ifs-files",
        include_str!("../../assets/snippets/largest-ifs-files.sql"),
    ),
    (
        "object-authorities",
        include_str!("../../assets/snippets/object-authorities.sql"),
    ),
    (
        "passwords-expiring",
        include_str!("../../assets/snippets/passwords-expiring.sql"),
    ),
    (
        "journals",
        include_str!("../../assets/snippets/journals.sql"),
    ),
    (
        "data-areas",
        include_str!("../../assets/snippets/data-areas.sql"),
    ),
    (
        "subsystems",
        include_str!("../../assets/snippets/subsystems.sql"),
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snippet_reads_its_header_and_keeps_the_sql_r_sql_24() {
        let s = Snippet::parse(
            "-- title: Låste objekter\n-- tags: did you know, æøå ,, work\n-- about: Who locks.\n-- needs: IBM i 7.2\nSELECT 1\n  FROM T\n",
            "file",
        );
        assert_eq!(s.title, "Låste objekter");
        assert_eq!(s.tags, ["did you know", "æøå", "work"]);
        assert_eq!(s.about, "Who locks.");
        assert_eq!(s.needs, "IBM i 7.2");
        assert_eq!(s.sql, "SELECT 1\n  FROM T");
    }

    #[test]
    fn a_plain_sql_file_is_a_snippet_titled_by_its_name_r_sql_24() {
        let s = Snippet::parse("-- just a comment\nVALUES 1", "my-query");
        assert_eq!(s.title, "my-query");
        assert_eq!(
            s.sql, "-- just a comment\nVALUES 1",
            "an unknown comment stays SQL"
        );
        assert_eq!(Snippet::parse("", "empty").sql, "");
    }

    #[test]
    fn a_snippet_written_and_read_back_is_the_same_r_sql_24() {
        let s = Snippet {
            title: "Spool\nfiles".to_owned(),
            tags: vec!["did you know".to_owned()],
            about: "A user's spooled files".to_owned(),
            needs: String::new(),
            sql: "SELECT * FROM QSYS2.OUTPUT_QUEUE_ENTRIES".to_owned(),
        };
        let back = Snippet::parse(&s.to_text(), "x");
        assert_eq!(
            back.title, "Spool files",
            "a line break would end the header"
        );
        assert_eq!(back.sql, s.sql);
        assert_eq!(back.tags, s.tags);
    }

    #[test]
    fn search_looks_in_title_tags_about_and_sql_ignoring_case_r_sql_25() {
        let s = Snippet::parse(
            "-- title: Locks\n-- tags: Did You Know\nSELECT * FROM QSYS2.OBJECT_LOCK_INFO",
            "x",
        );
        for query in ["", "LOCKS", "did you", "object_lock"] {
            assert!(s.matches(query), "{query}");
        }
        assert!(!s.matches("æøå"));
    }

    #[test]
    fn placeholders_are_found_once_with_their_defaults_r_sql_26() {
        let sql = "SELECT * FROM T WHERE A = '${lib:QGPL}' AND B = ${n} AND C = '${lib}' AND D = '${empty:}'";
        assert_eq!(
            placeholders(sql),
            [
                Placeholder {
                    name: "lib".to_owned(),
                    default: Some("QGPL".to_owned())
                },
                Placeholder {
                    name: "n".to_owned(),
                    default: None
                },
                Placeholder {
                    name: "empty".to_owned(),
                    default: Some(String::new())
                },
            ]
        );
        assert_eq!(
            placeholders("SELECT '$' || '{x}' FROM T WHERE X = '${open'"),
            []
        );
    }

    #[test]
    fn filling_puts_values_in_and_defaults_where_none_is_given_r_sql_26() {
        let sql = "WHERE L = '${lib:QGPL}' AND N = ${n} AND U = '${user}'";
        let filled = fill(sql, |name| (name == "user").then(|| "ÆØÅ".to_owned()));
        assert_eq!(filled, "WHERE L = 'QGPL' AND N =  AND U = 'ÆØÅ'");
    }

    #[test]
    fn every_starter_has_a_title_tags_about_needs_and_sql_r_sql_27() {
        for (name, text) in STARTERS {
            let s = Snippet::parse(text, name);
            assert_ne!(s.title, name, "{name} has its own title");
            assert!(s.tags.iter().any(|t| t == "did you know"), "{name}");
            assert!(
                !s.about.is_empty() && s.needs.starts_with("IBM i 7."),
                "{name}"
            );
            assert!(
                s.sql.to_uppercase().starts_with("SELECT"),
                "{name}: read-only"
            );
            for p in placeholders(&s.sql) {
                assert!(!p.name.contains(' '), "{name}: {}", p.name);
            }
        }
    }
}
