//! The password substitute (R-SQL-2): what goes to the host instead of the password. The host
//! sends a random seed, Parley another; the substitute is built from both, the user and the
//! password, and is no use for another connection. RFC 4777 section 5 specifies both
//! algorithms, with test vectors, for telnet's sign-on; IBM i takes the same substitutes at
//! the sign-on of its host servers (to be confirmed by the first recorded sign-on, R-NET-9).
//!
//! The host's password level (system value `QPWDLVL`, which the sign-on server's exchange of
//! attributes names) decides the algorithm: DES for levels 0 and 1, SHA-1 for 2 and 3.
//! Restored from the auto sign-on Parley removed (R-NET-7): the password is still typed each
//! time and never kept (D16).

use des::Des;
use des::cipher::{BlockEncrypt, KeyInit};
use sha1::{Digest, Sha1};

use crate::CodePage;
use crate::error::{Error, Result};
use crate::password::Password;

/// The substitute's algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordLevel {
    /// `QPWDLVL` 0 or 1: passwords of up to 10 characters, not case-sensitive.
    Des,
    /// `QPWDLVL` 2 or 3: passphrases of up to 128 characters, case-sensitive.
    Sha1,
}

impl PasswordLevel {
    /// The algorithm for the password level the host named; another level is an error that
    /// names it.
    pub fn from_qpwdlvl(level: u8) -> Result<Self> {
        match level {
            0 | 1 => Ok(Self::Des),
            2 | 3 => Ok(Self::Sha1),
            other => Err(Error::host_server(format!(
                "password level {other}, which Parley cannot sign on with yet"
            ))),
        }
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

/// The sequence number: one substitute per connection (RFC 4777 section 5.1 step 5).
const SEQUENCE: u64 = 1;
const BLANK: u8 = 0x40;

/// The substitute for `password` at `level`. An error, never naming the password's characters,
/// when the user or password cannot be sent this way.
pub fn password_substitute(
    level: PasswordLevel,
    profile: &Profile,
    password: &Password,
    server_seed: [u8; 8],
    client_seed: [u8; 8],
    code_page: CodePage,
) -> Result<Vec<u8>> {
    match level {
        PasswordLevel::Des => des_substitute(
            profile,
            password.expose(),
            server_seed,
            client_seed,
            code_page,
        )
        .map(Vec::from),
        PasswordLevel::Sha1 => {
            sha1_substitute(profile, password.expose(), server_seed, client_seed).map(Vec::from)
        }
    }
}

/// A user profile's name as the sign-on takes it: 1 to 10 of A–Z, 0–9, `#`, `$`, `@` and `_`,
/// in capitals. The national letters that sit on the keys of `#`, `$` and `@` are folded onto
/// them (Æ Ä Ñ £ → `#`, Å ¥ İ → `$`, Ø Ö Ð § Ş → `@`), as IBM i does: the sign-on table of
/// Manticore (MIT; D19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// EBCDIC, blank-padded: what the sign-on request and the DES substitute carry.
    ebcdic: [u8; 10],
    len: usize,
    /// The name after folding, which the SHA-1 substitute hashes.
    canonical: String,
}

impl Profile {
    pub fn new(name: &str) -> Result<Self> {
        let name = name.trim();
        let mut ebcdic = [BLANK; 10];
        let mut canonical = String::new();
        let mut len = 0;
        for (i, c) in name.chars().enumerate() {
            let slot = ebcdic.get_mut(i).ok_or_else(|| {
                Error::SignOn("A user name has at most 10 characters.".to_owned())
            })?;
            let (byte, folded) = sign_on_char(c).ok_or_else(|| {
                Error::SignOn(format!(
                    "A user name may only hold A–Z, 0–9, #, $, @ and _ (character {} does not).",
                    i + 1
                ))
            })?;
            *slot = byte;
            canonical.push(folded);
            len = i + 1;
        }
        if len == 0 {
            return Err(Error::SignOn("Type a user name.".to_owned()));
        }
        Ok(Self {
            ebcdic,
            len,
            canonical,
        })
    }

    /// The name in EBCDIC, padded with blanks to 10 bytes.
    pub const fn ebcdic(&self) -> [u8; 10] {
        self.ebcdic
    }

