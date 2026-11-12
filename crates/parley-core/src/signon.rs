//! Automatic sign-on (R-NET-4, RFC 4777 section 5): the password substitute Parley sends in
//! NEW-ENVIRON instead of the password, so the host can bypass its sign-on screen. The host
//! sends a random seed, Parley another; the substitute is built from both, the user and the
//! password, and is no use for another session.
//!
//! Which algorithm the host checks depends on its password level (system value `QPWDLVL`):
//! DES for levels 0 and 1, SHA-1 for 2 and 3. The host does not say, so it is a setting.

use des::Des;
use des::cipher::{BlockEncrypt, KeyInit};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::CodePage;

/// The password substitute's algorithm: the host's password level (`QPWDLVL`) decides it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PasswordLevel {
    /// `QPWDLVL` 0 or 1: passwords of up to 10 characters, not case-sensitive.
    #[default]
    Des,
    /// `QPWDLVL` 2 or 3: passphrases of up to 128 characters, case-sensitive.
    Sha1,
}

impl PasswordLevel {
    pub const ALL: [Self; 2] = [Self::Des, Self::Sha1];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Des => "DES (QPWDLVL 0 or 1)",
            Self::Sha1 => "SHA-1 (QPWDLVL 2 or 3)",
        }
    }
}

/// How a session signs on by itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignOn {
    /// The operator signs on.
    #[default]
    Off,
    /// Parley types the user and password into the sign-on screen and presses Enter.
    Screen,
    /// The host skips its sign-on screen: the user and a password substitute go in NEW-ENVIRON
    /// (RFC 4777). The host's `QRMTSIGN` must be `*VERIFY` or `*SAMEPRF`.
    Bypass,
}

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

/// What telnet needs to answer the host's seed with a password substitute.
#[derive(Debug, Clone)]
pub struct Credentials {
    pub user: String,
    pub password: Password,
    pub level: PasswordLevel,
    pub code_page: CodePage,
    /// Parley's random seed (RFC 4777 `IBMRSEED`); fixed in tests.
    pub client_seed: [u8; 8],
}

impl Credentials {
    /// The NEW-ENVIRON user name: English capitals (RFC 4777 section 4).
    pub fn user_name(&self) -> String {
        self.user.trim().to_ascii_uppercase()
    }

    /// The substitute for the host's seed, `None` if the password cannot be sent this way.
    pub fn substitute(&self, server_seed: [u8; 8]) -> Option<Vec<u8>> {
        password_substitute(
            self.level,
            &self.user_name(),
            self.password.expose(),
            server_seed,
            self.client_seed,
            self.code_page,
        )
    }
}

/// A random client seed, from the operating system.
pub fn random_seed() -> [u8; 8] {
    let mut seed = [0; 8];
    if getrandom::fill(&mut seed).is_err() {
        // No system randomness: the time still differs between connections. The seed only has
        // to make each substitute unlike the last; the host's seed does the same.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        seed = nanos.to_le_bytes()[..8].try_into().unwrap_or_default();
    }
    seed
}

/// The sequence number: telnet sends one substitute per connection (RFC 4777 section 5.1 step 5).
const SEQUENCE: u64 = 1;
const BLANK: u8 = 0x40;

/// The substitute for `password`, or `None` if the user or password cannot be sent with this
/// algorithm (empty, too long, or a character the code page lacks).
pub fn password_substitute(
    level: PasswordLevel,
    user: &str,
    password: &str,
    server_seed: [u8; 8],
    client_seed: [u8; 8],
    code_page: CodePage,
) -> Option<Vec<u8>> {
    match level {
        PasswordLevel::Des => {
            des_substitute(user, password, server_seed, client_seed, code_page).map(Vec::from)
        }
        PasswordLevel::Sha1 => {
            sha1_substitute(user, password, server_seed, client_seed).map(Vec::from)
        }
    }
}

/// Text as EBCDIC capitals, `None` if a character is missing from the code page.
fn ebcdic_upper(text: &str, code_page: CodePage) -> Option<Vec<u8>> {
    text.chars()
        .map(|c| code_page.to_byte(c.to_ascii_uppercase()))
        .collect()
}

/// `bytes` left-justified in `N` bytes, padded with EBCDIC blanks.
fn padded<const N: usize>(bytes: &[u8]) -> [u8; N] {
    let mut out = [BLANK; N];
    for (o, b) in out.iter_mut().zip(bytes) {
        *o = *b;
    }
    out
}

fn des_encrypt(key: [u8; 8], block: [u8; 8]) -> [u8; 8] {
    let cipher = Des::new(&key.into());
    let mut block = block.into();
    cipher.encrypt_block(&mut block);
    block.into()
}

fn xor(a: [u8; 8], b: [u8; 8]) -> [u8; 8] {
    std::array::from_fn(|i| a[i] ^ b[i])
}

/// RFC 4777 section 5.1, steps 1–4: the password token, the user ID encrypted with a key made
/// from up to 8 password characters.
fn des_token(password: &[u8], user: [u8; 8]) -> [u8; 8] {
    let key = u64::from_be_bytes(padded::<8>(password).map(|b| b ^ 0x55)) << 1;
    des_encrypt(key.to_be_bytes(), user)
}

/// RFC 4777 section 5.1 step 4: a 9- or 10-character user ID folds its last two characters
/// into the first eight, two bits to a byte.
fn fold_user(user: &[u8]) -> [u8; 8] {
    let mut out = padded::<8>(user);
    if user.len() > 8 {
        let tail = padded::<10>(user);
        for (i, o) in out.iter_mut().enumerate() {
            let source = tail[8 + i / 4];
            let shift = 6 - 2 * (i % 4);
            *o ^= ((source >> shift) & 0x03) << 6;
        }
    }
    out
}

