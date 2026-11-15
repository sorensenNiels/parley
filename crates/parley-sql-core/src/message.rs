//! IBM i messages in the SQL window (R-SQL-31): the message ID in an error, and the SQL that
//! reads a message's second-level text (cause and recovery) or the SQL job's job log.

/// The message ID an error's text starts with, as `JTOpen` writes it: `[SQL0204] T in X type
/// *FILE not found.` Three letters and four digits or hex letters (CPF2110, SQL0204, CPD0C4F).
pub fn message_id(text: &str) -> Option<&str> {
    let id = text.trim_start().strip_prefix('[')?.split_once(']')?.0;
    let (prefix, number) = (id.get(..3)?, id.get(3..)?);
    let valid = id.len() == 7
        && prefix.bytes().all(|b| b.is_ascii_uppercase())
        && number
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'A'..=b'F'));
    valid.then_some(id)
}

/// What the host's message says it stopped at, as `JTOpen` writes it (R-SQL-75): the token of
/// SQL0104, the keyword of SQL0199, the column of SQL0206 and SQL0205, the object of SQL0204.
/// `None` for anything else, and for a token like `<END-OF-STATEMENT>`, which has no place.
pub fn named_in(message: &str) -> Option<&str> {
    let text = message.split_once("] ").map_or(message, |(_, rest)| rest);
    let before = |end: &str| text.split_once(end).map(|(name, _)| name);
    let after = |start: &str, end: &str| {
        text.split_once(start)
            .and_then(|(_, rest)| rest.split_once(end))
            .map(|(name, _)| name)
    };
    let name = match message_id(message)? {
        "SQL0104" => after("Token ", " was not valid"),
        "SQL0199" => after("Keyword ", " not expected"),
        "SQL0206" => after("Column or global variable ", " not found"),
        "SQL0205" => after("Column ", " not in table"),
        "SQL0204" => before(" in "),
        _ => None,
    }?
    .trim();
    (!name.is_empty() && !name.starts_with('<')).then_some(name)
}

/// Where in `statement` the error `message` names is (R-SQL-75): the first place of what
/// [`named_in`] reads, as a whole word (or the symbol itself), ignoring case, outside strings
/// and comments; a delimited name matches its quotes too. A byte range of `statement`.
pub fn error_place(statement: &str, message: &str) -> Option<std::ops::Range<usize>> {
    let name = named_in(message)?;
    let skipped: Vec<std::ops::Range<usize>> = super::text::highlight(statement)
        .into_iter()
        .filter(|(_, kind)| matches!(kind, super::text::Kind::String | super::text::Kind::Comment))
        .map(|(range, _)| range)
        .collect();
    let word = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'#' | b'$' | b'@');
    let is_word = name.bytes().all(word);
    // `to_lowercase` keeps byte offsets only where case changes keep lengths (not for `İ`,
    // which becomes three bytes): there, only ASCII letters are matched without case.
    let (lower, wanted) = match statement.to_lowercase() {
        lower if lower.len() == statement.len() => (lower, name.to_lowercase()),
        _ => (statement.to_ascii_lowercase(), name.to_ascii_lowercase()),
    };
    let bytes = statement.as_bytes();
    let mut from = 0;
    while let Some(found) = lower.get(from..)?.find(&wanted) {
        let start = from + found;
        let end = start + wanted.len();
        from = end;
        if skipped.iter().any(|r| r.start <= start && start < r.end) {
            continue;
        }
        let quoted =
            start > 0 && bytes.get(start - 1) == Some(&b'"') && bytes.get(end) == Some(&b'"');
        if quoted {
            return Some(start - 1..end + 1);
        }
        let alone = !is_word
            || (bytes.get(start.wrapping_sub(1)).is_none_or(|&b| !word(b))
                && bytes.get(end).is_none_or(|&b| !word(b)));
        if alone {
            return Some(start..end);
        }
    }
    None
}

/// The SQL job's newest message `id` from its own job log, where the message's values are
/// filled in. `id` must have come through [`message_id`]: it is put in the text as it is.
pub fn from_job_log(id: &str) -> String {
    format!(
        "SELECT MESSAGE_TEXT, MESSAGE_SECOND_LEVEL_TEXT FROM TABLE(QSYS2.JOBLOG_INFO('*')) \
         WHERE MESSAGE_ID = '{id}' ORDER BY ORDINAL_POSITION DESC FETCH FIRST 1 ROW ONLY"
    )
}

/// Message `id` from its message file: QSQLMSG for SQL messages, QCPFMSG for the rest; its
/// values are shown as `&1`. `id` must have come through [`message_id`].
pub fn from_message_file(id: &str) -> String {
    let file = if id.starts_with("SQ") {
        "QSQLMSG"
    } else {
        "QCPFMSG"
    };
    format!(
        "SELECT MESSAGE_TEXT, MESSAGE_SECOND_LEVEL_TEXT FROM QSYS2.MESSAGE_FILE_DATA \
         WHERE MESSAGE_FILE_LIBRARY = 'QSYS' AND MESSAGE_FILE = '{file}' AND MESSAGE_ID = '{id}'"
    )
}

/// The SQL job's newest messages, newest first.
pub const JOB_LOG: &str = "SELECT MESSAGE_TIMESTAMP, MESSAGE_ID, MESSAGE_TYPE, SEVERITY, MESSAGE_TEXT, \
    MESSAGE_SECOND_LEVEL_TEXT, FROM_PROGRAM FROM TABLE(QSYS2.JOBLOG_INFO('*')) \
    ORDER BY ORDINAL_POSITION DESC FETCH FIRST 100 ROWS ONLY";

