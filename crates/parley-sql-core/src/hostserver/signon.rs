//! The sign-on server's messages (R-SQL-2): the exchange of attributes, the sign-on, and the end.
//! Sans-I/O: these build the requests and read the replies; `net::hostserver` sends them.
//!
//! ```text
//! 0x7003 exchange attributes  client version (0x1101), data stream level (0x1102), seed (0x1103)
//! 0xF003 reply                return code; server version, level, seed, password level (0x1119)
//! 0x7004 sign on              template: credential type (DES 1, SHA-1 3);
//!                             client CCSID (0x1113), password substitute (0x1105), user (0x1104)
//! 0xF004 reply                return code; server CCSID (0x1114), …
//! 0x7006 end                  no reply
//! ```
//!
//! The layouts are Manticore's (MIT; D19), checked against the replies `make probe-sql`
//! recorded; replies carry a 4-byte return code as their template.

use crate::error::{Error, Result};
use parley_core::CodePage;
use parley_core::password::Password;

use super::substitute::{PasswordLevel, Profile, password_substitute};
use super::{Param, Record, ServerId};

const EXCHANGE_ATTRIBUTES: u16 = 0x7003;
const EXCHANGE_ATTRIBUTES_REPLY: u16 = 0xF003;
const SIGN_ON: u16 = 0x7004;
const SIGN_ON_REPLY: u16 = 0xF004;
const END: u16 = 0x7006;

/// Code points of the sign-on server's parameters.
pub(crate) mod cp {
    pub const VERSION: u16 = 0x1101;
    pub const LEVEL: u16 = 0x1102;
    pub const SEED: u16 = 0x1103;
    pub const USER: u16 = 0x1104;
    pub const SUBSTITUTE: u16 = 0x1105;
    pub const CLIENT_CCSID: u16 = 0x1113;
    pub const SERVER_CCSID: u16 = 0x1114;
    pub const PASSWORD_LEVEL: u16 = 0x1119;
    pub const JOB: u16 = 0x111F;
}

/// The client version and data stream level Parley gives, as the client Manticore observed.
const CLIENT_VERSION: u32 = 1;
const CLIENT_LEVEL: u16 = 2;
/// UTF-16, the client CCSID the sign-on request names.
const CLIENT_CCSID: u32 = 13488;

/// The exchange of attributes, with Parley's random seed.
pub fn exchange_attributes(client_seed: [u8; 8]) -> Record {
    Record::request(
        ServerId::SignOn,
        EXCHANGE_ATTRIBUTES,
        vec![
            Param::new(cp::VERSION, CLIENT_VERSION.to_be_bytes()),
            Param::new(cp::LEVEL, CLIENT_LEVEL.to_be_bytes()),
            Param::new(cp::SEED, client_seed),
        ],
    )
}

/// What the sign-on server says about itself before a sign-on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerAttributes {
    /// Version, release, modification: `[0, 7, 5, 0]` for IBM i 7.5.
    pub release: [u8; 4],
    pub level: u16,
    pub seed: [u8; 8],
    /// The system value `QPWDLVL`; 0 when the server does not say.
    pub password_level: u8,
    /// The server job, as `number/user/name`.
    pub job: Option<String>,
}

impl ServerAttributes {
    /// The release as IBM writes it: "V7R5M0".
    pub fn release_name(&self) -> String {
        let [_, version, release, modification] = self.release;
        format!("V{version}R{release}M{modification}")
    }
}

/// Reads the reply to [`exchange_attributes`].
pub fn read_attributes(reply: &Record) -> Result<ServerAttributes> {
    expect_reply(reply, EXCHANGE_ATTRIBUTES_REPLY)?;
    let code = return_code(reply)?;
    if code != 0 {
        return Err(refused(code));
    }
    Ok(ServerAttributes {
        release: fixed(reply, cp::VERSION)?,
        level: u16::from_be_bytes(fixed(reply, cp::LEVEL)?),
        seed: fixed(reply, cp::SEED)?,
        password_level: reply
            .param(cp::PASSWORD_LEVEL)
            .and_then(|l| l.first().copied())
            .unwrap_or(0),
        job: reply
            .param(cp::JOB)
            .and_then(|job| job.get(4..)) // a CCSID, then the name in EBCDIC
            .map(|name| ebcdic(name, CodePage::Ccsid37)),
    })
}

