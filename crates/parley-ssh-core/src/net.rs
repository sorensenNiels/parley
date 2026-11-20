//! A connection on a thread of its own (D42): russh on a current-thread tokio runtime, so nothing outside this
//! module is async. The UI sends [`Command`]s and reads [`Event`]s; output goes straight into the shared
//! [`Terminal`], whose replies go back to the host from here.

pub mod agent;
mod client;
mod jobs;
pub mod jump;
mod public_key;
mod sign_on;
mod transport;

use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::time::Duration;

use russh::{ChannelMsg, Pty, client as ssh};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::known_hosts::{HostKeyStatus, KnownHosts};
use crate::term::Terminal;
use crate::{Error, Secret};
pub use jobs::{Exec, JobEnding, JobId, Stream};
pub use transport::{AddressFamily, Proxy};

/// How long connecting may take before it is given up.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// How long the server has to answer an environment variable (R-SSH-28).
const ENV_ANSWER: Duration = Duration::from_secs(5);
/// What the shell is told the terminal is (R-SSH-5).
const TERMINAL_TYPE: &str = "xterm-256color";

/// Where to connect and as whom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub host: String,
    pub port: u16,
    pub user: String,
}

impl Target {
    /// `user@host`, with `:port` when it is not 22; without `user@` while the user is not known (R-SSH-62).
    pub fn label(&self) -> String {
        let user = if self.user.is_empty() {
            String::new()
        } else {
            format!("{}@", self.user)
        };
        if self.port == 22 {
            format!("{user}{}", self.host)
        } else {
            format!("{user}{}:{}", self.host, self.port)
        }
    }
}

/// How to sign on and what to allow, besides the target (R-SSH-12, R-SSH-13).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// Private key files, tried in this order after the agent's keys (R-SSH-12).
    pub key_files: Vec<std::path::PathBuf>,
    /// The running agent to sign on with; `None` leaves agents out (R-SSH-13).
    pub agent: Option<agent::AgentAddress>,
    /// Only the agent key with this SHA-256 fingerprint (`SHA256:…`); `None`: each of the agent's keys.
    pub agent_key: Option<String>,
    /// Let the server use the agent while connected (`auth-agent-req@openssh.com`); off by default.
    pub agent_forwarding: bool,
    /// Jump hosts reached first, in order, each through the one before (R-SSH-43).
    pub jumps: Vec<jump::Jump>,
    /// How the connection is kept and carried (R-SSH-26, R-SSH-27, R-SSH-29).
    pub transport: Transport,
    /// What runs on the server (R-SSH-28).
    pub shell: Shell,
}

/// How the connection is kept and carried (D51).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transport {
    /// `keepalive@openssh.com` after this long without a word from the server; three unanswered end it (R-SSH-26).
    pub keepalive: Option<Duration>,
    pub family: AddressFamily,
    /// Keys are exchanged again after this long, and after this many bytes each way (R-SSH-27).
    pub rekey_after: Duration,
    pub rekey_bytes: usize,
    /// `zlib@openssh.com` compression (R-SSH-27).
    pub compression: bool,
    /// Before the first hop (R-SSH-29).
    pub proxy: Proxy,
}

impl Default for Transport {
    fn default() -> Self {
        Self {
            keepalive: None,
            family: AddressFamily::Any,
            rekey_after: Duration::from_secs(3600),
            rekey_bytes: 1 << 30,
            compression: false,
            proxy: Proxy::None,
        }
    }
}

/// What runs on the server (R-SSH-28).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shell {
    /// A command instead of the shell, still with a pseudo-terminal.
    pub command: Option<String>,
    /// Environment variables sent as `env` requests; the server may refuse them.
    pub env: Vec<(String, String)>,
    /// What the pseudo-terminal says it is.
    pub terminal_type: String,
}

impl Default for Shell {
    fn default() -> Self {
        Self {
            command: None,
            env: Vec::new(),
            terminal_type: TERMINAL_TYPE.to_owned(),
        }
    }
}

