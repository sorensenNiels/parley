//! Snippets' placeholders (R-SSH-49, D50): `{{name}}` and `{{name:default}}` are asked for when a snippet is sent,
//! each name once; `{{{{` sends a literal `{{`. Anything else between braces is sent as written.

/// A placeholder: its name and the default written with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placeholder {
    pub name: String,
    pub default: Option<String>,
}

/// One piece of a snippet's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Piece<'a> {
    Text(&'a str),
    Field {
        name: &'a str,
        default: Option<&'a str>,
    },
}

/// The placeholders of `text`, each name once, in the order they first appear; the first default given wins.
pub fn placeholders(text: &str) -> Vec<Placeholder> {
    let mut found: Vec<Placeholder> = Vec::new();
    for piece in pieces(text) {
        if let Piece::Field { name, default } = piece {
            match found.iter_mut().find(|p| p.name == name) {
                Some(p) if p.default.is_none() => p.default = default.map(str::to_owned),
                Some(_) => {}
                None => found.push(Placeholder {
                    name: name.to_owned(),
                    default: default.map(str::to_owned),
                }),
            }
        }
    }
    found
}

/// `text` with each placeholder replaced by `value(name)`, or its default, or nothing.
pub fn fill(text: &str, value: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(text.len());
    for piece in pieces(text) {
        match piece {
            Piece::Text(t) => out.push_str(t),
            Piece::Field { name, default } => {
                out.push_str(
                    &value(name).unwrap_or_else(|| default.unwrap_or_default().to_owned()),
                );
            }
        }
    }
    out
}

/// A name is letters, digits, `_`, `-`, `.` and spaces inside, not empty.
fn is_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ' '))
}

fn pieces(text: &str) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("{{") {
        let after = &rest[at + 2..];
        if let Some(literal) = after.strip_prefix("{{") {
            pieces.push(Piece::Text(&rest[..at]));
            pieces.push(Piece::Text("{{"));
            rest = literal;
            continue;
        }
        let field = after.find("}}").and_then(|end| {
            let inside = &after[..end];
            let (name, default) = match inside.split_once(':') {
                Some((name, default)) => (name, Some(default)),
                None => (inside, None),
            };
            is_name(name).then(|| (end, name.trim(), default))
        });
        if let Some((end, name, default)) = field {
            pieces.push(Piece::Text(&rest[..at]));
            pieces.push(Piece::Field { name, default });
            rest = &after[end + 2..];
        } else {
            // Not a placeholder: the braces are text, and the search goes on after them.
            pieces.push(Piece::Text(&rest[..at + 2]));
            rest = after;
        }
    }
    pieces.push(Piece::Text(rest));
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(name: &str, default: Option<&str>) -> Placeholder {
        Placeholder {
            name: name.into(),
            default: default.map(Into::into),
        }
    }

    #[test]
    fn placeholders_are_found_once_with_their_defaults_r_ssh_49() {
        let text =
            "tail -f /var/log/{{service}}.log | grep {{word:ERROR}} # {{service:web}} {{ word }}";
        assert_eq!(
            placeholders(text),
            [named("service", Some("web")), named("word", Some("ERROR"))]
        );
        assert_eq!(placeholders("echo æøå"), []);
    }

    #[test]
    fn a_snippet_is_filled_with_answers_then_defaults_r_ssh_49() {
        let text = "ssh {{user:root}}@{{host}} -p {{port:22}}";
        let filled = fill(text, |name| (name == "host").then(|| "bøx".to_owned()));
        assert_eq!(filled, "ssh root@bøx -p 22");
        assert_eq!(fill("{{nothing}}!", |_| None), "!");
    }

    #[test]
    fn braces_that_are_no_placeholder_are_sent_as_written_r_ssh_49() {
        let text = "awk '{print $1}' {{ }} {{a/b}} {{{{x}} {{open";
        assert_eq!(placeholders(text), []);
        assert_eq!(
            fill(text, |_| Some("?".into())),
            "awk '{print $1}' {{ }} {{a/b}} {{x}} {{open"
        );
        assert_eq!(fill("{{{{name}}", |_| Some("?".into())), "{{name}}");
        assert_eq!(fill("", |_| None), "");
    }
}