/// Second-level text with IBM i's format codes made into lines: `&N` a new line, `&P` a new
/// line indented a little, `&B` indented more.
pub fn second_level(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(rest.get(..at).unwrap_or_default());
        let after = rest.get(at + 1..).unwrap_or_default();
        let break_with = match after.chars().next() {
            Some('N') => Some(""),
            Some('P') => Some("  "),
            Some('B') => Some("    "),
            _ => None,
        };
        match break_with {
            Some(indent)
                if (out.is_empty() || out.ends_with(' '))
                    && after
                        .get(1..)
                        .is_some_and(|a| a.is_empty() || a.starts_with(' ')) =>
            {
                out.truncate(out.trim_end().len());
                out.push('\n');
                out.push_str(indent);
                rest = after.get(1..).unwrap_or_default().trim_start();
            }
            _ => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_message_id_is_read_from_jtopens_text_r_sql_31() {
        assert_eq!(
            message_id("[SQL0204] T in QGPL type *FILE not found."),
            Some("SQL0204")
        );
        assert_eq!(message_id(" [CPD0C4F] x"), Some("CPD0C4F"));
        for text in [
            "SQL0204 no brackets",
            "[SQL02] short",
            "[sql0204] lower",
            "[SQL0204",
            "",
            "[ÆØÅ0204] x",
            "[SQL0204'] x",
        ] {
            assert_eq!(message_id(text), None, "{text}");
        }
    }

    #[test]
    fn the_message_file_follows_the_prefix_r_sql_31() {
        assert!(from_message_file("SQL0204").contains("'QSQLMSG'"));
        assert!(from_message_file("CPF2110").contains("'QCPFMSG'"));
        assert!(from_job_log("SQL0204").contains("MESSAGE_ID = 'SQL0204'"));
    }

    #[test]
    fn format_codes_become_lines_r_sql_31() {
        assert_eq!(
            second_level(
                "Cause . . . . . :   T in QGPL æøå. &N Recovery  . . . :   Do one of: &P -- Change it. &B -- Or not. A&B stays."
            ),
            "Cause . . . . . :   T in QGPL æøå.\nRecovery  . . . :   Do one of:\n  -- Change it.\n    -- Or not. A&B stays."
        );
        assert_eq!(second_level(""), "");
        assert_eq!(second_level("ends with &N"), "ends with");
    }

    #[test]
    fn the_name_an_error_stopped_at_is_read_from_its_message_r_sql_75() {
        assert_eq!(
            named_in("[SQL0104] Token FORM was not valid. Valid tokens: FROM INTO."),
            Some("FORM")
        );
        assert_eq!(
            named_in("[SQL0206] Column or global variable NAVNX not found."),
            Some("NAVNX")
        );
        assert_eq!(
            named_in("[SQL0204] KUNDER in LIB type *FILE not found."),
            Some("KUNDER")
        );
        assert_eq!(
            named_in("[SQL0199] Keyword WHERE not expected. Valid tokens: ;."),
            Some("WHERE")
        );
        assert_eq!(
            named_in("[SQL0104] Token <END-OF-STATEMENT> was not valid. Valid tokens: ,."),
            None
        );
        assert_eq!(
            named_in("[SQL0551] Not authorized to object T in L type *FILE."),
            None
        );
        assert_eq!(named_in("connection lost"), None);
    }

    #[test]
    fn the_place_is_a_whole_word_outside_strings_and_comments_r_sql_75() {
        let statement = "SELECT 'form' -- form\n, formx, FORM FORM T";
        let place = error_place(statement, "[SQL0104] Token FORM was not valid.").expect("found");
        assert_eq!(statement.get(place), Some("FORM"));
        let quoted = "SELECT \"navn\" FROM T";
        let place = error_place(
            quoted,
            "[SQL0206] Column or global variable navn not found.",
        )
        .expect("found");
        assert_eq!(quoted.get(place), Some("\"navn\""));
        let symbol = "SELECT A,, B FROM T";
        let place = error_place(symbol, "[SQL0104] Token , was not valid.").expect("found");
        assert_eq!(place, 8..9);
        assert_eq!(
            error_place("SELECT 1", "[SQL0204] KUNDER in LIB type *FILE not found."),
            None
        );
    }

    #[test]
    fn names_with_aeoeaa_are_found_in_their_byte_range_r_sql_75() {
        let statement = "SELECT 'æ', BELØB FROM T";
        let place = error_place(
            statement,
            "[SQL0206] Column or global variable BELØB not found.",
        )
        .expect("found");
        assert_eq!(statement.get(place), Some("BELØB"));
    }

    #[test]
    fn a_letter_whose_lowercase_is_longer_does_not_move_or_lose_the_place_r_sql_75() {
        let statement = "SELECT 'İstanbul', BELØB, navnx FROM T";
        let place = error_place(
            statement,
            "[SQL0206] Column or global variable NAVNX not found.",
        )
        .expect("found");
        assert_eq!(statement.get(place), Some("navnx"));
        let named = "SELECT İL FROM T";
        let place =
            error_place(named, "[SQL0206] Column or global variable İL not found.").expect("found");
        assert_eq!(named.get(place), Some("İL"));
        assert_eq!(
            error_place(
                "SELECT İ FROM T",
                "[SQL0206] Column or global variable X not found."
            ),
            None
        );
    }
}