/// The sign-on request for `profile`, with the substitute the server's password level asks for.
/// Fails before anything is sent when the user or password cannot be signed on with this way.
pub fn sign_on(
    server: &ServerAttributes,
    profile: &Profile,
    password: &Password,
    client_seed: [u8; 8],
    code_page: CodePage,
) -> Result<Record> {
    let level = PasswordLevel::from_qpwdlvl(server.password_level)?;
    let substitute = password_substitute(
        level,
        profile,
        password,
        server.seed,
        client_seed,
        code_page,
    )?;
    let credential_type = match level {
        PasswordLevel::Des => 0x01,
        PasswordLevel::Sha1 => 0x03,
    };
    let mut request = Record::request(
        ServerId::SignOn,
        SIGN_ON,
        vec![
            Param::new(cp::CLIENT_CCSID, CLIENT_CCSID.to_be_bytes()),
            Param::new(cp::SUBSTITUTE, substitute),
            Param::new(cp::USER, profile.ebcdic()),
        ],
    );
    request.template = vec![credential_type];
    Ok(request)
}

/// What the host says once it has taken the sign-on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedOn {
    /// The host's CCSID, when it says.
    pub server_ccsid: Option<u32>,
}

/// Reads the reply to [`sign_on`]: the host's refusal as an error the operator can act on.
pub fn read_sign_on(reply: &Record) -> Result<SignedOn> {
    expect_reply(reply, SIGN_ON_REPLY)?;
    let code = return_code(reply)?;
    if code != 0 {
        return Err(refused(code));
    }
    let server_ccsid = match reply.param(cp::SERVER_CCSID) {
        Some(bytes) => Some(u32::from_be_bytes(bytes.try_into().map_err(|_| {
            Error::host_server(format!("a server CCSID of {} bytes", bytes.len()))
        })?)),
        None => None,
    };
    Ok(SignedOn { server_ccsid })
}

/// Ends the sign-on server's job; it does not answer.
pub fn end() -> Record {
    Record::request(ServerId::SignOn, END, Vec::new())
}

/// Why the host refused, by its return code (as Manticore classifies them; D19).
fn refused(code: u32) -> Error {
    let message = match code {
        0x0002_0001 => "The user profile does not exist on this host.".to_owned(),
        0x0002_0002 => {
            "The user profile is disabled. Ask the security officer to enable it.".to_owned()
        }
        0x0003_000B => "The password is not correct.".to_owned(),
        0x0003_000C => "The password is not correct. One more wrong password disables the user \
                        profile: check it before signing on again."
            .to_owned(),
        0x0003_000D => {
            "The password has expired. Change it in a 5250 session, then sign on again.".to_owned()
        }
        0x0003_0010 => "The user profile has no password (*NONE), so it cannot sign on.".to_owned(),
        _ => match code >> 16 {
            0x0001 => format!(
                "The host could not read Parley's sign-on request (code {code:08X}). Please \
                 report this."
            ),
            0x0002 => format!("The host refused the user profile (code {code:08X})."),
            0x0003 => format!("The host refused the password (code {code:08X})."),
            0x0004 => format!("The host's security refused the sign-on (code {code:08X})."),
            0x0005 => format!("An exit program on the host refused the sign-on (code {code:08X})."),
            _ => format!("The host refused the sign-on (code {code:08X})."),
        },
    };
    Error::SignOn(message)
}

fn expect_reply(reply: &Record, id: u16) -> Result<()> {
    if reply.server == ServerId::SignOn && reply.id == id {
        Ok(())
    } else {
        Err(Error::host_server(format!(
            "reply {:#06X} from the {:?} server where {id:#06X} from the sign-on server was due",
            reply.id, reply.server
        )))
    }
}

/// The return code a reply carries as its template.
fn return_code(reply: &Record) -> Result<u32> {
    super::u32_at(&reply.template, 0)
}

