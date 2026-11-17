//! A saved connection's database options (R-SQL-97..100), and the `JTOpen` connection properties
//! they become ("IBM Toolbox for Java JDBC properties").

use serde::{Deserialize, Serialize};

/// How table names are qualified (R-SQL-97); System by default (R-SQL-102, D52).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Naming {
    /// `SCHEMA.TABLE`.
    Sql,
    /// `LIBRARY/FILE` and the library list.
    #[default]
    System,
}

/// The isolation level (R-SQL-99), by IBM i's names.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Isolation {
    /// The driver's default.
    #[default]
    Default,
    None,
    Chg,
    Cs,
    All,
    Rr,
}

/// The date format (R-SQL-100).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DateFormat {
    #[default]
    Iso,
    Usa,
    Eur,
    Jis,
    Mdy,
    Dmy,
    Ymd,
    Julian,
}

/// The time format (R-SQL-100).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeFormat {
    #[default]
    Iso,
    Usa,
    Eur,
    Jis,
    Hms,
}

/// The decimal separator (R-SQL-100).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecimalSeparator {
    #[default]
    Period,
    Comma,
}

/// A saved connection's database options; the defaults are what Parley SQL always used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectOptions {
    pub naming: Naming,
    /// Libraries, separated by commas or blanks; empty for the user profile's (R-SQL-98).
    pub libraries: String,
    /// How the SQL window's Autocommit switch starts (R-SQL-99).
    pub autocommit: bool,
    pub isolation: Isolation,
    pub date_format: DateFormat,
    pub time_format: TimeFormat,
    pub decimal_separator: DecimalSeparator,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            naming: Naming::default(),
            libraries: String::new(),
            autocommit: true,
            isolation: Isolation::default(),
            date_format: DateFormat::default(),
            time_format: TimeFormat::default(),
            decimal_separator: DecimalSeparator::default(),
        }
    }
}

impl ConnectOptions {
    /// The `JTOpen` properties these options set, by `JTOpen`'s names and values; the helper puts
    /// them over its own defaults.
    pub fn jdbc_properties(&self) -> Vec<(&'static str, String)> {
        let mut properties = vec![
            (
                "naming",
                match self.naming {
                    Naming::Sql => "sql",
                    Naming::System => "system",
                }
                .to_owned(),
            ),
            ("date format", self.date_format.jdbc().to_owned()),
            ("time format", self.time_format.jdbc().to_owned()),
            (
                "decimal separator",
                match self.decimal_separator {
                    DecimalSeparator::Period => ".",
                    DecimalSeparator::Comma => ",",
                }
                .to_owned(),
            ),
        ];
        let libraries = self.library_list();
        if !libraries.is_empty() {
            properties.push(("libraries", libraries.join(",")));
        }
        let isolation = match self.isolation {
            Isolation::Default => None,
            Isolation::None => Some("none"),
            Isolation::Chg => Some("read uncommitted"),
            Isolation::Cs => Some("read committed"),
            Isolation::All => Some("repeatable read"),
            Isolation::Rr => Some("serializable"),
        };
        if let Some(level) = isolation {
            properties.push(("transaction isolation", level.to_owned()));
        }
        properties
    }

    /// The libraries, upper case, in order, without blanks or empty entries.
    pub fn library_list(&self) -> Vec<String> {
        self.libraries
            .split([',', ' ', '\t'])
            .filter(|l| !l.is_empty())
            .map(str::to_uppercase)
            .collect()
    }
}

impl DateFormat {
    pub const ALL: [(Self, &'static str); 8] = [
        (Self::Iso, "ISO"),
        (Self::Usa, "USA"),
        (Self::Eur, "EUR"),
        (Self::Jis, "JIS"),
        (Self::Mdy, "MDY"),
        (Self::Dmy, "DMY"),
        (Self::Ymd, "YMD"),
        (Self::Julian, "Julian"),
    ];

    const fn jdbc(self) -> &'static str {
        match self {
            Self::Iso => "iso",
            Self::Usa => "usa",
            Self::Eur => "eur",
            Self::Jis => "jis",
            Self::Mdy => "mdy",
            Self::Dmy => "dmy",
            Self::Ymd => "ymd",
            Self::Julian => "julian",
        }
    }
}

