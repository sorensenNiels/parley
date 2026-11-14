//! Add-ons (D15, R-ADD-1): parts of Parley not everyone needs, each turned on or off in
//! Settings ▸ Add-ons and not loaded while off. As Tortuga's add-ons (its D78).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// An add-on. The one list of them: the rest of the UI asks only this enum whether a part
/// of Parley is there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddOn {
    /// Recording and playing keyboard macros (R-KEY-3, R-KEY-4, R-ADD-2). On by default.
    Macros,
    /// The SQL window over a Java helper with `JTOpen` (R-SQL-10, D20). Off by default.
    Sql,
    /// The SQL assistant in the SQL window (R-SQL-44, R-ADD-3, D25). Off by default.
    Assistant,
}

impl AddOn {
    /// Every add-on, in the order Settings lists them.
    pub const ALL: [Self; 3] = [Self::Macros, Self::Sql, Self::Assistant];

    /// The name the settings keep it under.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Macros => "macros",
            Self::Sql => "sql",
            Self::Assistant => "sql-assistant",
        }
    }

    /// The name Settings shows.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Macros => "Macros",
            Self::Sql => "SQL",
            Self::Assistant => "SQL assistant",
        }
    }

    /// One line on what it does.
    pub const fn about(self) -> &'static str {
        match self {
            Self::Macros => "Record keystrokes and play them back, also when a session connects.",
            Self::Sql => {
                "Run SQL on the session's host in a window of its own. Needs Java 11 or later."
            }
            Self::Assistant => {
                "Ask Claude to write SQL from plain words in the SQL window; sends your question and \
                 table and column names to Anthropic. Needs the SQL add-on and ANTHROPIC_API_KEY."
            }
        }
    }

    /// Whether it is on until turned off.
    pub const fn on_by_default(self) -> bool {
        match self {
            Self::Macros => true,
            Self::Sql | Self::Assistant => false,
        }
    }
}

/// Which add-ons are on. Only what differs from each add-on's default is kept, so an add-on
/// a later version brings starts with its own default (R-ADD-1).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AddOnSettings {
    /// Ids of add-ons that are off by default and turned on. Kept as text, so settings
    /// written by a newer version with add-ons this one doesn't know still load.
    pub enabled: BTreeSet<String>,
    /// Ids of add-ons that are on by default and turned off.
    pub disabled: BTreeSet<String>,
}

impl AddOnSettings {
    pub fn enabled(&self, addon: AddOn) -> bool {
        if addon.on_by_default() {
            !self.disabled.contains(addon.id())
        } else {
            self.enabled.contains(addon.id())
        }
    }

    pub fn set_enabled(&mut self, addon: AddOn, enabled: bool) {
        let (differs, other) = if addon.on_by_default() {
            (&mut self.disabled, &mut self.enabled)
        } else {
            (&mut self.enabled, &mut self.disabled)
        };
        other.remove(addon.id());
        if enabled == addon.on_by_default() {
            differs.remove(addon.id());
        } else {
            differs.insert(addon.id().to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R-ADD-1: each add-on starts with its default; macros are on (R-ADD-2).
    #[test]
    fn add_ons_start_with_their_default_r_add_1() {
        let settings = AddOnSettings::default();
        for addon in AddOn::ALL {
            assert_eq!(settings.enabled(addon), addon.on_by_default());
        }
        assert!(settings.enabled(AddOn::Macros));
        assert!(
            !settings.enabled(AddOn::Sql),
            "SQL is off until turned on (R-SQL-10)"
        );
        assert!(
            !settings.enabled(AddOn::Assistant),
            "nothing is sent to Anthropic until turned on (R-ADD-3)"
        );
    }

    /// R-ADD-1: only a change from the default is kept, and turning it back forgets it.
    #[test]
    fn only_a_change_from_the_default_is_kept_r_add_1() {
        let mut settings = AddOnSettings::default();
        settings.set_enabled(AddOn::Macros, false);
        assert!(!settings.enabled(AddOn::Macros));
        assert_eq!(settings.disabled, BTreeSet::from(["macros".to_owned()]));
        assert!(settings.enabled.is_empty());
        settings.set_enabled(AddOn::Macros, true);
        assert_eq!(settings, AddOnSettings::default());
    }

    /// R-ADD-1: ids this version doesn't know still load, and are kept.
    #[test]
    fn unknown_add_on_ids_still_load_r_add_1() {
        let text = r#"{"enabled": ["from-the-future"], "disabled": ["macros"]}"#;
        let settings: AddOnSettings = serde_json::from_str(text).expect("parse");
        assert!(!settings.enabled(AddOn::Macros));
        assert!(settings.enabled.contains("from-the-future"));
    }
}
