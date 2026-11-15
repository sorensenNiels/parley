//! Data transfer (Phase 15, D31): every row of a query written to a CSV or Excel file as it
//! arrives (R-SQL-71, R-SQL-72).

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use rust_xlsxwriter::{ExcelDateTime, Format as CellFormat, Workbook, Worksheet};

use super::protocol::{Column, Row};
use crate::error::{Error, Result};

/// What a download writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    /// UTF-8 with a byte-order mark, comma-separated, as Export… writes (R-SQL-8).
    #[default]
    Csv,
    /// An `.xlsx` workbook (R-SQL-72).
    Excel,
}

impl Format {
    /// The file name's extension.
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Excel => "xlsx",
        }
    }
}

/// One line of CSV (RFC 4180): fields with a comma, quote or line break are quoted.
pub fn csv_line<'a>(fields: impl IntoIterator<Item = &'a str>) -> String {
    let quoted: Vec<String> = fields
        .into_iter()
        .map(|s| {
            if s.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.to_owned()
            }
        })
        .collect();
    let mut line = quoted.join(",");
    line.push_str("\r\n");
    line
}

/// The rows a sheet holds, its header row included: Excel's limit.
const SHEET_ROWS: u32 = 1_048_576;

/// The most significant digits a number keeps in Excel; longer ones are written as text.
const EXCEL_DIGITS: usize = 15;

/// A download being written: the header first, then each batch of rows as it comes.
#[derive(Debug)]
pub struct Download {
    path: PathBuf,
    rows: u64,
    sink: Sink,
}

#[derive(Debug)]
enum Sink {
    Csv(BufWriter<File>),
    Excel(Box<Excel>),
}

impl Download {
    /// Starts `path` in `format` with a header row of `names` (one per column of `columns`);
    /// an Excel file's first sheet is called `sheet` (made a valid sheet name).
    ///
    /// # Errors
    /// When the file cannot be created.
    pub fn create(
        path: &Path,
        format: Format,
        columns: &[Column],
        names: &[String],
        sheet: &str,
    ) -> Result<Self> {
        let sink = match format {
            Format::Csv => {
                let file = File::create(path).map_err(|e| file_error(path, &e))?;
                let mut out = BufWriter::new(file);
                let header = csv_line(names.iter().map(String::as_str));
                out.write_all(format!("\u{FEFF}{header}").as_bytes())
                    .map_err(|e| file_error(path, &e))?;
                Sink::Csv(out)
            }
            Format::Excel => Sink::Excel(Box::new(Excel::new(columns, names, sheet)?)),
        };
        Ok(Self {
            path: path.to_owned(),
            rows: 0,
            sink,
        })
    }

    /// Writes `rows` after those already written.
    ///
    /// # Errors
    /// When the file cannot be written.
    pub fn write(&mut self, rows: &[Row]) -> Result<()> {
        match &mut self.sink {
            Sink::Csv(out) => {
                for row in rows {
                    let line = csv_line(row.iter().map(|v| v.as_deref().unwrap_or("")));
                    out.write_all(line.as_bytes())
                        .map_err(|e| file_error(&self.path, &e))?;
                }
            }
            Sink::Excel(excel) => {
                for row in rows {
                    excel.row(row)?;
                }
            }
        }
        self.rows += rows.len() as u64;
        Ok(())
    }

