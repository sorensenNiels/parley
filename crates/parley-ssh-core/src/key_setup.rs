//! Setting up key sign-on from the window (R-SSH-67, R-SSH-68, D63): the commands that add a public key to the
//! server's `authorized_keys` and check the folders IBM i's sshd is strict about, what they answer, and the keys on
//! this computer. Pure functions over text, run by the window on a side channel ([`crate::net::Command::Exec`]);
//! plain Bourne shell, as the transfer commands, so IBM i's PASE runs them too.

mod make;

use std::path::{Path, PathBuf};

use russh::keys::ssh_key::{PrivateKey, PublicKey};

use crate::known_hosts::fingerprint;
use crate::transfer::quote;

pub use make::{MadeKey, make_key};

/// The key files looked for in `~/.ssh`, in the order they are offered.
const USUAL: [&str; 3] = ["id_ed25519", "id_ecdsa", "id_rsa"];

/// What a key setup's commands print first on a line of their own, so the shell's own output is told apart.
const MARK: &str = "parley:";

/// A public key to put on the server: its `authorized_keys` line and its SHA-256 fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicLine {
    /// `type base64 [comment]`, one line.
    pub line: String,
    pub fingerprint: String,
}

impl PublicLine {
    /// `text` as a key line: one OpenSSH public key, its comment kept.
    ///
    /// # Errors
    /// It is not one, said for the user.
    pub fn parse(text: &str) -> Result<Self, String> {
        let line = text.trim();
        if line.contains(['\n', '\r']) {
            return Err("A public key is one line.".to_owned());
        }
        let key = PublicKey::from_openssh(line)
            .map_err(|e| format!("This is not an OpenSSH public key: {e}"))?;
        Ok(Self::of(&key, line))
    }

    fn of(key: &PublicKey, line: &str) -> Self {
        Self {
            line: line.to_owned(),
            fingerprint: fingerprint(key),
        }
    }

    /// The key without its comment (`type base64`): how a key already on the server is recognised.
    fn body(&self) -> String {
        self.line
            .split_whitespace()
            .take(2)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// The public key of the key file at `private`: its `.pub` file beside it, or else the public part an OpenSSH
/// private key holds unencrypted, so no passphrase is asked.
///
/// # Errors
/// Neither can be read, said for the user with the file's name.
pub fn public_line(private: &Path) -> Result<PublicLine, String> {
    let mut beside = private.as_os_str().to_owned();
    beside.push(".pub");
    if let Ok(text) = std::fs::read_to_string(&beside) {
        let first = text.lines().next().unwrap_or_default();
        return PublicLine::parse(first)
            .map_err(|e| format!("{}: {e}", Path::new(&beside).display()));
    }
    let text = std::fs::read_to_string(private)
        .map_err(|e| format!("{} cannot be read: {e}", private.display()))?;
    let key = PrivateKey::from_openssh(&text).map_err(|_| {
        format!(
            "{} is not an OpenSSH private key, and there is no {} beside it.",
            private.display(),
            Path::new(&beside).display()
        )
    })?;
    let public = key.public_key();
    let line = public
        .to_openssh()
        .map_err(|e| format!("{}: {e}", private.display()))?;
    Ok(PublicLine::of(public, &line))
}

/// The usual key files in `ssh_dir` that exist, in the order they are offered (R-SSH-67).
pub fn usual_keys(ssh_dir: &Path) -> Vec<PathBuf> {
    USUAL
        .iter()
        .map(|name| ssh_dir.join(name))
        .filter(|path| path.is_file())
        .collect()
}

/// What adding a key did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Added {
    Added,
    /// The same key (type and data; the comment does not count) was there already.
    AlreadyThere,
}

/// The command that adds `key` to `~/.ssh/authorized_keys`: `~/.ssh` (mode 700) and the file (600) made where
/// missing, the existing ones left as they are; the line added at the end, after a line end if the file lacks its
/// last one, only when the same key is not there. Prints [`MARK`] `added` or `already`.
pub fn add_command(key: &PublicLine) -> String {
    format!(
        "umask 077; d=\"$HOME/.ssh\"; f=\"$d/authorized_keys\"; \
         [ -d \"$d\" ] || mkdir \"$d\" || exit 3; \
         [ -f \"$f\" ] || : > \"$f\" || exit 4; \
         if grep -qF -- {body} \"$f\"; then echo {MARK}already; exit 0; fi; \
         if [ -s \"$f\" ] && [ -n \"$(tail -c 1 \"$f\")\" ]; then echo >> \"$f\" || exit 5; fi; \
         printf '%s\\n' {line} >> \"$f\" || exit 5; echo {MARK}added",
        body = quote(&key.body()),
        line = quote(&key.line),
    )
}

/// What [`add_command`] said: its output and exit status.
///
/// # Errors
/// It failed, in plain words with what the server said.
pub fn read_added(output: &str, status: Option<u32>) -> Result<Added, String> {
    let said = |word: &str| output.lines().any(|l| l.trim() == format!("{MARK}{word}"));
    match status {
        Some(0) if said("added") => Ok(Added::Added),
        Some(0) if said("already") => Ok(Added::AlreadyThere),
        Some(3) => Err(failed("~/.ssh could not be made", output)),
        Some(4) => Err(failed("~/.ssh/authorized_keys could not be made", output)),
        Some(5) => Err(failed(
            "~/.ssh/authorized_keys could not be written",
            output,
        )),
        _ => Err(failed("The key could not be added", output)),
    }
}

fn failed(what: &str, output: &str) -> String {
    let last = output
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty() && !l.starts_with(MARK));
    match last {
        Some(line) => format!("{what}: {line}"),
        None => format!("{what}."),
    }
}

