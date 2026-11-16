//! OpenSSH's client config, read-only (R-SSH-42; `ssh_config(5)`): the hosts of `~/.ssh/config` and its
//! `Include`s, and the settings OpenSSH would use for one of them. Only the keywords Parley SSH uses are read;
//! others are left alone, as they are OpenSSH's. `Match` blocks are skipped.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// How deep `Include`s may nest, as OpenSSH's `READCONF_MAX_DEPTH`.
const MAX_DEPTH: usize = 16;

/// A keyword's value, as read (`ssh_config(5)`).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Setting {
    HostName(String),
    User(String),
    Port(u16),
    IdentityFile(String),
    ForwardAgent(bool),
    ProxyJump(String),
}

/// When a line applies: each list of `Host` patterns matches (an `Include` inside a `Host` block adds one), and
/// never inside a `Match` block.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Condition {
    hosts: Vec<Vec<String>>,
    never: bool,
}

impl Condition {
    fn applies_to(&self, alias: &str) -> bool {
        !self.never && self.hosts.iter().all(|patterns| matches(patterns, alias))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    condition: Condition,
    setting: Setting,
}

/// What OpenSSH would use for one host (R-SSH-42).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Host {
    /// The name on the `Host` line.
    pub alias: String,
    /// Where to connect: `HostName`, else the alias.
    pub host_name: String,
    pub user: Option<String>,
    pub port: Option<u16>,
    /// Every `IdentityFile` that applies, in order, with `~` and `%` tokens expanded.
    pub identity_files: Vec<PathBuf>,
    pub forward_agent: Option<bool>,
    /// The jump hosts (R-SSH-43); `None` also for `ProxyJump none`.
    pub proxy_jump: Option<String>,
}

/// OpenSSH's config as read, with the files it came from to notice changes.
#[derive(Debug, Clone, Default)]
pub struct Config {
    home: PathBuf,
    top: PathBuf,
    entries: Vec<Entry>,
    aliases: Vec<String>,
    problems: Vec<String>,
    /// Each file and folder read, with its time of change then; `None` when it was not there.
    seen: Vec<(PathBuf, Option<SystemTime>)>,
}

impl Config {
    /// The user's `~/.ssh/config`; `None` without a home folder.
    pub fn from_home() -> Option<Self> {
        let home = std::env::home_dir()?;
        let top = home.join(".ssh").join("config");
        Some(Self::read(&top, &home))
    }

    /// Reads `top` and its `Include`s; `home` is where `~` and relative includes (in `home/.ssh`) point.
    pub fn read(top: &Path, home: &Path) -> Self {
        let mut config = Self {
            home: home.to_owned(),
            top: top.to_owned(),
            ..Self::default()
        };
        config.read_file(top, &Condition::default(), 0);
        config
    }

    /// The names on `Host` lines that are not patterns, each once, in file order.
    pub fn hosts(&self) -> &[String] {
        &self.aliases
    }

    /// The lines that were skipped, with their file and line number.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    /// A file read, or a folder an `Include` looked in, changed, came or went since it was read.
    pub fn changed(&self) -> bool {
        self.seen.iter().any(|(path, when)| modified(path) != *when)
    }

    /// The config read again, as [`Config::read`].
    #[must_use]
    pub fn reread(&self) -> Self {
        Self::read(&self.top, &self.home)
    }