/// RFC 4777 section 5.1: the 8-byte DES substitute.
fn des_substitute(
    user: &str,
    password: &str,
    server_seed: [u8; 8],
    client_seed: [u8; 8],
    code_page: CodePage,
) -> Option<[u8; 8]> {
    let user = ebcdic_upper(user, code_page)?;
    let password = ebcdic_upper(password, code_page)?;
    if !(1..=10).contains(&user.len()) || !(1..=10).contains(&password.len()) {
        return None;
    }
    let folded = fold_user(&user);
    let (first, rest) = password.split_at(password.len().min(8));
    let mut token = des_token(first, folded);
    if !rest.is_empty() {
        token = xor(token, des_token(rest, folded));
    }

    // Step 6 and 7: the seeds and the user ID, chained through DES (CBC) with the token as key.
    let sequence = SEQUENCE.to_be_bytes();
    let rdr_seq = u64::from_be_bytes(server_seed)
        .wrapping_add(SEQUENCE)
        .to_be_bytes();
    let user16 = padded::<16>(&user);
    let (low, high) = user16.split_at(8);
    let blocks = [
        rdr_seq,
        client_seed,
        xor(padded::<8>(low), rdr_seq),
        xor(padded::<8>(high), rdr_seq),
        sequence,
    ];
    let mut chain = [0; 8];
    for block in blocks {
        chain = des_encrypt(token, xor(chain, block));
    }
    Some(chain)
}

/// Text in UTF-16, big-endian as on IBM i.
fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

/// RFC 4777 section 5.2: the 20-byte SHA-1 substitute.
fn sha1_substitute(
    user: &str,
    password: &str,
    server_seed: [u8; 8],
    client_seed: [u8; 8],
) -> Option<[u8; 20]> {
    let password_chars = password.chars().count();
    if !(1..=10).contains(&user.chars().count()) || !(1..=128).contains(&password_chars) {
        return None;
    }
    let user = utf16(&format!("{:<10}", user.to_uppercase()));
    let token = Sha1::new()
        .chain_update(&user)
        .chain_update(utf16(password))
        .finalize();
    let substitute = Sha1::new()
        .chain_update(token)
        .chain_update(server_seed)
        .chain_update(client_seed)
        .chain_update(&user)
        .chain_update(SEQUENCE.to_be_bytes())
        .finalize();
    Some(substitute.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("test hex"))
            .collect()
    }

    fn seed(text: &str) -> [u8; 8] {
        hex(text).try_into().expect("8-byte test seed")
    }

    #[test]
    fn the_des_substitute_matches_the_rfc_4777_test_vector() {
        // R-NET-4, RFC 4777 section 5.1.
        let substitute = des_substitute(
            "USER123",
            "ABCDEFG",
            seed("7D4C2319F28004B2"),
            seed("08BEF662D851F4B1"),
            CodePage::Ccsid37,
        );
        assert_eq!(substitute.map(Vec::from), Some(hex("5A58BD50E4DD9B5F")));
    }

    #[test]
    fn the_des_substitute_matches_the_rfc_4777_negotiation_example() {
        // R-NET-4, RFC 4777 section 5: the substitute in the example IS reply.
        let substitute = des_substitute(
            "dummyusr",
            "dummypw",
            seed("7D3E488F18080404"),
            seed("4E4142334E414233"),
            CodePage::Ccsid37,
        );
        assert_eq!(substitute.map(Vec::from), Some(hex("DFB0402F22ABA3BA")));
    }

    #[test]
    fn the_sha1_substitute_matches_the_rfc_4777_test_vector() {
        // R-NET-4, RFC 4777 section 5.2.
        let substitute = sha1_substitute(
            "USER123",
            "AbCdEfGh123?+",
            seed("3E3A71C78795E5F5"),
            seed("B1C806D5D377D994"),
        );
        assert_eq!(
            substitute.map(Vec::from),
            Some(hex("E7FAB5F034BEDA42E91F439DD07532A24140E3DD"))
        );
    }

    #[test]
    fn a_password_never_shows_in_debug_output() {
        let credentials = Credentials {
            user: "QSECOFR".into(),
            password: Password::new("SECRET".into()),
            level: PasswordLevel::Des,
            code_page: CodePage::Ccsid37,
            client_seed: [0; 8],
        };
        assert!(!format!("{credentials:?}").contains("SECRET"));
    }

    #[test]
    fn des_refuses_passwords_it_cannot_carry() {
        let seeds = (seed("0000000000000000"), seed("0000000000000000"));
        let des =
            |user, password| des_substitute(user, password, seeds.0, seeds.1, CodePage::Ccsid37);
        assert!(des("USER", "").is_none());
        assert!(des("", "PW").is_none());
        assert!(des("USER", "ELEVENCHARS").is_none());
        assert!(des("USER", "ÆØÅ€").is_none(), "€ is not in CCSID 37");
        assert!(des("USER", "TENCHARS10").is_some());
        assert!(des("TENCHARS10", "PW").is_some());
    }

    #[test]
    fn a_ten_character_password_changes_the_des_substitute() {
        let des = |password| {
            des_substitute(
                "USER",
                password,
                seed("0102030405060708"),
                seed("0807060504030201"),
                CodePage::Ccsid37,
            )
        };
        assert_ne!(des("PASSWORD"), des("PASSWORD12"));
    }
}