/// What the UI tells the connection.
#[derive(Debug)]
pub enum Command {
    /// Bytes typed or pasted, already encoded (`keys`).
    Input(Vec<u8>),
    /// The window's new size in cells.
    Resize {
        columns: usize,
        rows: usize,
    },
    /// The answer to [`Event::HostKey`].
    HostKey(HostKeyAnswer),
    /// The answers to [`Event::Prompt`], one per field; `None` cancels signing on.
    Answers(Option<Vec<Secret>>),
    /// Runs a command on a side channel beside the shell, once signed on (Phase 31).
    Exec(Exec),
    /// Ends a job started with [`Command::Exec`].
    EndJob(JobId),
    Disconnect,
}

/// What the user chose for a host key that was not known (R-SSH-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKeyAnswer {
    AcceptAndSave,
    ConnectOnce,
    Cancel,
}

/// A host key the user must decide on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostKeyQuestion {
    pub host: String,
    pub port: u16,
    pub key_type: String,
    pub fingerprint: String,
    /// [`HostKeyStatus::Unknown`] or [`HostKeyStatus::Changed`].
    pub status: HostKeyStatus,
}

/// A request for secret answers: the password, or a keyboard-interactive round (RFC 4256 §3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    /// Who asks: `user@host` of the hop being signed on to (R-SSH-43).
    pub host: String,
    pub name: String,
    pub instructions: String,
    pub fields: Vec<PromptField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptField {
    pub label: String,
    /// Show what is typed (a user name), or hide it (a password, a code).
    pub echo: bool,
}

/// The algorithms in use, for the status bar (R-SSH-10).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Algorithms {
    pub kex: String,
    pub host_key: String,
    pub cipher: String,
    pub mac: String,
}

/// What the connection tells the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A line for the event log (R-SSH-16): never a password or a key.
    Log(String),
    HostKey(HostKeyQuestion),
    /// The server's banner, shown before signing on (R-SSH-4).
    Banner(String),
    Prompt(Prompt),
    /// The user name the target was asked for, because the session named none (R-SSH-62).
    User(String),
    /// Key exchange is done.
    Secured {
        algorithms: Algorithms,
        fingerprint: String,
    },
    /// Signed on and the shell is running.
    Started,
    /// Something went wrong that did not end the connection, in plain words (a key file that cannot be read).
    Problem(String),
    /// Output of a job started with [`Command::Exec`], as it comes.
    JobOutput {
        id: JobId,
        stream: Stream,
        data: Vec<u8>,
    },
    /// A job ended, or never started; nothing more comes from it.
    JobEnded {
        id: JobId,
        ending: JobEnding,
    },
    Ended(Ending),
}

/// How a connection ended (R-SSH-5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ending {
    /// The shell ended with this exit status.
    Exited(u32),
    /// The shell was ended by this signal.
    Signal(String),
    /// The server closed the channel or the connection without saying why.
    Closed,
    /// The user disconnected.
    Disconnected,
    /// The host key or the sign-on was refused, by the user or the server, in plain words: trying again by
    /// itself would not help (R-SSH-51).
    Refused(String),
    /// Something went wrong, in plain words.
    Failed(String),
}

/// A connection running on its own thread. Dropping it disconnects.
#[derive(Debug)]
pub struct Connection {
    commands: UnboundedSender<Command>,
    events: mpsc::Receiver<Event>,
    terminal: Arc<Mutex<Terminal>>,
}