/// The command that says the system's name and which of the home folder and `~/.ssh` the group or others can
/// write (`find -prune -perm`, POSIX, as AIX's and so PASE's): one line [`MARK`] `open g|o|go PATH` each.
pub fn check_command() -> &'static str {
    "uname; for d in \"$HOME\" \"$HOME/.ssh\"; do [ -d \"$d\" ] || continue; \
     g=$(find \"$d\" -prune -perm -020 2>/dev/null); o=$(find \"$d\" -prune -perm -002 2>/dev/null); \
     [ -n \"$g$o\" ] && printf 'parley:open %s%s %s\\n' \"${g:+g}\" \"${o:+o}\" \"$d\"; done; true"
}

/// A folder sshd's `StrictModes` refuses keys for: who besides the user can write it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenFolder {
    pub path: String,
    pub group: bool,
    pub others: bool,
}

impl OpenFolder {
    /// Why sshd refuses keys, in plain words: "/home/NIELS can be written by the group".
    pub fn why(&self) -> String {
        let who = match (self.group, self.others) {
            (true, true) => "the group and others",
            (true, false) => "the group",
            (false, true | false) => "others",
        };
        format!("{} can be written by {who}", self.path)
    }
}

/// What [`check_command`] found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Checked {
    /// The server is IBM i (`uname` says `OS400`).
    pub ibm_i: bool,
    pub open: Vec<OpenFolder>,
}

impl Checked {
    /// The folders that would make IBM i's sshd refuse the key: none on other systems, where Parley SSH leaves the
    /// folders to the server's own rules.
    pub fn to_fix(&self) -> &[OpenFolder] {
        if self.ibm_i { &self.open } else { &[] }
    }
}

/// Reads [`check_command`]'s output.
pub fn read_checked(output: &str) -> Checked {
    let mut checked = Checked {
        ibm_i: output
            .lines()
            .next()
            .is_some_and(|l| l.trim().eq_ignore_ascii_case("OS400")),
        open: Vec::new(),
    };
    for line in output.lines() {
        let Some(rest) = line.strip_prefix("parley:open ") else {
            continue;
        };
        let Some((who, path)) = rest.split_once(' ') else {
            continue;
        };
        checked.open.push(OpenFolder {
            path: path.to_owned(),
            group: who.contains('g'),
            others: who.contains('o'),
        });
    }
    checked
}

