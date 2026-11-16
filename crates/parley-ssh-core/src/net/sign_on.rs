//! Signing on (R-SSH-4, R-SSH-12, R-SSH-13; RFC 4252, RFC 4256): public keys from the agent and key files, then
//! keyboard-interactive, then password, with every answer and passphrase asked of the user each time and never
//! kept (D16, D44).

use russh::MethodKind;
use russh::client::{self as ssh, AuthResult, KeyboardInteractiveAuthResponse};

use super::client::Client;
use super::{
    Command, Commands, Event, Link, Options, Prompt, PromptField, Target, public_key, wait_for,
};
use crate::{Error, Secret};

/// How often a method is tried before the next one, as OpenSSH's `NumberOfPasswordPrompts`.
const TRIES: usize = 3;

/// The methods tried, in order.
const ORDER: [MethodKind; 3] = [
    MethodKind::PublicKey,
    MethodKind::KeyboardInteractive,
    MethodKind::Password,
];

/// How often each method of [`ORDER`] is tried: public keys once (every key in that round), the others
/// [`TRIES`] times.
const fn limit(kind: MethodKind) -> usize {
    match kind {
        MethodKind::PublicKey => 1,
        _ => TRIES,
    }
}

pub(super) async fn sign_on(
    handle: &mut ssh::Handle<Client>,
    target: &Target,
    options: &Options,
    commands: &Commands,
    link: &Link,
) -> Result<(), Error> {
    // "none" asks the server which methods it accepts (RFC 4252 §5.2), and a few servers let it through.
    let mut methods = match handle.authenticate_none(target.user.as_str()).await? {
        AuthResult::Success => return Ok(()),
        AuthResult::Failure {
            remaining_methods, ..
        } => remaining_methods.to_vec(),
    };
    link.log(format!("The server accepts: {}", names(&methods)));
    let mut tries = [0; ORDER.len()];
    loop {
        let Some(index) = next_method(&methods, &tries) else {
            return Err(Error::SignOnRefused(names(&methods)));
        };
        tries[index] += 1;
        let outcome = match ORDER[index] {
            MethodKind::PublicKey => {
                public_key::sign_on(handle, target, options, commands, link).await?
            }
            MethodKind::KeyboardInteractive => {
                keyboard_interactive(handle, target, commands, link).await?
            }
            _ => password(handle, target, commands, link).await?,
        };
        match outcome {
            Outcome::Success => {
                link.log(format!("Signed on with {}", <&str>::from(&ORDER[index])));
                return Ok(());
            }
            Outcome::Failure(remaining) => {
                link.log(format!("{} was refused", <&str>::from(&ORDER[index])));
                methods = methods_after_failure(methods, remaining);
            }
        }
    }
}

/// The first method in [`ORDER`] the server accepts and that has tries left.
fn next_method(methods: &[MethodKind], tries: &[usize; ORDER.len()]) -> Option<usize> {
    ORDER
        .iter()
        .enumerate()
        .position(|(i, kind)| methods.contains(kind) && tries[i] < limit(*kind))
}

/// The methods to go on with after a refusal: those the server names, or, when it names none (some servers
/// leave the list empty), the ones it named before. The tries per method still end the sign-on.
fn methods_after_failure(before: Vec<MethodKind>, named: Vec<MethodKind>) -> Vec<MethodKind> {
    if named.is_empty() { before } else { named }
}

fn names(methods: &[MethodKind]) -> String {
    let names: Vec<&str> = methods.iter().map(<&str>::from).collect();
    if names.is_empty() {
        "nothing".to_owned()
    } else {
        names.join(", ")
    }
}

pub(super) enum Outcome {
    Success,
    Failure(Vec<MethodKind>),
}

async fn password(
    handle: &mut ssh::Handle<Client>,
    target: &Target,
    commands: &Commands,
    link: &Link,
) -> Result<Outcome, Error> {
    let prompt = Prompt {
        host: target.label(),
        name: String::new(),
        instructions: String::new(),
        fields: vec![PromptField {
            label: format!("Password for {}", target.label()),
            echo: false,
        }],
    };
    let mut answers = ask(prompt, commands, link).await?;
    let password = answers.pop().unwrap_or_default();
    Ok(
        match handle
            .authenticate_password(target.user.as_str(), password.expose())
            .await?
        {
            AuthResult::Success => Outcome::Success,
            AuthResult::Failure {
                remaining_methods, ..
            } => Outcome::Failure(remaining_methods.to_vec()),
        },
    )
}