impl Connection {
    /// Starts connecting to `target`, drawing into `terminal`. `repaint` is called whenever there is something
    /// new to show or an event to read.
    pub fn open(
        target: Target,
        options: Options,
        known_hosts: KnownHosts,
        terminal: Arc<Mutex<Terminal>>,
        repaint: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let (commands, command_rx) = unbounded_channel();
        let (event_tx, events) = mpsc::channel();
        let link = Link {
            events: event_tx,
            repaint: Arc::new(repaint),
        };
        let shared = Arc::clone(&terminal);
        let started = std::thread::Builder::new()
            .name("parley-ssh".into())
            .spawn({
                let link = link.clone();
                move || {
                    let ending = match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(runtime) => runtime.block_on(run(
                            target,
                            options,
                            known_hosts,
                            shared,
                            command_rx,
                            link.clone(),
                        )),
                        Err(e) => Ending::Failed(Error::Runtime(e).to_string()),
                    };
                    link.send(Event::Ended(ending));
                }
            });
        if let Err(e) = started {
            link.send(Event::Ended(Ending::Failed(Error::Runtime(e).to_string())));
        }
        Self {
            commands,
            events,
            terminal,
        }
    }

    /// Sends a command. After the connection ended there is no one to tell, which is harmless.
    /// A connection that never started: it ends at once with `problem` (jump hosts that cannot be read).
    pub fn failed(problem: String, terminal: Arc<Mutex<Terminal>>) -> Self {
        let (commands, _) = unbounded_channel();
        let (event_tx, events) = mpsc::channel();
        // The receiver is held just below, so this cannot fail.
        let _ = event_tx.send(Event::Ended(Ending::Failed(problem)));
        Self {
            commands,
            events,
            terminal,
        }
    }

    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    /// The events since the last call.
    pub fn events(&self) -> impl Iterator<Item = Event> + '_ {
        self.events.try_iter()
    }

    pub fn terminal(&self) -> &Arc<Mutex<Terminal>> {
        &self.terminal
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.send(Command::Disconnect);
    }
}

/// The way back to the UI: events, and a nudge to draw them.
#[derive(Clone)]
struct Link {
    events: mpsc::Sender<Event>,
    repaint: Arc<dyn Fn() + Send + Sync>,
}

impl Link {
    fn send(&self, event: Event) {
        // The UI may be gone while the thread still ends; nothing is waiting for the event then.
        let _ = self.events.send(event);
        (self.repaint)();
    }

    fn log(&self, line: impl Into<String>) {
        self.send(Event::Log(line.into()));
    }
}

/// The commands, shared between the handler (host key questions during key exchange) and the session loop.
type Commands = Arc<tokio::sync::Mutex<UnboundedReceiver<Command>>>;

async fn run(
    target: Target,
    options: Options,
    known_hosts: KnownHosts,
    terminal: Arc<Mutex<Terminal>>,
    commands: UnboundedReceiver<Command>,
    link: Link,
) -> Ending {
    let commands: Commands = Arc::new(tokio::sync::Mutex::new(commands));
    match session(&target, &options, known_hosts, &terminal, &commands, &link).await {
        Ok(ending) => ending,
        Err(Error::Cancelled) => Ending::Disconnected,
        Err(e @ (Error::HostKeyRejected | Error::HostKeyRevoked | Error::SignOnRefused(_))) => {
            Ending::Refused(e.to_string())
        }
        Err(e) => Ending::Failed(e.to_string()),
    }
}

