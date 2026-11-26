//! A title and tags suggested for a statement saved as a snippet (R-SQL-119).

use crate::complete;

/// A title and tags for `sql`: the first table it names in words, after its verb when it is not
/// a query, and the table's library in lower case as a tag. Empty when it names none.
pub fn suggest(sql: &str) -> (String, Vec<String>) {
    let verb = first_word(sql).to_ascii_uppercase();
    let (object, library) = if verb == "CALL" {
        called(sql)
    } else {
        complete::tables_in(sql)
            .into_iter()
            .next()
            .map(|t| (t.table, t.library))
            .unwrap_or_default()
    };
    if object.is_empty() {
        return (String::new(), Vec::new());
    }
    let words = in_words(&object);
    let title = match verb.as_str() {
        "UPDATE" => format!("Update {words}"),
        "DELETE" => format!("Delete from {words}"),
        "INSERT" => format!("Insert into {words}"),
        "MERGE" => format!("Merge into {words}"),
        "CALL" => format!("Call {words}"),
        _ => capitalised(&words),
    };
    let tags = library
        .map(|l| vec![l.trim_matches('"').to_lowercase()])
        .unwrap_or_default();
    (title, tags)
}

fn first_word(sql: &str) -> &str {
    sql.split(|c: char| !c.is_alphanumeric())
        .find(|w| !w.is_empty())
        .unwrap_or_default()
}

/// The procedure after `CALL`, and its library.
fn called(sql: &str) -> (String, Option<String>) {
    let after = sql.trim_start().get(4..).unwrap_or_default().trim_start();
    let name: String = after
        .chars()
        .take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '$' | '#' | '@' | '.' | '/'))
        .collect();
    match name.rsplit_once(['.', '/']) {
        Some((library, procedure)) => (procedure.to_owned(), Some(library.to_owned())),
        None => (name, None),
    }
}

/// `ACTIVE_JOB_INFO` → `active job info`.
fn in_words(name: &str) -> String {
    name.trim_matches('"')
        .split('_')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn title(sql: &str) -> (String, Vec<String>) {
        suggest(sql)
    }

    fn of(title: &str, tags: &[&str]) -> (String, Vec<String>) {
        (
            title.to_owned(),
            tags.iter().map(|t| (*t).to_owned()).collect(),
        )
    }

    #[test]
    fn a_query_is_titled_by_its_first_table_and_tagged_by_its_library_r_sql_119() {
        assert_eq!(
            title("SELECT * FROM QSYS2.ACTIVE_JOB_INFO a JOIN QSYS2.X b ON 1=1"),
            of("Active job info", &["qsys2"])
        );
        assert_eq!(title("select * from orders"), of("Orders", &[]));
        assert_eq!(title("SELECT * FROM ÆBLER_ØST"), of("Æbler øst", &[]));
        assert_eq!(
            title("WITH a AS (SELECT 1 FROM sysibm.sysdummy1) SELECT * FROM a"),
            of("Sysdummy1", &["sysibm"])
        );
    }

    #[test]
    fn a_change_says_its_verb_r_sql_119() {
        assert_eq!(
            title("UPDATE MYLIB.ORDERS SET X = 1"),
            of("Update orders", &["mylib"])
        );
        assert_eq!(
            title("DELETE FROM ORDERS WHERE 1=0"),
            of("Delete from orders", &[])
        );
        assert_eq!(
            title("INSERT INTO ORDERS VALUES (1)"),
            of("Insert into orders", &[])
        );
        assert_eq!(
            title("CALL QSYS2.QCMDEXC('X')"),
            of("Call qcmdexc", &["qsys2"])
        );
        assert_eq!(title("call my_proc()"), of("Call my proc", &[]));
    }

    #[test]
    fn nothing_named_suggests_nothing_r_sql_119() {
        for sql in ["", "VALUES 1", "SET SCHEMA X", "CALL", "   "] {
            let (t, tags) = title(sql);
            assert_eq!((t.as_str(), tags.len()), ("", 0), "{sql}");
        }
    }
}