    /// How many rows were written.
    pub const fn rows(&self) -> u64 {
        self.rows
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Finishes the file; the rows written.
    ///
    /// # Errors
    /// When the file cannot be written.
    pub fn finish(self) -> Result<u64> {
        match self.sink {
            Sink::Csv(mut out) => out.flush().map_err(|e| file_error(&self.path, &e))?,
            Sink::Excel(mut excel) => excel.workbook.save(&self.path).map_err(|e| {
                Error::Transfer(format!("{} was not saved: {e}", self.path.display()))
            })?,
        }
        Ok(self.rows)
    }

    /// Stops and removes what was written (Cancel, or an error on the way).
    pub fn abandon(self) {
        let path = self.path.clone();
        drop(self);
        // An Excel file is only written by `finish`; a CSV file may not exist after an error.
        // Either way, nothing of a stopped download is left behind, and there is no more to do.
        std::fs::remove_file(path).ok();
    }
}

fn file_error(path: &Path, error: &std::io::Error) -> Error {
    Error::Transfer(format!("{} could not be written: {error}", path.display()))
}

fn excel_error(error: &rust_xlsxwriter::XlsxError) -> Error {
    Error::Transfer(format!("The Excel file could not be written: {error}"))
}

/// How a column's values go into Excel (R-SQL-72).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cell {
    /// A number; DECIMAL, NUMERIC and BIGINT values with more digits than Excel keeps as text.
    Number,
    /// DOUBLE, REAL, FLOAT: a number whatever its digits, as the host had it.
    Float,
    Date,
    Timestamp,
    Text,
}

impl Cell {
    fn of(column: &Column) -> Self {
        let kind = column.type_name.to_ascii_uppercase();
        match kind.as_str() {
            "DATE" => Self::Date,
            "TIMESTAMP" => Self::Timestamp,
            "DOUBLE" | "REAL" | "FLOAT" | "DECFLOAT" => Self::Float,
            _ if column.numeric => Self::Number,
            _ => Self::Text,
        }
    }
}

/// Significant digits in a number written as text: no sign, point, or zeros at either end.
fn significant_digits(value: &str) -> usize {
    let digits: String = value.chars().filter(char::is_ascii_digit).collect();
    let trimmed = digits.trim_start_matches('0');
    let trimmed = if value.contains('.') {
        trimmed.trim_end_matches('0')
    } else {
        trimmed
    };
    trimmed.len()
}

/// An Excel workbook written in constant memory: sheets of at most [`SHEET_ROWS`] rows.
struct Excel {
    workbook: Workbook,
    cells: Vec<Cell>,
    names: Vec<String>,
    sheet: String,
    /// Sheets started, and the next row on the last one.
    sheets: usize,
    next_row: u32,
    header: CellFormat,
    date: CellFormat,
    timestamp: CellFormat,
}

impl std::fmt::Debug for Excel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Excel")
            .field("sheets", &self.sheets)
            .field("next_row", &self.next_row)
            .finish_non_exhaustive()
    }
}

impl Excel {
    fn new(columns: &[Column], names: &[String], sheet: &str) -> Result<Self> {
        let mut excel = Self {
            workbook: Workbook::new(),
            cells: columns.iter().map(Cell::of).collect(),
            names: names.to_vec(),
            sheet: sheet_name(sheet),
            sheets: 0,
            next_row: 0,
            header: CellFormat::new().set_bold(),
            date: CellFormat::new().set_num_format("yyyy-mm-dd"),
            timestamp: CellFormat::new().set_num_format("yyyy-mm-dd hh:mm:ss"),
        };
        excel.new_sheet()?;
        Ok(excel)
    }

    /// The sheet being written.
    fn sheet(&mut self) -> Result<&mut Worksheet> {
        self.workbook
            .worksheet_from_index(self.sheets - 1)
            .map_err(|e| excel_error(&e))
    }

    /// A new sheet with the header row: "Result", then "Result (2)" and on.
    fn new_sheet(&mut self) -> Result<()> {
        self.sheets += 1;
        let name = if self.sheets == 1 {
            self.sheet.clone()
        } else {
            let suffix = format!(" ({})", self.sheets);
            let room = 31 - suffix.chars().count();
            format!(
                "{}{suffix}",
                self.sheet.chars().take(room).collect::<String>()
            )
        };
        let header = self.header.clone();
        let names = self.names.clone();
        let sheet = self
            .workbook
            .add_worksheet_with_constant_memory()
            .map_err(|e| excel_error(&e))?;
        sheet.set_name(&name).map_err(|e| excel_error(&e))?;
        for (c, name) in names.iter().enumerate() {
            let c = u16::try_from(c).map_err(|_| too_many_columns())?;
            sheet
                .write_string_with_format(0, c, name, &header)
                .map_err(|e| excel_error(&e))?;
        }
        sheet.set_freeze_panes(1, 0).map_err(|e| excel_error(&e))?;
        self.next_row = 1;
        Ok(())
    }

