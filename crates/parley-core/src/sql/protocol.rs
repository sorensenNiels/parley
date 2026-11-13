//! What Parley and the SQL helper say to each other (R-SQL-11): JSON, one object per line.
//! Requests carry an `id` that their answer repeats; `cancel` has none and no answer.
//! The helper's side is `helpers/sql/src/parley/sql/Helper.java`.

use serde::Deserialize;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::password::Password;

/// The protocol version this Parley speaks; the helper names its own in [`Hello`].
pub const PROTOCOL: u32 = 1;

/// Rows fetched for a query at first, and with each "Fetch more" (R-SQL-8).
pub const FIRST_ROWS: u32 = 1000;

/// A request to the helper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Hello,
    Connect(Connect),
    Run {
        sql: String,
        max_rows: u32,
    },
    Fetch {
        cursor: u64,
        max_rows: u32,
    },
    CloseCursor {
        cursor: u64,
    },
    Autocommit {
        on: bool,
    },
    Commit,
    Rollback,
    /// Stops the statement running now; it has no id and no answer.
    Cancel,
    Close,
}

/// Where and as whom to connect. The password goes to the helper and nowhere else.
#[derive(Clone, PartialEq, Eq)]
pub struct Connect {
    pub host: String,
    pub tls: bool,
    pub user: String,
    pub password: Password,
    /// A company CA certificate (PEM) to trust besides Java's own; empty for none.
    pub ca_file: String,
}

impl std::fmt::Debug for Connect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connect")
            .field("host", &self.host)
            .field("tls", &self.tls)
            .field("user", &self.user)
            .field("password", &self.password)
            .field("ca_file", &self.ca_file)
            .finish()
    }
}

impl Request {
    /// The line to send, without its newline; `id` is ignored for [`Request::Cancel`].
    pub fn line(&self, id: u64) -> String {
        let mut object = serde_json::Map::new();
        let mut put = |key: &str, value: Value| {
            object.insert(key.to_owned(), value);
        };
        let op = match self {
            Self::Hello => "hello",
            Self::Connect(c) => {
                put("host", c.host.clone().into());
                put("tls", c.tls.into());
                put("user", c.user.clone().into());
                put("password", c.password.expose().into());
                put("ca_file", c.ca_file.clone().into());
                "connect"
            }
            Self::Run { sql, max_rows } => {
                put("sql", sql.clone().into());
                put("max_rows", (*max_rows).into());
                "run"
            }
            Self::Fetch { cursor, max_rows } => {
                put("cursor", (*cursor).into());
                put("max_rows", (*max_rows).into());
                "fetch"
            }
            Self::CloseCursor { cursor } => {
                put("cursor", (*cursor).into());
                "close_cursor"
            }
            Self::Autocommit { on } => {
                put("on", (*on).into());
                "autocommit"
            }
            Self::Commit => "commit",
            Self::Rollback => "rollback",
            Self::Cancel => "cancel",
            Self::Close => "close",
        };
        put("op", op.into());
        if !matches!(self, Self::Cancel) {
            put("id", id.into());
        }
        Value::Object(object).to_string()
    }

    /// Whether the helper answers it.
    pub const fn answered(&self) -> bool {
        !matches!(self, Self::Cancel)
    }
}

/// An SQL error as the host or `JTOpen` gave it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SqlError {
    pub message: String,
    pub sqlstate: Option<String>,
    #[serde(default)]
    pub sqlcode: i64,
}

impl std::fmt::Display for SqlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.sqlstate {
            Some(state) => write!(
                f,
                "{} (SQLSTATE {state}, SQLCODE {})",
                self.message, self.sqlcode
            ),
            None => f.write_str(&self.message),
        }
    }
}

/// One line from the helper: the request it answers and its outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// `None` when the helper could not read the request it answers.
    pub id: Option<u64>,
    pub outcome: std::result::Result<Value, SqlError>,
}

#[derive(Deserialize)]
struct RawAnswer {
    id: Option<u64>,
    ok: Option<Value>,
    error: Option<SqlError>,
}

impl Answer {
    /// Reads one line from the helper.
    pub fn parse(line: &str) -> Result<Self> {
        let raw: RawAnswer = serde_json::from_str(line)
            .map_err(|e| Error::SqlHelper(format!("an answer Parley cannot read ({e})")))?;
        let outcome = match (raw.ok, raw.error) {
            (Some(value), None) => Ok(value),
            (None, Some(error)) => Err(error),
            _ => {
                return Err(Error::SqlHelper(
                    "an answer with neither or both of ok and error".to_owned(),
                ));
            }
        };
        Ok(Self {
            id: raw.id,
            outcome,
        })
    }

    /// The outcome read as `T`, or the SQL error as an [`Error::Sql`].
    pub fn into_result<T: for<'de> Deserialize<'de>>(self) -> Result<T> {
        let value = self.outcome.map_err(Error::Sql)?;
        serde_json::from_value(value)
            .map_err(|e| Error::SqlHelper(format!("an answer of an unexpected shape ({e})")))
    }
}

/// The answer to [`Request::Hello`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Hello {
    pub protocol: u32,
    pub java: String,
    pub jtopen: String,
}

/// The answer to [`Request::Connect`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Connected {
    /// The database job, as number/user/name.
    pub job: String,
    pub release: String,
    pub user: String,
}

