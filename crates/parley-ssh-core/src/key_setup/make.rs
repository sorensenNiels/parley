//! Making a key when there is none (R-SSH-68): Ed25519 only, encrypted with the passphrase the user chose (or
//! none), in `~/.ssh` beside the user's other keys and never over an existing file. Other key types, changing a
//! passphrase and converting formats are left to `ssh-keygen`.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use russh::keys::ssh_key::{Algorithm, LineEnding, PrivateKey};
use ssh_key::getrandom::SysRng;
use ssh_key::rand_core::UnwrapErr;

use super::PublicLine;
use crate::Secret;

/// The file name a made key gets, before a number is added to it.
const NAME: &str = "id_ed25519";

/// How many numbered names are tried before giving up.
const NAMES: usize = 100;

/// A key made by [`make_key`]: where its private key went, and its public key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MadeKey {
    pub private: PathBuf,
    pub public: PublicLine,
}

/// Makes an Ed25519 key with `comment` (`user@computer`) in `ssh_dir` (made, readable by the user only, when
/// missing): `id_ed25519` and `id_ed25519.pub`, or `id_ed25519_2` and so on when those exist; the private key
/// encrypted with `passphrase` unless it is `None` or empty, and readable by the user only.
///
/// # Errors
/// The key could not be made or written, said for the user.
pub fn make_key(
    ssh_dir: &Path,
    comment: &str,
    passphrase: Option<&Secret>,
) -> Result<MadeKey, String> {
    make_folder(ssh_dir)?;
    let private = free_name(ssh_dir)?;
    let mut key = PrivateKey::random(&mut UnwrapErr(SysRng), Algorithm::Ed25519)
        .map_err(|e| format!("The key could not be made: {e}"))?;
    key.set_comment(comment);
    let public = key
        .public_key()
        .to_openssh()
        .map_err(|e| format!("The key could not be made: {e}"))?;
    let key = match passphrase.map(Secret::expose).filter(|p| !p.is_empty()) {
        Some(passphrase) => key
            .encrypt(&mut UnwrapErr(SysRng), passphrase)
            .map_err(|e| format!("The key could not be encrypted: {e}"))?,
        None => key,
    };
    let text = key
        .to_openssh(LineEnding::LF)
        .map_err(|e| format!("The key could not be written: {e}"))?;
    write_new(&private, text.as_bytes(), 0o600)?;
    write_new(&with_pub(&private), format!("{public}\n").as_bytes(), 0o644)?;
    Ok(MadeKey {
        private,
        public: PublicLine::parse(&public)?,
    })
}

fn make_folder(ssh_dir: &Path) -> Result<(), String> {
    if ssh_dir.is_dir() {
        return Ok(());
    }
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder
        .create(ssh_dir)
        .map_err(|e| format!("{} could not be made: {e}", ssh_dir.display()))
}

/// The first of `id_ed25519`, `id_ed25519_2`, … whose private and public files are both free.
fn free_name(ssh_dir: &Path) -> Result<PathBuf, String> {
    (1..=NAMES)
        .map(|n| match n {
            1 => ssh_dir.join(NAME),
            n => ssh_dir.join(format!("{NAME}_{n}")),
        })
        .find(|path| !path.exists() && !with_pub(path).exists())
        .ok_or_else(|| format!("{} has no free name for a new key.", ssh_dir.display()))
}

fn with_pub(private: &Path) -> PathBuf {
    let mut name = private.as_os_str().to_owned();
    name.push(".pub");
    PathBuf::from(name)
}

/// Writes `bytes` to a file that must not exist yet (never through an existing file or link), with `mode` on Unix.
fn write_new(path: &Path, bytes: &[u8], mode: u32) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, mode);
    #[cfg(not(unix))]
    let _ = mode;
    options
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|e| format!("{} could not be written: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh folder for `~/.ssh`, removed when dropped.
    struct Folder(PathBuf);

    impl Folder {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("parley-make-key-{name}-{}", std::process::id()));
            // Left over from an earlier run that panicked: harmless if it is not there.
            let _ = std::fs::remove_dir_all(&dir);
            Self(dir)
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            // Only the test's own folder; one left behind is harmless.
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_made_key_reads_back_with_its_passphrase_and_is_private_r_ssh_68() {
        let folder = Folder::new("passphrase");
        let ssh = folder.0.join(".ssh");
        let made = make_key(
            &ssh,
            "niels@æøå",
            Some(&Secret::new("hemmeligt æøå".into())),
        )
        .expect("made");
        assert_eq!(made.private, ssh.join("id_ed25519"));
        assert!(made.public.line.starts_with("ssh-ed25519 "));
        assert!(made.public.line.ends_with(" niels@æøå"));

        let text = std::fs::read_to_string(&made.private).expect("read");
        let key = PrivateKey::from_openssh(&text).expect("an OpenSSH key");
        assert!(key.is_encrypted());
        let open = key
            .decrypt("hemmeligt æøå")
            .expect("the passphrase opens it");
        assert_eq!(
            open.public_key().to_openssh().expect("a line"),
            made.public.line
        );
        assert!(key.decrypt("wrong").is_err());
        assert_eq!(
            super::super::public_line(&made.private),
            Ok(made.public.clone())
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode_of =
                |p: &Path| std::fs::metadata(p).expect("there").permissions().mode() & 0o777;
            assert_eq!(mode_of(&made.private), 0o600);
            assert_eq!(mode_of(&ssh), 0o700);
        }
    }

    #[test]
    fn an_existing_key_is_never_overwritten_and_none_means_no_passphrase_r_ssh_68() {
        let folder = Folder::new("names");
        let ssh = folder.0.join(".ssh");
        std::fs::create_dir_all(&ssh).expect("made");
        std::fs::write(ssh.join("id_ed25519.pub"), "mine").expect("written");
        let made = make_key(&ssh, "a@b", None).expect("made");
        assert_eq!(made.private, ssh.join("id_ed25519_2"));
        assert_eq!(
            std::fs::read_to_string(ssh.join("id_ed25519.pub")).expect("read"),
            "mine"
        );
        let text = std::fs::read_to_string(&made.private).expect("read");
        assert!(
            !PrivateKey::from_openssh(&text)
                .expect("a key")
                .is_encrypted()
        );
        let empty = make_key(&ssh, "a@b", Some(&Secret::new(String::new()))).expect("made");
        assert_eq!(empty.private, ssh.join("id_ed25519_3"));
    }
}