    fn row(&mut self, row: &Row) -> Result<()> {
        if self.next_row == SHEET_ROWS {
            self.new_sheet()?;
        }
        let r = self.next_row;
        let cells = self.cells.clone();
        let (date, timestamp) = (self.date.clone(), self.timestamp.clone());
        let sheet = self.sheet()?;
        for (c, value) in row.iter().enumerate() {
            let Some(value) = value.as_deref() else {
                continue; // NULL: an empty cell.
            };
            let column = u16::try_from(c).map_err(|_| too_many_columns())?;
            let kind = cells.get(c).copied().unwrap_or(Cell::Text);
            let written = match kind {
                Cell::Number | Cell::Float => match value.trim().parse::<f64>() {
                    Ok(number)
                        if kind == Cell::Float || significant_digits(value) <= EXCEL_DIGITS =>
                    {
                        sheet.write_number(r, column, number)
                    }
                    _ => sheet.write_string(r, column, value),
                },
                Cell::Date | Cell::Timestamp => match ExcelDateTime::parse_from_str(value) {
                    Ok(when) => {
                        let format = if kind == Cell::Date {
                            &date
                        } else {
                            &timestamp
                        };
                        sheet.write_datetime_with_format(r, column, &when, format)
                    }
                    Err(_) => sheet.write_string(r, column, value),
                },
                Cell::Text => sheet.write_string(r, column, value),
            };
            written.map_err(|e| excel_error(&e))?;
        }
        self.next_row += 1;
        Ok(())
    }
}

fn too_many_columns() -> Error {
    Error::Transfer("An Excel sheet holds at most 16 384 columns.".to_owned())
}

/// `name` as an Excel sheet may be called: no `[]:*?/\`, at most 31 characters, "Result" when
/// empty.
pub fn sheet_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| if "[]:*?/\\".contains(c) { '_' } else { c })
        .take(31)
        .collect();
    let clean = clean.trim().trim_matches('\'').to_owned();
    if clean.is_empty() {
        "Result".to_owned()
    } else {
        clean
    }
}

