//! A password or an answer to a sign-on prompt (R-SSH-4, D44): never shown by `Debug`, wiped when dropped.

use zeroize::Zeroizing;

/// Secret text typed by the user. Parley SSH never keeps one after the sign-on that asked for it.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Secret(Zeroizing<String>);

impl Secret {
    pub fn new(text: String) -> Self {
        Self(Zeroizing::new(text))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    /// The text to edit in place, for a password field: it is wiped with the secret.
    pub fn text_mut(&mut self) -> &mut String {
        &mut self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_empty() {
            "Secret(empty)"
        } else {
            "Secret(hidden)"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_shows_the_secret_r_ssh_4() {
        let shown = format!("{:?}", Secret::new("æøå-hemmelig".into()));
        assert!(!shown.contains("hemmelig"), "{shown}");
    }
}