#[expect(
    clippy::significant_drop_tightening,
    reason = "the commands are held for the whole session on purpose"
)]
async fn session(
    target: &Target,
    options: &Options,
    known_hosts: KnownHosts,
    terminal: &Arc<Mutex<Terminal>>,
    commands: &Commands,
    link: &Link,
) -> Result<Ending, Error> {
    // Each jump host is reached through the one before and kept open while the session runs (R-SSH-43).
    let mut jumps: Vec<Through> = Vec::new();
    for jump in &options.jumps {
        let hop = Options {
            key_files: jump.key_files.clone(),
            agent: options.agent.clone(),
            agent_key: None,
            agent_forwarding: false,
            jumps: Vec::new(),
            transport: options.transport.clone(),
            shell: Shell::default(),
        };
        let handle = hop_connect(
            &jump.target,
            &hop,
            jumps.last(),
            (Hop::Jump, jobs::AgentGate::default()),
            known_hosts.clone(),
            commands,
            link,
        )
        .await?;
        jumps.push((handle, jump.target.label()));
    }
    let gate = jobs::AgentGate::new(options.agent_forwarding);
    let handle = hop_connect(
        target,
        options,
        jumps.last(),
        (Hop::Target, gate.clone()),
        known_hosts,
        commands,
        link,
    )
    .await?;

    let (columns, rows) = {
        let t = lock(terminal);
        (t.columns(), t.rows())
    };
    let mut channel = handle.channel_open_session().await?;
    start_shell(&mut channel, options, (columns, rows), link).await?;

    let mut started = false;
    let mut ending = Ending::Closed;
    let mut side = jobs::Jobs::default();
    // The handler no longer needs the commands once signed on: the session loop keeps them to the end.
    let mut commands = commands.lock().await;
    loop {
        tokio::select! {
            message = channel.wait() => match message {
                Some(ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. }) => {
                    let replies = {
                        let mut t = lock(terminal);
                        t.feed(&data);
                        t.take_replies()
                    };
                    if !replies.is_empty() {
                        channel.data_bytes(replies).await?;
                    }
                    (link.repaint)();
                }
                Some(ChannelMsg::Success) if !started => {
                    started = true;
                    link.send(Event::Started);
                }
                Some(ChannelMsg::Failure) if !started => return Err(Error::ShellRefused),
                Some(ChannelMsg::ExitStatus { exit_status }) => ending = Ending::Exited(exit_status),
                Some(ChannelMsg::ExitSignal { signal_name, .. }) => ending = Ending::Signal(format!("{signal_name:?}")),
                Some(ChannelMsg::Close) | None => break,
                Some(_) => {}
            },
            command = commands.recv() => match command {
                Some(Command::Input(bytes)) => channel.data_bytes(bytes).await?,
                Some(Command::Resize { columns, rows }) => channel.window_change(cells(columns), cells(rows), 0, 0).await?,
                Some(Command::Disconnect) | None => {
                    // Leaving anyway: a failure to say goodbye changes nothing.
                    let _ = handle.disconnect(russh::Disconnect::ByApplication, "", "en").await;
                    return Ok(Ending::Disconnected);
                }
                Some(Command::Exec(exec)) => side.start(&handle, exec, &gate, options.agent.is_some(), link).await,
                Some(Command::EndJob(id)) => side.end(id),
                Some(Command::HostKey(_) | Command::Answers(_)) => {}
            },
        }
    }
    Ok(ending)
}

/// A jump host signed on to, with its `user@host` for messages.
type Through = (ssh::Handle<client::Client>, String);

/// Whether a hop is a jump host or the target: only the target's algorithms go to the status bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hop {
    Jump,
    Target,
}

/// Connects to `target`, directly or through the jump host `through` (`direct-tcpip`, RFC 4254 §7.2), checks
/// its host key and signs on.
async fn hop_connect(
    target: &Target,
    options: &Options,
    through: Option<&Through>,
    (hop, gate): (Hop, jobs::AgentGate),
    known_hosts: KnownHosts,
    commands: &Commands,
    link: &Link,
) -> Result<ssh::Handle<client::Client>, Error> {
    let via = through.map(|(_, label)| format!(" through {label}"));
    link.log(format!(
        "Connecting to {} port {}{}",
        target.host,
        target.port,
        via.unwrap_or_default()
    ));
    let handler = client::Client::new(
        target,
        known_hosts,
        (options.agent.clone(), gate),
        hop,
        Arc::clone(commands),
        link.clone(),
    );
    let config = Arc::new(client::config(&options.transport));
    let connecting = async {
        match through {
            None => {
                let transport = &options.transport;
                let stream =
                    transport::open(target, transport.family, &transport.proxy, commands, link)
                        .await?;
                ssh::connect_stream(config, stream, handler).await
            }
            Some((jump, label)) => {
                let channel = jump
                    .channel_open_direct_tcpip(
                        target.host.as_str(),
                        u32::from(target.port),
                        "127.0.0.1",
                        0,
                    )
                    .await
                    .map_err(|e| Error::JumpRefused {
                        jump: label.clone(),
                        host: target.host.clone(),
                        port: target.port,
                        reason: e.to_string(),
                    })?;
                ssh::connect_stream(config, channel.into_stream(), handler).await
            }
        }
    };
    let mut handle = match tokio::time::timeout(CONNECT_TIMEOUT, connecting).await {
        Ok(result) => result?,
        Err(_) => {
            return Err(Error::Timeout(
                target.host.clone(),
                CONNECT_TIMEOUT.as_secs(),
            ));
        }
    };
    // A session without a user asks for one once the host is known, as PuTTY's "login as:" (R-SSH-62).
    let asked;
    let target = if target.user.is_empty() {
        let user = sign_on::ask_user(target, commands, link).await?;
        if hop == Hop::Target {
            link.send(Event::User(user.clone()));
        }
        asked = Target {
            user,
            ..target.clone()
        };
        &asked
    } else {
        target
    };
    sign_on::sign_on(&mut handle, target, options, commands, link).await?;
    Ok(handle)
}

