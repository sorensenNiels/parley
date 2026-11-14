//! Two results compared (R-SQL-41, D23): rows matched by key columns; what is only in one, and
//! rows whose other values differ, with the differing values marked.

use std::collections::{BTreeMap, VecDeque};

use super::protocol::{Column, Row};

/// The first column of a comparison, saying where each row comes from.
pub const SIDE_COLUMN: &str = "COMPARED";

/// What a row of a comparison is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    OnlyFirst,
    OnlySecond,
    /// The first result's row of a pair that differs.
    First,
    /// The second result's row of that pair.
    Second,
}

impl Side {
    pub fn label(self) -> &'static str {
        match self {
            Side::OnlyFirst => "only in first",
            Side::OnlySecond => "only in second",
            Side::First => "first",
            Side::Second => "second",
        }
    }
}

/// A comparison, as a result: [`SIDE_COLUMN`] and the first result's columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    /// The values that differ: (row, column) in `rows` and `columns`.
    pub marked: Vec<(usize, usize)>,
    /// Rows that matched and are the same.
    pub same: usize,
}

/// Why two results cannot be compared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mismatch {
    /// Their column names differ.
    Columns,
    /// No key column, or one the results do not have.
    Key,
}

/// `first` and `second` compared, rows matched by the columns named in `keys` (the first
/// column when empty). The columns are matched by name, in any order. Rows with the same key
/// are paired in order.
pub fn compare(
    first: (&[Column], &[Row]),
    second: (&[Column], &[Row]),
    keys: &[String],
) -> Result<Comparison, Mismatch> {
    let (columns, rows_a) = first;
    let (columns_b, rows_b) = second;
    // For each of the first result's columns, its place in the second.
    let place: Vec<usize> = columns
        .iter()
        .map(|c| columns_b.iter().position(|b| b.name == c.name))
        .collect::<Option<_>>()
        .ok_or(Mismatch::Columns)?;
    if columns.len() != columns_b.len() {
        return Err(Mismatch::Columns);
    }
    let keys: Vec<usize> = if keys.is_empty() {
        vec![0]
    } else {
        keys.iter()
            .map(|k| columns.iter().position(|c| c.name == *k))
            .collect::<Option<_>>()
            .ok_or(Mismatch::Key)?
    };
    if columns.is_empty() {
        return Err(Mismatch::Key);
    }
    // The second result's rows in the first's column order.
    let aligned = |row: &Row| -> Row {
        place
            .iter()
            .map(|&p| row.get(p).cloned().flatten())
            .collect()
    };
    let key_of = |row: &Row| -> Row {
        keys.iter()
            .map(|&k| row.get(k).cloned().flatten())
            .collect()
    };
    let mut waiting: BTreeMap<Row, VecDeque<Row>> = BTreeMap::new();
    let mut order = Vec::new();
    for row in rows_b {
        let row = aligned(row);
        let key = key_of(&row);
        order.push(key.clone());
        waiting.entry(key).or_default().push_back(row);
    }
    let mut out = Comparison {
        columns: Vec::with_capacity(columns.len() + 1),
        rows: Vec::new(),
        marked: Vec::new(),
        same: 0,
    };
    out.columns.push(Column {
        name: SIDE_COLUMN.to_owned(),
        type_name: "VARCHAR".to_owned(),
        length: 14,
        scale: 0,
        nullable: false,
        numeric: false,
        field: None,
        schema: None,
        table: None,
        heading: None,
        text: None,
    });
    out.columns.extend(columns.iter().cloned());
    let labelled = |side: Side, row: &Row| -> Row {
        std::iter::once(Some(side.label().to_owned()))
            .chain(row.iter().cloned())
            .collect()
    };
    for row in rows_a {
        let key = key_of(row);
        match waiting.get_mut(&key).and_then(VecDeque::pop_front) {
            None => out.rows.push(labelled(Side::OnlyFirst, row)),
            Some(other) if other == *row => out.same += 1,
            Some(other) => {
                let at = out.rows.len();
                for c in 0..columns.len() {
                    if row.get(c) != other.get(c) {
                        out.marked.push((at, c + 1));
                        out.marked.push((at + 1, c + 1));
                    }
                }
                out.rows.push(labelled(Side::First, row));
                out.rows.push(labelled(Side::Second, &other));
            }
        }
    }
    for key in order {
        if let Some(row) = waiting.get_mut(&key).and_then(VecDeque::pop_front) {
            out.rows.push(labelled(Side::OnlySecond, &row));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns(names: &[&str]) -> Vec<Column> {
        names
            .iter()
            .map(|n| Column {
                name: (*n).to_owned(),
                type_name: "VARCHAR".to_owned(),
                length: 10,
                scale: 0,
                nullable: true,
                numeric: false,
                field: None,
                schema: None,
                table: None,
                heading: None,
                text: None,
            })
            .collect()
    }

    fn row(values: &[Option<&str>]) -> Row {
        values.iter().map(|v| v.map(str::to_owned)).collect()
    }

    #[test]
    fn rows_are_matched_by_key_in_any_order_and_differences_marked_r_sql_41() {
        let a_cols = columns(&["ID", "NAVN"]);
        let b_cols = columns(&["NAVN", "ID"]);
        let a = [
            row(&[Some("1"), Some("æøå")]),
            row(&[Some("2"), Some("to")]),
            row(&[Some("3"), None]),
        ];
        let b = [
            row(&[Some("TO"), Some("2")]),
            row(&[Some("æøå"), Some("1")]),
            row(&[Some("fire"), Some("4")]),
        ];
        let c = compare((&a_cols, &a), (&b_cols, &b), &[]).expect("comparable");
        assert_eq!(c.same, 1);
        let sides: Vec<_> = c
            .rows
            .iter()
            .map(|r| r[0].clone().unwrap_or_default())
            .collect();
        assert_eq!(
            sides,
            ["first", "second", "only in first", "only in second"]
        );
        assert_eq!(c.rows[1], row(&[Some("second"), Some("2"), Some("TO")]));
        assert_eq!(c.marked, [(0, 2), (1, 2)], "NAVN differs, ID does not");
        assert_eq!(c.columns[0].name, SIDE_COLUMN);
    }

    #[test]
    fn keys_can_be_several_columns_and_duplicates_pair_in_order_r_sql_41() {
        let cols = columns(&["A", "B", "V"]);
        let a = [
            row(&[Some("x"), Some("1"), Some("v")]),
            row(&[Some("x"), Some("1"), Some("w")]),
        ];
        let b = [
            row(&[Some("x"), Some("1"), Some("v")]),
            row(&[Some("x"), Some("2"), Some("w")]),
        ];
        let keys = ["A".to_owned(), "B".to_owned()];
        let c = compare((&cols, &a), (&cols, &b), &keys).expect("comparable");
        assert_eq!(c.same, 1);
        let sides: Vec<_> = c
            .rows
            .iter()
            .map(|r| r[0].clone().unwrap_or_default())
            .collect();
        assert_eq!(sides, ["only in first", "only in second"]);
    }

    #[test]
    fn different_columns_or_an_unknown_key_cannot_be_compared_r_sql_41() {
        let a = columns(&["A", "B"]);
        assert_eq!(
            compare((&a, &[]), (&columns(&["A", "C"]), &[]), &[]),
            Err(Mismatch::Columns)
        );
        assert_eq!(
            compare((&a, &[]), (&columns(&["A", "B", "C"]), &[]), &[]),
            Err(Mismatch::Columns)
        );
        assert_eq!(
            compare((&a, &[]), (&a, &[]), &["Z".to_owned()]),
            Err(Mismatch::Key)
        );
        assert_eq!(compare((&[], &[]), (&[], &[]), &[]), Err(Mismatch::Key));
        let empty = compare((&a, &[]), (&a, &[]), &[]).expect("comparable");
        assert!(empty.rows.is_empty() && empty.same == 0);
    }
}