/// A column of a query's result.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Column {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub length: i64,
    pub scale: i64,
    pub nullable: bool,
    /// Right-aligned in the grid (R-SQL-8).
    pub numeric: bool,
}

/// A row: each value as text, `None` for NULL.
pub type Row = Vec<Option<String>>;

/// What a statement gave.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Outcome {
    /// A query: its columns, the first rows, and whether more can be fetched from `cursor`.
    Rows {
        cursor: u64,
        columns: Vec<Column>,
        rows: Vec<Row>,
        more: bool,
    },
    /// Any other statement: the rows it changed.
    Count { count: i64 },
}

/// The answer to a request that only says it was done: autocommit, commit, rollback, close.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Done {}

/// The answer to [`Request::Fetch`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MoreRows {
    pub rows: Vec<Row>,
    pub more: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_one_json_object_each_r_sql_11() {
        let run = Request::Run {
            sql: "SELECT 'æøå' FROM \"T\"\n".to_owned(),
            max_rows: 1000,
        };
        let line = run.line(3);
        assert!(!line.contains('\n'), "one line: {line}");
        let value: Value = serde_json::from_str(&line).expect("JSON");
        assert_eq!(value["op"], "run");
        assert_eq!(value["id"], 3);
        assert_eq!(value["sql"], "SELECT 'æøå' FROM \"T\"\n");
        let cancel: Value = serde_json::from_str(&Request::Cancel.line(9)).expect("JSON");
        assert_eq!(
            cancel,
            serde_json::json!({"op": "cancel"}),
            "no id, no answer"
        );
        assert!(!Request::Cancel.answered());
    }

    #[test]
    fn the_password_goes_in_the_connect_line_but_never_in_debug_output_r_sql_11() {
        let connect = Request::Connect(Connect {
            host: "ibmi".to_owned(),
            tls: true,
            user: "USR".to_owned(),
            password: Password::new("TopSecret9".to_owned()),
            ca_file: String::new(),
        });
        assert!(connect.line(1).contains("\"password\":\"TopSecret9\""));
        assert!(!format!("{connect:?}").contains("TopSecret9"));
    }

    #[test]
    fn answers_the_helper_gave_are_read_r_sql_11() {
        // As the helper answered on a test run (JTOpen 21.0.7, Java 11).
        let hello = Answer::parse(
            r#"{"id":1,"ok":{"protocol":1,"java":"11.0.33","jtopen":"Open Source Software, JTOpen 21.0.7"}}"#,
        )
        .expect("an answer");
        assert_eq!(hello.id, Some(1));
        let hello: Hello = hello.into_result().expect("hello");
        assert_eq!(hello.protocol, PROTOCOL);

        let refused = Answer::parse(
            r#"{"id":2,"error":{"message":"The application requester cannot establish the connection. (connect timed out)","sqlstate":"08001","sqlcode":-99999}}"#,
        )
        .expect("an answer");
        let error = refused
            .into_result::<Connected>()
            .expect_err("an SQL error");
        assert!(error.to_string().contains("SQLSTATE 08001"), "{error}");

        let unreadable = Answer::parse(
            r#"{"id":null,"error":{"message":"invalid JSON at character 0","sqlstate":null,"sqlcode":0}}"#,
        )
        .expect("an answer");
        assert_eq!(unreadable.id, None);
    }

    #[test]
    fn rows_keep_null_apart_from_empty_and_decimals_exact_r_sql_4() {
        let line = r#"{"id":3,"ok":{"kind":"rows","cursor":1,"columns":[
            {"name":"AMOUNT","type":"DECIMAL","length":31,"scale":6,"nullable":true,"numeric":true},
            {"name":"NAME","type":"VARCHAR","length":10,"scale":0,"nullable":true,"numeric":false}],
            "rows":[["12345678901234567890.123456","æøå"],[null,""]],"more":false}}"#
            .replace('\n', "");
        let outcome: Outcome = Answer::parse(&line)
            .expect("an answer")
            .into_result()
            .expect("rows");
        let Outcome::Rows {
            columns,
            rows,
            more,
            ..
        } = outcome
        else {
            panic!("rows expected");
        };
        assert!(columns[0].numeric && !columns[1].numeric);
        assert_eq!(rows[0][0].as_deref(), Some("12345678901234567890.123456"));
        assert_eq!(rows[1], [None, Some(String::new())]);
        assert!(!more);
        let count: Outcome = Answer::parse(r#"{"id":4,"ok":{"kind":"count","count":5}}"#)
            .expect("an answer")
            .into_result()
            .expect("a count");
        assert_eq!(count, Outcome::Count { count: 5 });
    }

    #[test]
    fn broken_answers_are_errors_not_panics_r_sql_11() {
        for line in [
            "",
            "{",
            "[]",
            r#"{"id":1}"#,
            r#"{"id":1,"ok":1,"error":{"message":"x"}}"#,
        ] {
            assert!(Answer::parse(line).is_err(), "{line}");
        }
        let wrong_shape = Answer::parse(r#"{"id":1,"ok":{"protocol":"one"}}"#).expect("an answer");
        assert!(wrong_shape.into_result::<Hello>().is_err());
    }
}