/// The name a download is offered under: the table the columns come from, else "query".
pub fn suggested_name(columns: &[Column]) -> String {
    columns
        .first()
        .and_then(|c| c.table.clone())
        .filter(|_| super::export::source_table(columns).is_some())
        .unwrap_or_else(|| "query".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str, type_name: &str, numeric: bool) -> Column {
        Column {
            name: name.to_owned(),
            type_name: type_name.to_owned(),
            length: 0,
            scale: 0,
            nullable: true,
            numeric,
            field: None,
            schema: Some("LIB".to_owned()),
            table: Some("KUNDER".to_owned()),
            heading: None,
            text: None,
        }
    }

    fn row(cells: &[Option<&str>]) -> Row {
        cells.iter().map(|c| c.map(str::to_owned)).collect()
    }

    /// A file in a fresh folder under the target directory, removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("parley-transfer-{}-{name}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("a scratch folder");
            Self(dir)
        }
        fn file(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            // A test's scratch folder; leaving it behind harms nothing.
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn csv_quotes_only_what_needs_it_r_sql_8() {
        assert_eq!(
            csv_line(["a", "b,c", "say \"hi\"", "æøå"]),
            "a,\"b,c\",\"say \"\"hi\"\"\",æøå\r\n"
        );
        assert_eq!(csv_line(["line\nbreak", ""]), "\"line\nbreak\",\r\n");
    }

    #[test]
    fn a_csv_download_writes_the_header_then_each_batch_as_it_comes_r_sql_71() {
        let scratch = Scratch::new("csv");
        let path = scratch.file("kunder.csv");
        let columns = vec![
            column("NAVN", "VARCHAR", false),
            column("BELOB", "DECIMAL", true),
        ];
        let names = vec!["NAVN".to_owned(), "BELØB".to_owned()];
        let mut download =
            Download::create(&path, Format::Csv, &columns, &names, "KUNDER").expect("created");
        download
            .write(&[row(&[Some("Søren, Å"), Some("12.50")])])
            .expect("written");
        download
            .write(&[row(&[None, Some("-1")])])
            .expect("written");
        assert_eq!(download.rows(), 2);
        assert_eq!(download.finish().expect("finished"), 2);
        let text = std::fs::read_to_string(&path).expect("the file");
        assert_eq!(text, "\u{FEFF}NAVN,BELØB\r\n\"Søren, Å\",12.50\r\n,-1\r\n");
    }

    #[test]
    fn a_stopped_download_leaves_no_file_r_sql_71() {
        let scratch = Scratch::new("abandon");
        for format in [Format::Csv, Format::Excel] {
            let path = scratch.file(&format!("stopped.{}", format.extension()));
            let columns = vec![column("A", "INTEGER", true)];
            let mut download =
                Download::create(&path, format, &columns, &["A".to_owned()], "T").expect("created");
            download.write(&[row(&[Some("1")])]).expect("written");
            download.abandon();
            assert!(!path.exists(), "{format:?} removed");
        }
    }

    #[test]
    fn a_folder_that_is_not_there_is_an_error_naming_the_file_r_sql_71() {
        let path = Path::new("/no/such/folder/æøå.csv");
        let err = Download::create(path, Format::Csv, &[], &[], "T").expect_err("no folder");
        assert!(err.to_string().contains("æøå.csv"), "{err}");
    }

    #[test]
    fn an_excel_download_is_a_workbook_r_sql_72() {
        let scratch = Scratch::new("xlsx");
        let path = scratch.file("kunder.xlsx");
        let columns = vec![
            column("NAVN", "VARCHAR", false),
            column("BELOB", "DECIMAL", true),
            column("DATO", "DATE", false),
            column("TID", "TIMESTAMP", false),
            column("KL", "TIME", false),
            column("ID", "BIGINT", true),
        ];
        let names: Vec<String> = columns.iter().map(|c| c.name.clone()).collect();
        let mut download =
            Download::create(&path, Format::Excel, &columns, &names, "KUNDER").expect("created");
        download
            .write(&[
                row(&[
                    Some("æøå"),
                    Some("1234.50"),
                    Some("2026-11-14"),
                    Some("2026-11-14 15:59:07.123456"),
                    Some("15:59:07"),
                    Some("1234567890123456789"),
                ]),
                row(&[None, None, Some("not a date"), None, None, Some("7")]),
            ])
            .expect("written");
        assert_eq!(download.finish().expect("finished"), 2);
        let bytes = std::fs::read(&path).expect("the file");
        assert!(bytes.starts_with(b"PK"), "a zip, as .xlsx files are");
    }

    #[test]
    fn numbers_too_long_for_excel_are_counted_by_their_significant_digits_r_sql_72() {
        assert_eq!(significant_digits("-0012.3400"), 4);
        assert_eq!(significant_digits("1000"), 4, "zeros before no point count");
        assert_eq!(significant_digits("1234567890123456"), 16);
        assert_eq!(significant_digits("0.000000000000000001"), 1);
        assert_eq!(significant_digits(""), 0);
    }

    #[test]
    fn sheet_names_follow_excels_rules_r_sql_72() {
        assert_eq!(sheet_name("A/B:C*D?[E]\\"), "A_B_C_D__E__");
        assert_eq!(sheet_name(""), "Result");
        assert_eq!(sheet_name("'quoted'"), "quoted");
        assert_eq!(sheet_name(&"Æ".repeat(40)).chars().count(), 31);
    }

    #[test]
    fn a_download_is_named_after_its_one_table_or_query_r_sql_71() {
        let columns = vec![column("A", "INTEGER", true)];
        assert_eq!(suggested_name(&columns), "KUNDER");
        let mut other = column("B", "INTEGER", true);
        other.table = Some("ORDRER".to_owned());
        assert_eq!(suggested_name(&[columns[0].clone(), other]), "query");
        assert_eq!(suggested_name(&[]), "query");
    }
}