    /// What OpenSSH would use for `alias`: for each keyword the first value that applies; `IdentityFile`s add
    /// up. `local_user` stands for `%u` and for `%r` when no `User` is set.
    pub fn host(&self, alias: &str, local_user: &str) -> Host {
        let mut host = Host {
            alias: alias.to_owned(),
            ..Host::default()
        };
        let mut host_name = None;
        let mut identity_files = Vec::new();
        let mut proxy_jump = None;
        let applying = self
            .entries
            .iter()
            .filter(|e| e.condition.applies_to(alias));
        for entry in applying {
            match &entry.setting {
                Setting::HostName(v) => host_name = host_name.or_else(|| Some(v.clone())),
                Setting::User(v) => host.user = host.user.take().or_else(|| Some(v.clone())),
                Setting::Port(p) => host.port = host.port.or(Some(*p)),
                Setting::IdentityFile(v) => identity_files.push(v.clone()),
                Setting::ForwardAgent(on) => host.forward_agent = host.forward_agent.or(Some(*on)),
                Setting::ProxyJump(v) => proxy_jump = proxy_jump.or_else(|| Some(v.clone())),
            }
        }
        host.host_name =
            host_name.map_or_else(|| alias.to_owned(), |name| expand(&name, &[('h', alias)]));
        host.proxy_jump = proxy_jump.filter(|j| !j.eq_ignore_ascii_case("none"));
        let home = self.home.to_string_lossy();
        let remote_user = host.user.as_deref().unwrap_or(local_user);
        let tokens = [
            ('d', home.as_ref()),
            ('u', local_user),
            ('h', host.host_name.as_str()),
            ('r', remote_user),
        ];
        host.identity_files = identity_files
            .iter()
            .map(|file| self.tilde(&expand(file, &tokens)))
            .collect();
        host
    }

    fn read_file(&mut self, path: &Path, outer: &Condition, depth: usize) {
        self.seen.push((path.to_owned(), modified(path)));
        // A file that is not there is no config, as for OpenSSH.
        let Ok(text) = fs::read_to_string(path) else {
            return;
        };
        let mut current = outer.clone();
        for (number, line) in text.lines().enumerate() {
            let place = format!("{}:{}", self.shown(path), number + 1);
            match split_line(line) {
                Ok(Some((keyword, args))) => {
                    self.line(&place, &keyword, &args, outer, &mut current, depth);
                }
                Ok(None) => {}
                Err(e) => self.problems.push(format!("{place}: {e}")),
            }
        }
    }

    /// One line's keyword and arguments; `current` is the condition of the block it is in.
    fn line(
        &mut self,
        place: &str,
        keyword: &str,
        args: &[String],
        outer: &Condition,
        current: &mut Condition,
        depth: usize,
    ) {
        let first = args.first().cloned();
        let setting = match keyword.to_ascii_lowercase().as_str() {
            "host" => {
                if args.is_empty() {
                    self.problems.push(format!("{place}: Host names no host."));
                }
                for name in args {
                    let pattern = name.contains(['*', '?', '!']);
                    if !pattern && !self.aliases.iter().any(|a| a.eq_ignore_ascii_case(name)) {
                        self.aliases.push(name.clone());
                    }
                }
                *current = outer.clone();
                current
                    .hosts
                    .push(args.iter().map(|a| a.to_lowercase()).collect());
                return;
            }
            "match" => {
                self.problems
                    .push(format!("{place}: Match blocks are not used; skipped."));
                *current = Condition {
                    never: true,
                    ..outer.clone()
                };
                return;
            }
            "include" => {
                self.include(place, args, current, depth);
                return;
            }
            "hostname" => first.map(Setting::HostName),
            "user" => first.map(Setting::User),
            "identityfile" => first.map(Setting::IdentityFile),
            "proxyjump" => first.map(Setting::ProxyJump),
            "port" => match first.as_deref().map(str::parse::<u16>) {
                Some(Ok(port)) if port > 0 => Some(Setting::Port(port)),
                _ => {
                    self.problems
                        .push(format!("{place}: Port is a number from 1 to 65535."));
                    return;
                }
            },
            "forwardagent" => match first.as_deref().map(str::to_ascii_lowercase).as_deref() {
                Some("yes" | "true") => Some(Setting::ForwardAgent(true)),
                Some("no" | "false") => Some(Setting::ForwardAgent(false)),
                _ => {
                    self.problems
                        .push(format!("{place}: only ForwardAgent yes or no is used."));
                    return;
                }
            },
            // OpenSSH's other keywords are OpenSSH's business.
            _ => return,
        };
        match setting {
            Some(setting) => self.entries.push(Entry {
                condition: current.clone(),
                setting,
            }),
            None => self
                .problems
                .push(format!("{place}: {keyword} has no value.")),
        }
    }