impl TimeFormat {
    pub const ALL: [(Self, &'static str); 5] = [
        (Self::Iso, "ISO"),
        (Self::Usa, "USA"),
        (Self::Eur, "EUR"),
        (Self::Jis, "JIS"),
        (Self::Hms, "HMS"),
    ];

    const fn jdbc(self) -> &'static str {
        match self {
            Self::Iso => "iso",
            Self::Usa => "usa",
            Self::Eur => "eur",
            Self::Jis => "jis",
            Self::Hms => "hms",
        }
    }
}

impl Isolation {
    pub const ALL: [(Self, &'static str); 6] = [
        (Self::Default, "Driver's default"),
        (Self::None, "*NONE"),
        (Self::Chg, "*CHG"),
        (Self::Cs, "*CS"),
        (Self::All, "*ALL"),
        (Self::Rr, "*RR"),
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    fn property<'a>(properties: &'a [(&str, String)], name: &str) -> Option<&'a str> {
        properties
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn the_defaults_are_what_parley_sql_always_used_but_system_naming_r_sql_97_r_sql_102() {
        let options = ConnectOptions::default();
        assert!(options.autocommit);
        let p = options.jdbc_properties();
        assert_eq!(property(&p, "naming"), Some("system"));
        assert_eq!(property(&p, "date format"), Some("iso"));
        assert_eq!(property(&p, "time format"), Some("iso"));
        assert_eq!(property(&p, "decimal separator"), Some("."));
        assert_eq!(property(&p, "libraries"), None);
        assert_eq!(property(&p, "transaction isolation"), None);
    }

    #[test]
    fn chosen_options_become_jtopen_properties_r_sql_97_r_sql_99_r_sql_100() {
        let options = ConnectOptions {
            naming: Naming::Sql,
            libraries: " nodejt400, qgpl  æøålib\t".into(),
            autocommit: false,
            isolation: Isolation::Cs,
            date_format: DateFormat::Dmy,
            time_format: TimeFormat::Hms,
            decimal_separator: DecimalSeparator::Comma,
        };
        let p = options.jdbc_properties();
        assert_eq!(property(&p, "naming"), Some("sql"));
        assert_eq!(property(&p, "libraries"), Some("NODEJT400,QGPL,ÆØÅLIB"));
        assert_eq!(
            property(&p, "transaction isolation"),
            Some("read committed")
        );
        assert_eq!(property(&p, "date format"), Some("dmy"));
        assert_eq!(property(&p, "time format"), Some("hms"));
        assert_eq!(property(&p, "decimal separator"), Some(","));
    }

    #[test]
    fn each_isolation_level_has_its_jdbc_name_r_sql_99() {
        let level = |isolation| {
            let options = ConnectOptions {
                isolation,
                ..ConnectOptions::default()
            };
            property(&options.jdbc_properties(), "transaction isolation").map(str::to_owned)
        };
        assert_eq!(level(Isolation::None).as_deref(), Some("none"));
        assert_eq!(level(Isolation::Chg).as_deref(), Some("read uncommitted"));
        assert_eq!(level(Isolation::All).as_deref(), Some("repeatable read"));
        assert_eq!(level(Isolation::Rr).as_deref(), Some("serializable"));
    }

    #[test]
    fn old_saved_connections_read_with_the_defaults() {
        let options: ConnectOptions = serde_json::from_str("{}").expect("reads");
        assert_eq!(options, ConnectOptions::default());
    }

    #[test]
    fn a_connection_saved_with_sql_naming_keeps_it_r_sql_102() {
        let options: ConnectOptions = serde_json::from_str(r#"{"naming":"Sql"}"#).expect("reads");
        assert_eq!(options.naming, Naming::Sql);
    }
}