/// A parameter that must be there with exactly `N` bytes.
fn fixed<const N: usize>(reply: &Record, code_point: u16) -> Result<[u8; N]> {
    let bytes = reply.param(code_point).ok_or_else(|| {
        Error::host_server(format!("a reply without parameter {code_point:#06X}"))
    })?;
    bytes.try_into().map_err(|_| {
        Error::host_server(format!(
            "parameter {code_point:#06X} with {} bytes, not {N}",
            bytes.len()
        ))
    })
}

fn ebcdic(bytes: &[u8], code_page: CodePage) -> String {
    bytes
        .iter()
        .map(|&b| code_page.to_char(b))
        .collect::<String>()
        .trim_end()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::{
        OWNER_EXCHANGE_ATTRIBUTES, OWNER_SIGN_ON_REPLY, OWNER_SIGN_ON_REQUEST,
        PUB400_EXCHANGE_ATTRIBUTES,
    };
    use super::*;

    fn attributes(fixture: &[u8]) -> ServerAttributes {
        let record = Record::decode(fixture).expect("a recorded reply");
        read_attributes(&record).expect("valid attributes")
    }

    /// A reply from the sign-on server with `code` and `params`.
    fn reply(id: u16, code: u32, params: Vec<Param>) -> Record {
        Record {
            server: ServerId::SignOn,
            cs_instance: 0,
            correlation: 0,
            id,
            template: code.to_be_bytes().to_vec(),
            params,
        }
    }

    #[test]
    fn the_exchange_of_attributes_is_the_request_both_hosts_answered_r_sql_2() {
        // The bytes the probe sent to the owner's IBM i and PUB400 (R-NET-8), with its seed.
        let request = exchange_attributes(*b"PARLEY\x00\x01")
            .encode()
            .expect("short");
        assert_eq!(
            request,
            [
                0, 0, 0, 52, 0, 0, 0xE0, 0x09, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x70,
                0x03, // header
                0, 0, 0, 10, 0x11, 0x01, 0, 0, 0, 1, // client version 1
                0, 0, 0, 8, 0x11, 0x02, 0, 2, // data stream level 2
                0, 0, 0, 14, 0x11, 0x03, b'P', b'A', b'R', b'L', b'E', b'Y', 0, 1, // seed
            ]
        );
    }

    #[test]
    fn recorded_attributes_give_release_seed_level_and_job_r_sql_2() {
        let pub400 = attributes(&PUB400_EXCHANGE_ATTRIBUTES);
        assert_eq!(pub400.release_name(), "V7R5M0");
        assert_eq!(pub400.level, 15);
        assert_eq!(pub400.password_level, 3);
        assert_eq!(
            pub400.seed,
            [0x25, 0x32, 0xdc, 0x13, 0x51, 0x5d, 0x34, 0x49]
        );
        assert_eq!(pub400.job.as_deref(), Some("722903/QUSER/QZSOSIGN"));
        let owner = attributes(&OWNER_EXCHANGE_ATTRIBUTES);
        assert_eq!(owner.release_name(), "V7R3M0");
        assert_eq!(owner.password_level, 0);
    }

    #[test]
    fn a_des_sign_on_carries_type_1_ccsid_substitute_and_user_r_sql_2() {
        let owner = attributes(&OWNER_EXCHANGE_ATTRIBUTES);
        let profile = Profile::new("usr").expect("valid");
        let password = Password::new("secret".to_owned());
        let request =
            sign_on(&owner, &profile, &password, [1; 8], CodePage::Ccsid37).expect("a request");
        assert_eq!(request.id, 0x7004);
        assert_eq!(request.template, [0x01]);
        let codes: Vec<u16> = request.params.iter().map(|p| p.code_point).collect();
        assert_eq!(codes, [0x1113, 0x1105, 0x1104]);
        assert_eq!(request.param(0x1113), Some(&13488u32.to_be_bytes()[..]));
        assert_eq!(request.param(0x1105).map(<[u8]>::len), Some(8));
        assert_eq!(request.param(0x1104), Some(&profile.ebcdic()[..]));
        let bytes = request.encode().expect("short");
        assert!(
            !bytes.windows(6).any(|w| w == b"secret"),
            "no password in the request"
        );
    }

    #[test]
    fn the_des_sign_on_is_byte_for_byte_the_one_the_owners_ibm_i_took_r_sql_2() {
        let owner = attributes(&OWNER_EXCHANGE_ATTRIBUTES);
        let profile = Profile::new("parley").expect("valid");
        let password = Password::new("anything".to_owned());
        let mut request =
            sign_on(&owner, &profile, &password, [1; 8], CodePage::Ccsid277).expect("a request");
        for param in &mut request.params {
            if param.code_point == 0x1105 {
                param.data.fill(0); // as the recording keeps it
            }
        }
        assert_eq!(request.encode().expect("short"), OWNER_SIGN_ON_REQUEST);
    }

    #[test]
    fn the_owners_ibm_i_took_the_sign_on_and_named_ccsid_277_r_sql_2() {
        let reply = Record::decode(&OWNER_SIGN_ON_REPLY).expect("a recorded reply");
        assert_eq!(
            read_sign_on(&reply).ok(),
            Some(SignedOn {
                server_ccsid: Some(277)
            })
        );
    }

    #[test]
    fn a_sha1_sign_on_carries_type_3_and_a_20_byte_substitute_r_sql_2() {
        let pub400 = attributes(&PUB400_EXCHANGE_ATTRIBUTES);
        let profile = Profile::new("usr").expect("valid");
        let password = Password::new("Långt kodeord æøå".to_owned());
        let request =
            sign_on(&pub400, &profile, &password, [1; 8], CodePage::Ccsid277).expect("a request");
        assert_eq!(request.template, [0x03]);
        assert_eq!(request.param(0x1105).map(<[u8]>::len), Some(20));
    }

    #[test]
    fn a_password_the_level_cannot_carry_is_refused_before_sending_r_sql_2() {
        let owner = attributes(&OWNER_EXCHANGE_ATTRIBUTES);
        let profile = Profile::new("usr").expect("valid");
        let long = Password::new("elevenchars".to_owned());
        assert!(sign_on(&owner, &profile, &long, [1; 8], CodePage::Ccsid37).is_err());
        let mut level4 = owner;
        level4.password_level = 4;
        let short = Password::new("pw".to_owned());
        assert!(sign_on(&level4, &profile, &short, [1; 8], CodePage::Ccsid37).is_err());
    }

    #[test]
    fn a_taken_sign_on_gives_the_host_ccsid_r_sql_2() {
        let ok = reply(0xF004, 0, vec![Param::new(0x1114, 37u32.to_be_bytes())]);
        assert_eq!(
            read_sign_on(&ok).ok(),
            Some(SignedOn {
                server_ccsid: Some(37)
            })
        );
        let bare = reply(0xF004, 0, Vec::new());
        assert_eq!(
            read_sign_on(&bare).ok(),
            Some(SignedOn { server_ccsid: None })
        );
    }

    #[test]
    fn refusals_are_sentences_the_operator_can_act_on_r_sql_2() {
        for (code, words) in [
            (0x0003_000B, "password is not correct"),
            (0x0003_000C, "One more wrong password disables"),
            (0x0003_000D, "expired"),
            (0x0002_0001, "does not exist"),
            (0x0002_0002, "disabled"),
            (0x0001_0004, "could not read"),
            (0x0009_0000, "code 00090000"),
        ] {
            let error = read_sign_on(&reply(0xF004, code, Vec::new())).expect_err("refused");
            assert!(error.to_string().contains(words), "{code:08X}: {error}");
        }
    }

    #[test]
    fn a_reply_that_is_not_the_one_due_is_an_error_r_sql_2() {
        let wrong = reply(0xF003, 0, Vec::new());
        assert!(read_sign_on(&wrong).is_err());
        let mut no_code = reply(0xF004, 0, Vec::new());
        no_code.template.clear();
        assert!(read_sign_on(&no_code).is_err());
        let mut short_seed = Record::decode(&PUB400_EXCHANGE_ATTRIBUTES).expect("valid");
        if let Some(seed) = short_seed
            .params
            .iter_mut()
            .find(|p| p.code_point == 0x1103)
        {
            seed.data.truncate(4);
        }
        assert!(read_attributes(&short_seed).is_err());
    }

    #[test]
    fn end_is_a_bare_request_r_sql_2() {
        assert_eq!(
            end().encode().expect("short"),
            [
                0, 0, 0, 20, 0, 0, 0xE0, 0x09, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x70, 0x06
            ]
        );
    }
}