    /// `Include`: each file, in `~/.ssh` when relative, with `*` and `?` in the file name sorted as glob(3).
    fn include(&mut self, place: &str, args: &[String], current: &Condition, depth: usize) {
        if depth + 1 > MAX_DEPTH {
            self.problems
                .push(format!("{place}: Include nests deeper than {MAX_DEPTH}."));
            return;
        }
        for arg in args {
            let path = self.tilde(arg);
            let path = if path.is_absolute() {
                path
            } else {
                self.home.join(".ssh").join(path)
            };
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
            match (name, path.parent()) {
                (Some(name), Some(folder)) if name.contains(['*', '?']) => {
                    let folder = folder.to_owned();
                    self.seen.push((folder.clone(), modified(&folder)));
                    let mut found: Vec<PathBuf> = fs::read_dir(&folder)
                        .into_iter()
                        .flatten()
                        .flatten()
                        .filter(|e| wildcard(&name, &e.file_name().to_string_lossy()))
                        .map(|e| e.path())
                        .filter(|p| p.is_file())
                        .collect();
                    found.sort();
                    for file in found {
                        self.read_file(&file, current, depth + 1);
                    }
                }
                _ => self.read_file(&path, current, depth + 1),
            }
        }
    }

    /// A file as it is said: under the home folder from `~`.
    fn shown(&self, path: &Path) -> String {
        match path.strip_prefix(&self.home) {
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => path.display().to_string(),
        }
    }

    /// `~` and `~/…` as the home folder.
    fn tilde(&self, path: &str) -> PathBuf {
        match path.strip_prefix('~') {
            Some("") => self.home.clone(),
            Some(rest) if rest.starts_with(['/', '\\']) => self.home.join(&rest[1..]),
            _ => PathBuf::from(path),
        }
    }
}

/// The local user's name, for `%u` and a host with no `User`.
pub fn local_user() -> String {
    ["USER", "USERNAME", "LOGNAME"]
        .iter()
        .find_map(|v| std::env::var(v).ok().filter(|u| !u.is_empty()))
        .unwrap_or_default()
}

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// A line's keyword and arguments: `Keyword value`, `Keyword=value` or `Keyword = value`; arguments split at
/// spaces, kept whole in double quotes. `None` for an empty line or a comment.
fn split_line(line: &str) -> Result<Option<(String, Vec<String>)>, &'static str> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    let end = line
        .find(|c: char| c.is_whitespace() || c == '=')
        .unwrap_or(line.len());
    let (keyword, rest) = line.split_at(end);
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=').unwrap_or(rest);
    let mut args = Vec::new();
    let mut chars = rest.chars().peekable();
    loop {
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        let Some(&c) = chars.peek() else { break };
        if c == '#' {
            break;
        }
        let mut arg = String::new();
        if c == '"' {
            chars.next();
            loop {
                match chars.next() {
                    Some('"') => break,
                    Some(c) => arg.push(c),
                    None => return Err("a quote is not closed."),
                }
            }
        } else {
            while let Some(c) = chars.next_if(|c| !c.is_whitespace()) {
                arg.push(c);
            }
        }
        args.push(arg);
    }
    Ok(Some((keyword.to_owned(), args)))
}

/// A host matches a `Host` line when one of its patterns does and none of its `!` patterns (`ssh_config(5)`
/// PATTERNS); ignoring case.
fn matches(patterns: &[String], alias: &str) -> bool {
    let alias = alias.to_lowercase();
    let mut any = false;
    for pattern in patterns {
        if let Some(negated) = pattern.strip_prefix('!') {
            if wildcard(negated, &alias) {
                return false;
            }
        } else if wildcard(pattern, &alias) {
            any = true;
        }
    }
    any
}