async fn keyboard_interactive(
    handle: &mut ssh::Handle<Client>,
    target: &Target,
    commands: &Commands,
    link: &Link,
) -> Result<Outcome, Error> {
    let mut response = handle
        .authenticate_keyboard_interactive_start(target.user.as_str(), None)
        .await?;
    loop {
        match response {
            KeyboardInteractiveAuthResponse::Success => return Ok(Outcome::Success),
            KeyboardInteractiveAuthResponse::Failure {
                remaining_methods, ..
            } => {
                return Ok(Outcome::Failure(remaining_methods.to_vec()));
            }
            KeyboardInteractiveAuthResponse::InfoRequest {
                name,
                instructions,
                prompts,
            } => {
                // An empty round is answered with nothing (RFC 4256 §3.4).
                let answers = if prompts.is_empty() {
                    Vec::new()
                } else {
                    let fields = prompts
                        .into_iter()
                        .map(|p| PromptField {
                            label: p.prompt,
                            echo: p.echo,
                        })
                        .collect();
                    ask(
                        Prompt {
                            host: target.label(),
                            name,
                            instructions,
                            fields,
                        },
                        commands,
                        link,
                    )
                    .await?
                };
                let answers = answers.iter().map(|a| a.expose().to_owned()).collect();
                response = handle
                    .authenticate_keyboard_interactive_respond(answers)
                    .await?;
            }
        }
    }
}

/// Shows `prompt` and waits for the user's answers; Cancel ends the sign-on.
async fn ask(prompt: Prompt, commands: &Commands, link: &Link) -> Result<Vec<Secret>, Error> {
    ask_or_skip(prompt, commands, link)
        .await?
        .ok_or(Error::Cancelled)
}

/// Shows `prompt` and waits for the user's answers, one per field; `None` when Cancel was pressed.
pub(super) async fn ask_or_skip(
    prompt: Prompt,
    commands: &Commands,
    link: &Link,
) -> Result<Option<Vec<Secret>>, Error> {
    let fields = prompt.fields.len();
    link.send(Event::Prompt(prompt));
    let answers = wait_for(commands, |command| match command {
        Command::Answers(answers) => Some(answers),
        _ => None,
    })
    .await?;
    Ok(answers.map(|mut answers| {
        answers.resize(fields, Secret::default());
        answers
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_keys_come_first_once_then_keyboard_interactive_then_password_r_ssh_4_r_ssh_12() {
        let all = [
            MethodKind::Password,
            MethodKind::KeyboardInteractive,
            MethodKind::PublicKey,
        ];
        assert_eq!(next_method(&all, &[0, 0, 0]), Some(0));
        assert_eq!(
            next_method(&all, &[1, 0, 0]),
            Some(1),
            "public keys get one round"
        );
        assert_eq!(next_method(&all, &[1, 3, 0]), Some(2));
        assert_eq!(next_method(&all, &[1, 3, 3]), None);
        assert_eq!(next_method(&[MethodKind::HostBased], &[0, 0, 0]), None);
        assert_eq!(next_method(&[MethodKind::Password], &[0, 0, 2]), Some(2));
    }

    #[test]
    fn a_refusal_that_names_no_methods_keeps_the_earlier_ones_r_ssh_4() {
        let before = vec![MethodKind::Password];
        assert_eq!(methods_after_failure(before.clone(), vec![]), before);
        let named = vec![MethodKind::KeyboardInteractive];
        assert_eq!(methods_after_failure(before, named.clone()), named);
    }

    #[test]
    fn the_methods_are_named_for_the_message() {
        assert_eq!(
            names(&[MethodKind::PublicKey, MethodKind::Password]),
            "publickey, password"
        );
        assert_eq!(names(&[]), "nothing");
    }
}
