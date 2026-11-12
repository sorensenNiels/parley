//! A password typed for a macro's `password` step (R-KEY-4). Parley never stores one
//! (R-NET-7): it lives only as long as the step that asked for it.

/// A password. Its `Debug` hides it, so it cannot reach a log or an error by accident.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Password(String);

impl Password {
    pub const fn new(password: String) -> Self {
        Self(password)
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_empty() {
            "Password(empty)"
        } else {
            "Password(hidden)"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_shows_the_password() {
        let shown = format!("{:?}", Password::new("æøå-secret".into()));
        assert!(!shown.contains("secret"), "{shown}");
    }
}
