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
}