    /// The name as the operator should see it (after folding).
    pub fn name(&self) -> &str {
        &self.canonical
    }
}

/// One character of a user name: its EBCDIC byte and the character it stands for.
fn sign_on_char(c: char) -> Option<(u8, char)> {
    let mut upper = c.to_uppercase();
    let (Some(c), None) = (upper.next(), upper.next()) else {
        return None;
    };
    let c = match c {
        'Æ' | 'Ä' | 'Ñ' | '£' => '#',
        'Å' | '¥' | 'İ' => '$',
        'Ø' | 'Ö' | 'Ð' | '§' | 'Ş' => '@',
        other => other,
    };
    let byte = match c {
        '#' => 0x7B,
        '$' => 0x5B,
        '@' => 0x7C,
        '_' => 0x6D,
        '0'..='9' => 0xF0 + (c as u8 - b'0'),
        'A'..='I' => 0xC1 + (c as u8 - b'A'),
        'J'..='R' => 0xD1 + (c as u8 - b'J'),
        'S'..='Z' => 0xE2 + (c as u8 - b'S'),
        _ => return None,
    };
    Some((byte, c))
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

/// RFC 4777 section 5.1: the 8-byte DES substitute (password levels 0 and 1: up to 10
/// characters, not case-sensitive).
fn des_substitute(
    profile: &Profile,
    password: &str,
    server_seed: [u8; 8],
    client_seed: [u8; 8],
    code_page: CodePage,
) -> Result<[u8; 8]> {
    let user = profile.ebcdic.get(..profile.len).unwrap_or_default();
    let password = ebcdic_upper(password.trim_end(), code_page).ok_or_else(|| {
        Error::SignOn("The password holds a character the session's code page lacks.".to_owned())
    })?;
    if !(1..=10).contains(&password.len()) {
        return Err(Error::SignOn(
            "At this host's password level a password has 1 to 10 characters.".to_owned(),
        ));
    }
    let folded = fold_user(user);
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
    let user16 = padded::<16>(user);
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
    Ok(chain)
}

/// Text in UTF-16, big-endian as on IBM i.
fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

/// RFC 4777 section 5.2: the 20-byte SHA-1 substitute (password levels 2 and 3: up to 128
/// characters, case-sensitive). Trailing blanks are not part of a password on IBM i.
fn sha1_substitute(
    profile: &Profile,
    password: &str,
    server_seed: [u8; 8],
    client_seed: [u8; 8],
) -> Result<[u8; 20]> {
    let password = password.trim_end_matches([' ', '\0', '\u{3000}']);
    let password = utf16(password);
    if password.is_empty() || password.len() / 2 > 128 {
        return Err(Error::SignOn(
            "At this host's password level a password has 1 to 128 characters.".to_owned(),
        ));
    }
    let user = utf16(&format!("{:<10}", profile.canonical));
    let token = Sha1::new()
        .chain_update(&user)
        .chain_update(&password)
        .finalize();
    let substitute = Sha1::new()
        .chain_update(token)
        .chain_update(server_seed)
        .chain_update(client_seed)
        .chain_update(&user)
        .chain_update(SEQUENCE.to_be_bytes())
        .finalize();
    Ok(substitute.into())
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

    fn profile(name: &str) -> Profile {
        Profile::new(name).expect("a valid user name")
    }

    #[test]
    fn the_des_substitute_matches_the_rfc_4777_test_vector() {
        // R-SQL-2, RFC 4777 section 5.1.
        let substitute = des_substitute(
            &profile("USER123"),
            "ABCDEFG",
            seed("7D4C2319F28004B2"),
            seed("08BEF662D851F4B1"),
            CodePage::Ccsid37,
        );
        assert_eq!(
            substitute.ok().map(Vec::from),
            Some(hex("5A58BD50E4DD9B5F"))
        );
    }

    #[test]
    fn the_des_substitute_matches_the_rfc_4777_negotiation_example() {
        // R-SQL-2, RFC 4777 section 5: the substitute in the example IS reply.
        let substitute = des_substitute(
            &profile("dummyusr"),
            "dummypw",
            seed("7D3E488F18080404"),
            seed("4E4142334E414233"),
            CodePage::Ccsid37,
        );
        assert_eq!(
            substitute.ok().map(Vec::from),
            Some(hex("DFB0402F22ABA3BA"))
        );
    }

    #[test]
    fn the_sha1_substitute_matches_the_rfc_4777_test_vector() {
        // R-SQL-2, RFC 4777 section 5.2.
        let substitute = sha1_substitute(
            &profile("USER123"),
            "AbCdEfGh123?+",
            seed("3E3A71C78795E5F5"),
            seed("B1C806D5D377D994"),
        );
        assert_eq!(
            substitute.ok().map(Vec::from),
            Some(hex("E7FAB5F034BEDA42E91F439DD07532A24140E3DD"))
        );
    }

    #[test]
    fn trailing_blanks_are_not_part_of_a_password_r_sql_2() {
        let seeds = (seed("3E3A71C78795E5F5"), seed("B1C806D5D377D994"));
        let sha1 = |password| sha1_substitute(&profile("USER123"), password, seeds.0, seeds.1).ok();
        assert_eq!(sha1("AbCdEfGh123?+  "), sha1("AbCdEfGh123?+"));
        let des = |password| {
            des_substitute(
                &profile("USER123"),
                password,
                seeds.0,
                seeds.1,
                CodePage::Ccsid37,
            )
            .ok()
        };
        assert_eq!(des("ABCDEFG "), des("ABCDEFG"));
    }

    #[test]
    fn the_probed_password_levels_choose_des_and_sha1_and_level_4_is_named_r_sql_2() {
        use crate::hostserver::Record;
        use crate::hostserver::fixtures::{OWNER_EXCHANGE_ATTRIBUTES, PUB400_EXCHANGE_ATTRIBUTES};
        let level = |fixture: &[u8]| {
            let record = Record::decode(fixture).expect("a recorded reply");
            let level = record.param(0x1119).and_then(|l| l.first().copied());
            PasswordLevel::from_qpwdlvl(level.expect("a password level")).ok()
        };
        assert_eq!(level(&OWNER_EXCHANGE_ATTRIBUTES), Some(PasswordLevel::Des));
        assert_eq!(
            level(&PUB400_EXCHANGE_ATTRIBUTES),
            Some(PasswordLevel::Sha1)
        );
        let error = PasswordLevel::from_qpwdlvl(4).expect_err("not yet");
        assert!(error.to_string().contains("password level 4"), "{error}");
    }

    #[test]
    fn a_user_name_is_capitals_digits_and_hash_dollar_at_underscore_r_sql_2() {
        assert_eq!(profile("qsecofr").name(), "QSECOFR");
        assert_eq!(
            profile(" usr_1 ").ebcdic(),
            [0xE4, 0xE2, 0xD9, 0x6D, 0xF1, 0x40, 0x40, 0x40, 0x40, 0x40]
        );
        assert_eq!(
            profile("#$@").ebcdic().get(..3),
            Some(&[0x7B, 0x5B, 0x7C][..])
        );
        for bad in ["", "   ", "ELEVENCHARS", "A B", "A-B", "a.b", "ß"] {
            assert!(Profile::new(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn danish_letters_in_a_user_name_fold_onto_hash_dollar_at_r_sql_2() {
        // In CCSID 277 Æ, Ø and Å sit where CCSID 37 has #, @ and $: the same bytes, and the
        // SHA-1 substitute hashes the folded name.
        let danish = profile("æøå");
        assert_eq!(danish.name(), "#@$");
        assert_eq!(danish.ebcdic().get(..3), Some(&[0x7B, 0x7C, 0x5B][..]));
        let seeds = (seed("0102030405060708"), seed("0807060504030201"));
        assert_eq!(
            sha1_substitute(&danish, "pw", seeds.0, seeds.1).ok(),
            sha1_substitute(&profile("#@$"), "pw", seeds.0, seeds.1).ok()
        );
    }

    #[test]
    fn des_refuses_passwords_it_cannot_carry() {
        let seeds = (seed("0000000000000000"), seed("0000000000000000"));
        let des = |password| {
            des_substitute(
                &profile("USER"),
                password,
                seeds.0,
                seeds.1,
                CodePage::Ccsid37,
            )
        };
        assert!(des("").is_err());
        assert!(des("ELEVENCHARS").is_err());
        let error = des("ÆØÅ€").expect_err("€ is not in CCSID 37");
        assert!(
            !error.to_string().contains('€'),
            "the password stays out of errors"
        );
        assert!(des("TENCHARS10").is_ok());
        assert!(
            des_substitute(
                &profile("TENCHARS10"),
                "PW",
                seeds.0,
                seeds.1,
                CodePage::Ccsid37
            )
            .is_ok()
        );
    }

    #[test]
    fn a_ten_character_password_changes_the_des_substitute() {
        let des = |password| {
            des_substitute(
                &profile("USER"),
                password,
                seed("0102030405060708"),
                seed("0807060504030201"),
                CodePage::Ccsid37,
            )
            .ok()
        };
        assert_ne!(des("PASSWORD"), des("PASSWORD12"));
    }
}