/// The pseudo-terminal, agent forwarding, the environment, then the shell or the command (R-SSH-5, R-SSH-13,
/// R-SSH-28).
async fn start_shell(
    channel: &mut russh::Channel<ssh::Msg>,
    options: &Options,
    (columns, rows): (usize, usize),
    link: &Link,
) -> Result<(), Error> {
    let modes = [(Pty::IUTF8, 1)];
    channel
        .request_pty(
            false,
            &options.shell.terminal_type,
            cells(columns),
            cells(rows),
            0,
            0,
            &modes,
        )
        .await?;
    if options.agent.is_some() && options.agent_forwarding {
        channel.agent_forward(false).await?;
        link.log("Agent forwarding requested");
    }
    send_environment(channel, &options.shell.env, link).await?;
    if let Some(command) = &options.shell.command {
        channel.exec(true, command.as_bytes()).await?;
        link.log(format!("Command requested: {command}"));
    } else {
        channel.request_shell(true).await?;
        link.log("Shell requested");
    }
    Ok(())
}

/// Sends each environment variable and waits for the server's answer, saying a refusal in the event log
/// (RFC 4254 §6.4).
async fn send_environment(
    channel: &mut russh::Channel<ssh::Msg>,
    env: &[(String, String)],
    link: &Link,
) -> Result<(), Error> {
    for (name, value) in env {
        channel.set_env(true, name.as_str(), value.as_str()).await?;
        let answer = async {
            loop {
                match channel.wait().await {
                    Some(ChannelMsg::Success) => return Some(true),
                    Some(ChannelMsg::Failure) => return Some(false),
                    Some(ChannelMsg::Close) | None => return None,
                    Some(_) => {}
                }
            }
        };
        match tokio::time::timeout(ENV_ANSWER, answer).await {
            Ok(Some(true)) => link.log(format!("Environment variable {name} set")),
            Ok(Some(false)) => link.log(format!(
                "The server refused the environment variable {name} (OpenSSH's AcceptEnv decides)"
            )),
            Ok(None) => return Err(Error::ShellRefused),
            Err(_) => link.log(format!(
                "The server did not answer for the environment variable {name}"
            )),
        }
    }
    Ok(())
}

/// A size in cells as the protocol's `uint32`.
fn cells(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The terminal, also after a panic elsewhere left the lock poisoned: its data is still a screen.
pub fn lock(terminal: &Mutex<Terminal>) -> std::sync::MutexGuard<'_, Terminal> {
    terminal.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Waits for the next command of one of the wanted kinds; `Disconnect` or a closed channel cancels.
async fn wait_for<T>(
    commands: &Commands,
    mut wanted: impl FnMut(Command) -> Option<T>,
) -> Result<T, Error> {
    let mut commands = commands.lock().await;
    loop {
        match commands.recv().await {
            Some(Command::Disconnect) | None => return Err(Error::Cancelled),
            Some(command) => {
                if let Some(answer) = wanted(command) {
                    return Ok(answer);
                }
            }
        }
    }
}
