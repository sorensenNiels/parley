//! Signing on (R-SSH-4; RFC 4252, RFC 4256): keyboard-interactive, then password, with every answer asked of the
//! user each time and never kept (D16, D44).

use russh::MethodKind;
use russh::client::{self as ssh, AuthResult, KeyboardInteractiveAuthResponse};

use super::client::Client;
use super::{Command, Commands, Event, Link, Prompt, PromptField, Target, wait_for};
use crate::{Error, Secret};

/// How often a method is tried before the next one, as OpenSSH's `NumberOfPasswordPrompts`.
const TRIES: usize = 3;

/// The methods tried, in order.
const ORDER: [MethodKind; 2] = [MethodKind::KeyboardInteractive, MethodKind::Password];

pub(super) async fn sign_on(
    handle: &mut ssh::Handle<Client>,
    target: &Target,
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
        .position(|(i, kind)| methods.contains(kind) && tries[i] < TRIES)
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

enum Outcome {
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
    let fields = prompt.fields.len();
    link.send(Event::Prompt(prompt));
    let answers = wait_for(commands, |command| match command {
        Command::Answers(answers) => Some(answers),
        _ => None,
    })
    .await?;
    let mut answers = answers.ok_or(Error::Cancelled)?;
    answers.resize(fields, Secret::default());
    Ok(answers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_interactive_comes_before_password_and_each_gets_three_tries_r_ssh_4() {
        let both = [MethodKind::Password, MethodKind::KeyboardInteractive];
        assert_eq!(next_method(&both, &[0, 0]), Some(0));
        assert_eq!(next_method(&both, &[3, 0]), Some(1));
        assert_eq!(next_method(&both, &[3, 3]), None);
        assert_eq!(next_method(&[MethodKind::PublicKey], &[0, 0]), None);
        assert_eq!(next_method(&[MethodKind::Password], &[0, 2]), Some(1));
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