/// The command that takes the group's and others' write permission from just `folders` (R-SSH-67's Fix, D63).
pub fn fix_command(folders: &[OpenFolder]) -> String {
    let paths: Vec<String> = folders.iter().map(|f| quote(&f.path)).collect();
    format!("chmod go-w -- {}", paths.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ED25519: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAV5CINKVHRsJdOO4vr5nwVr0ASkuG8DODBufVj2pCQ9";

    fn line(comment: &str) -> PublicLine {
        PublicLine::parse(&format!("{ED25519} {comment}")).expect("a key line")
    }

    #[test]
    fn a_key_line_is_one_openssh_key_with_its_comment_r_ssh_67() {
        let key = line("niels@æøå");
        assert_eq!(key.line, format!("{ED25519} niels@æøå"));
        assert!(key.fingerprint.starts_with("SHA256:"));
        assert_eq!(key.body(), ED25519);
        assert!(PublicLine::parse("ssh-ed25519 nonsense").is_err());
        assert!(PublicLine::parse(&format!("{ED25519}\n{ED25519}")).is_err());
    }

    #[test]
    fn the_answer_says_added_already_or_what_failed_r_ssh_67() {
        assert_eq!(read_added("parley:added\n", Some(0)), Ok(Added::Added));
        assert_eq!(
            read_added("Welcome\nparley:already\n", Some(0)),
            Ok(Added::AlreadyThere)
        );
        assert_eq!(
            read_added("mkdir: /home/æøå/.ssh: Permission denied\n", Some(3)),
            Err("~/.ssh could not be made: mkdir: /home/æøå/.ssh: Permission denied".to_owned())
        );
        assert_eq!(
            read_added("", Some(0)),
            Err("The key could not be added.".to_owned())
        );
        assert!(read_added("parley:added", None).is_err());
    }

    #[test]
    fn ibm_i_s_open_folders_are_read_and_fixed_one_by_one_r_ssh_67() {
        let checked =
            read_checked("OS400\nparley:open g /home/NIELS\nparley:open go /home/NIELS/.ssh\n");
        assert!(checked.ibm_i);
        assert_eq!(checked.to_fix().len(), 2);
        assert_eq!(
            checked.open[0].why(),
            "/home/NIELS can be written by the group"
        );
        assert_eq!(
            checked.open[1].why(),
            "/home/NIELS/.ssh can be written by the group and others"
        );
        assert_eq!(
            fix_command(checked.to_fix()),
            "chmod go-w -- '/home/NIELS' '/home/NIELS/.ssh'"
        );

        let linux = read_checked("Linux\nparley:open o /home/æøå's\n");
        assert!(!linux.ibm_i);
        assert!(linux.to_fix().is_empty(), "left to the server's own rules");
        assert_eq!(linux.open[0].path, "/home/æøå's");
        assert_eq!(fix_command(&linux.open), "chmod go-w -- '/home/æøå'\\''s'");
    }

    #[cfg(unix)]
    mod with_a_shell {
        use super::*;
        use std::os::unix::fs::PermissionsExt;
        use std::process::Command;

        /// A fresh home folder, removed when dropped.
        struct Home(PathBuf);

        impl Home {
            fn new(name: &str) -> Self {
                let dir = std::env::temp_dir()
                    .join(format!("parley-key-setup-{name}-{}", std::process::id()));
                // Left over from an earlier run that panicked: harmless if it is not there.
                let _ = std::fs::remove_dir_all(&dir);
                std::fs::create_dir_all(&dir).expect("a home folder");
                Self(dir)
            }

            fn run(&self, script: &str) -> (String, Option<u32>) {
                let out = Command::new("sh")
                    .args(["-c", script])
                    .env("HOME", &self.0)
                    .output()
                    .expect("sh runs");
                let status = out.status.code().and_then(|c| u32::try_from(c).ok());
                (String::from_utf8_lossy(&out.stdout).into_owned(), status)
            }

            fn mode(&self, path: &str) -> u32 {
                std::fs::metadata(self.0.join(path))
                    .expect("there")
                    .permissions()
                    .mode()
                    & 0o777
            }
        }

        impl Drop for Home {
            fn drop(&mut self) {
                // Only the test's own folder; one left behind is harmless.
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        #[test]
        fn a_key_is_added_once_with_the_folder_and_file_made_private_r_ssh_67() {
            let home = Home::new("add");
            let (out, status) = home.run(&add_command(&line("niels@æøå")));
            assert_eq!(read_added(&out, status), Ok(Added::Added));
            assert_eq!(home.mode(".ssh"), 0o700);
            assert_eq!(home.mode(".ssh/authorized_keys"), 0o600);

            // The same key with another comment is there already.
            let (out, status) = home.run(&add_command(&line("other")));
            assert_eq!(read_added(&out, status), Ok(Added::AlreadyThere));
            let file = std::fs::read_to_string(home.0.join(".ssh/authorized_keys")).expect("read");
            assert_eq!(file, format!("{ED25519} niels@æøå\n"));
        }

        #[test]
        fn a_file_without_its_last_line_end_gets_one_before_the_key_r_ssh_67() {
            let home = Home::new("line-end");
            std::fs::create_dir(home.0.join(".ssh")).expect("made");
            std::fs::write(home.0.join(".ssh/authorized_keys"), "ssh-rsa AAAA old")
                .expect("written");
            let (out, status) = home.run(&add_command(&line("new")));
            assert_eq!(read_added(&out, status), Ok(Added::Added));
            let file = std::fs::read_to_string(home.0.join(".ssh/authorized_keys")).expect("read");
            assert_eq!(file, format!("ssh-rsa AAAA old\n{ED25519} new\n"));
            assert_eq!(
                home.mode(".ssh"),
                0o777 & !umask(),
                "an existing folder is left as it is"
            );
        }

        #[test]
        fn the_check_finds_folders_others_can_write_and_the_fix_closes_them_r_ssh_67() {
            let home = Home::new("check");
            std::fs::create_dir(home.0.join(".ssh")).expect("made");
            std::fs::set_permissions(&home.0, std::fs::Permissions::from_mode(0o775)).expect("set");
            std::fs::set_permissions(home.0.join(".ssh"), std::fs::Permissions::from_mode(0o702))
                .expect("set");
            let (out, _) = home.run(check_command());
            let checked = read_checked(&out);
            let home_path = home.0.display().to_string();
            assert_eq!(
                checked.open,
                [
                    OpenFolder {
                        path: home_path.clone(),
                        group: true,
                        others: false
                    },
                    OpenFolder {
                        path: format!("{home_path}/.ssh"),
                        group: false,
                        others: true
                    },
                ]
            );
            let (_, status) = home.run(&fix_command(&checked.open));
            assert_eq!(status, Some(0));
            assert_eq!(read_checked(&home.run(check_command()).0).open, []);
            assert_eq!(home.mode(".ssh"), 0o700);
        }

        /// The process's umask, as `sh` reports it.
        fn umask() -> u32 {
            let out = Command::new("sh")
                .args(["-c", "umask"])
                .output()
                .expect("sh runs");
            u32::from_str_radix(String::from_utf8_lossy(&out.stdout).trim(), 8).expect("octal")
        }
    }
}