/// `*` for any characters, `?` for one.
fn wildcard(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    // Positions to go back to when a `*` must take one more character.
    let (mut p, mut t) = (0, 0);
    let mut star = None;
    while t < text.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, t));
                p += 1;
            }
            Some(&c) if c == '?' || c == text[t] => {
                p += 1;
                t += 1;
            }
            _ => match star {
                Some((sp, st)) => {
                    p = sp + 1;
                    t = st + 1;
                    star = Some((sp, st + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

/// `%x` tokens replaced by their values; `%%` is `%`; an unknown token is kept.
fn expand(text: &str, tokens: &[(char, &str)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let token = chars.next();
        let value = token.and_then(|t| tokens.iter().find(|(k, _)| *k == t));
        match (token, value) {
            (_, Some((_, value))) => out.push_str(value),
            (Some(t), None) if t != '%' => {
                out.push('%');
                out.push(t);
            }
            // `%%`, or a `%` at the end.
            (_, None) => out.push('%'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A home folder of its own with `~/.ssh/config` holding `text`.
    struct Home(PathBuf);

    impl Home {
        fn new(name: &str, text: &str) -> Self {
            let home = std::env::temp_dir()
                .join(format!("parley-ssh-config-{name}-{}", std::process::id()));
            // A folder left by an earlier run: start clean.
            let _ = fs::remove_dir_all(&home);
            fs::create_dir_all(home.join(".ssh")).expect("the temp folder is writable");
            let home = Self(home);
            home.write("config", text);
            home
        }

        fn write(&self, name: &str, text: &str) {
            let path = self.0.join(".ssh").join(name);
            if let Some(folder) = path.parent() {
                fs::create_dir_all(folder).expect("the temp folder is writable");
            }
            fs::write(path, text).expect("the temp folder is writable");
        }

        fn config(&self) -> Config {
            Config::read(&self.0.join(".ssh").join("config"), &self.0)
        }
    }

    impl Drop for Home {
        fn drop(&mut self) {
            // Only a temp folder of the test's own.
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn hosts_are_the_names_that_are_not_patterns_r_ssh_42() {
        let home = Home::new(
            "names",
            "Host web db\n  User a\nHost *.lan !x.lan web\nhost=Æøå\nHost WEB\n",
        );
        assert_eq!(home.config().hosts(), ["web", "db", "Æøå"]);
    }

    #[test]
    fn the_first_value_wins_and_identity_files_add_up_r_ssh_42() {
        let home = Home::new(
            "first",
            "Host web\n  HostName web1.example.com\n  IdentityFile ~/.ssh/web\n\
             Host *\n  User me\n  Port 2222\n  HostName other\n  IdentityFile %d/.ssh/id_%r@%h\n",
        );
        let host = home.config().host("web", "local");
        assert_eq!(host.host_name, "web1.example.com");
        assert_eq!(host.user.as_deref(), Some("me"));
        assert_eq!(host.port, Some(2222));
        assert_eq!(
            host.identity_files,
            [
                home.0.join(".ssh/web"),
                PathBuf::from(format!("{}/.ssh/id_me@web1.example.com", home.0.display()))
            ]
        );
    }

    #[test]
    fn a_host_name_may_name_the_alias_and_defaults_to_it() {
        let home = Home::new("token", "Host box\n  HostName %h.lan\nHost plain\n");
        let config = home.config();
        assert_eq!(config.host("box", "u").host_name, "box.lan");
        assert_eq!(config.host("plain", "u").host_name, "plain");
        assert_eq!(config.host("plain", "u").user, None);
    }

    #[test]
    fn negated_patterns_keep_a_block_from_a_host() {
        let home = Home::new("negated", "Host *.lan !secret.lan\n  User lan\n");
        let config = home.config();
        assert_eq!(config.host("box.LAN", "u").user.as_deref(), Some("lan"));
        assert_eq!(config.host("secret.lan", "u").user, None);
    }

    #[test]
    fn includes_are_read_where_they_stand_and_inside_a_host_block_apply_to_it() {
        let home = Home::new(
            "include",
            "Include conf.d/*.conf\nHost web\n  Include web.extra\nHost *\n  User fallback\n",
        );
        home.write("conf.d/b.conf", "Host b\n  User bee\n");
        home.write("conf.d/a.conf", "Host a\n  User ay\n");
        home.write("conf.d/skip.txt", "Host skip\n");
        home.write("web.extra", "Port 2200\n");
        let config = home.config();
        assert_eq!(config.hosts(), ["a", "b", "web"]);
        assert_eq!(config.host("a", "u").user.as_deref(), Some("ay"));
        assert_eq!(config.host("web", "u").port, Some(2200));
        assert_eq!(config.host("a", "u").port, None);
        assert_eq!(config.host("web", "u").user.as_deref(), Some("fallback"));
    }

    #[test]
    fn an_include_that_includes_itself_stops_at_sixteen_levels() {
        let home = Home::new("loop", "Include config\nHost x\n");
        let config = home.config();
        assert_eq!(config.hosts(), ["x"]);
        assert!(config.problems().iter().any(|p| p.contains("deeper")));
    }

    #[test]
    fn match_blocks_and_bad_lines_are_skipped_and_said() {
        let home = Home::new(
            "bad",
            "Match host x\n  User matched\nHost x\n  Port none\n  IdentityFile \"no end\n  \
             ForwardAgent /tmp/agent\n  User\n  Port 22 # a comment\n",
        );
        let config = home.config();
        let x = config.host("x", "u");
        assert_eq!(x.user, None);
        assert_eq!(x.port, Some(22));
        assert_eq!(x.forward_agent, None);
        assert_eq!(config.problems().len(), 5, "{:?}", config.problems());
        assert_eq!(
            config.problems()[0],
            "~/.ssh/config:1: Match blocks are not used; skipped."
        );
    }

    #[test]
    fn quoted_values_and_equals_signs_are_read() {
        assert_eq!(
            split_line("IdentityFile=\"C:\\My Keys\\id\" more"),
            Ok(Some((
                "IdentityFile".to_owned(),
                vec!["C:\\My Keys\\id".to_owned(), "more".to_owned()]
            )))
        );
        assert_eq!(
            split_line("  Port = 22"),
            Ok(Some(("Port".to_owned(), vec!["22".to_owned()])))
        );
        assert_eq!(split_line("   # comment"), Ok(None));
        assert_eq!(split_line(""), Ok(None));
    }

    #[test]
    fn forward_agent_and_proxy_jump_none_are_read() {
        let home = Home::new(
            "agent",
            "Host a\n  ForwardAgent yes\n  ProxyJump bastion\nHost b\n  ProxyJump none\nHost *\n  ProxyJump x\n",
        );
        let config = home.config();
        assert_eq!(config.host("a", "u").forward_agent, Some(true));
        assert_eq!(config.host("a", "u").proxy_jump.as_deref(), Some("bastion"));
        assert_eq!(config.host("b", "u").proxy_jump, None);
    }

    #[test]
    fn a_missing_config_is_empty_and_noticed_when_it_comes() {
        let home = Home::new("missing", "");
        fs::remove_file(home.0.join(".ssh/config")).expect("the file was written");
        let config = home.config();
        assert_eq!(config.hosts(), [] as [String; 0]);
        assert_eq!(config.problems(), [] as [String; 0]);
        assert!(!config.changed());
        home.write("config", "Host new\n");
        assert!(config.changed());
        assert_eq!(config.reread().hosts(), ["new"]);
    }

    #[test]
    fn wildcards_match_as_ssh_does() {
        assert!(wildcard("*", ""));
        assert!(wildcard("*.lan", "a.b.lan"));
        assert!(wildcard("web?", "web1"));
        assert!(!wildcard("web?", "web"));
        assert!(wildcard("a*b*c", "aXXbYYc"));
        assert!(!wildcard("a*b", "aXXbYYc"));
        assert!(wildcard("æ*", "æøå"));
    }

    #[test]
    fn tokens_expand_and_unknown_ones_stay() {
        assert_eq!(expand("%h-%%-%z-%", &[('h', "x")]), "x-%-%z-%");
    }
}
