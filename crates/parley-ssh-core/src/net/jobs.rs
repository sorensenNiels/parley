//! Commands run on a side channel of an open connection, beside the shell (R-SSH-57, Phase 31): their output
//! goes to the UI as events, not onto the screen. A job may let the server use the agent while it runs
//! (R-SSH-53, D58), whatever the session's forwarding setting.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use russh::{ChannelMsg, client as ssh};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use super::{Event, Link, client};

/// A job's number, chosen by the UI; events about the job carry it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(pub u64);

/// A command to run on a side channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exec {
    pub id: JobId,
    /// Given to the server's shell as it is (RFC 4254 §6.5 `exec`).
    pub command: String,
    /// Forward the agent on this channel, and let the server use it until the job ends (D58).
    pub agent: bool,
}

/// Where a job's output came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Out,
    /// Extended data of type 1 (RFC 4254 §5.2).
    Err,
}

/// How a job ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobEnding {
    /// The command ended with this exit status.
    Exited(u32),
    /// The command was ended by this signal.
    Signal(String),
    /// Ended because the UI asked ([`super::Command::EndJob`]).
    Stopped,
    /// The channel closed without an exit status.
    Closed,
    /// The command never ran, in plain words.
    Refused(String),
}

/// Whether the server may use the agent: always when the session forwards it, else while a job that asked
/// for it runs (D58).
#[derive(Debug, Clone, Default)]
pub(super) struct AgentGate {
    always: bool,
    jobs: Arc<AtomicUsize>,
}

impl AgentGate {
    pub(super) fn new(always: bool) -> Self {
        Self {
            always,
            jobs: Arc::default(),
        }
    }

    pub(super) fn open(&self) -> bool {
        self.always || self.jobs.load(Ordering::SeqCst) > 0
    }

    /// Opens the gate until the guard is dropped.
    fn hold(&self) -> AgentUse {
        self.jobs.fetch_add(1, Ordering::SeqCst);
        AgentUse(Arc::clone(&self.jobs))
    }
}

/// A job's hold on the agent gate.
struct AgentUse(Arc<AtomicUsize>);

impl Drop for AgentUse {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The jobs running on a connection: a way to end each.
#[derive(Default)]
pub(super) struct Jobs(HashMap<JobId, UnboundedSender<()>>);

impl Jobs {
    /// Opens a channel and runs `exec` on it; when that fails, the job ends at once as refused.
    pub(super) async fn start(
        &mut self,
        handle: &ssh::Handle<client::Client>,
        exec: Exec,
        gate: &AgentGate,
        has_agent: bool,
        link: &Link,
    ) {
        self.0.retain(|_, stop| !stop.is_closed());
        let id = exec.id;
        match open(handle, exec, gate, has_agent, link).await {
            Ok(stop) => {
                self.0.insert(id, stop);
            }
            Err(reason) => link.send(Event::JobEnded {
                id,
                ending: JobEnding::Refused(reason),
            }),
        }
    }

    /// Ends a job; one that already ended is no longer there, which is harmless.
    pub(super) fn end(&mut self, id: JobId) {
        if let Some(stop) = self.0.remove(&id) {
            // The job may have ended meanwhile; then there is nothing to stop.
            let _ = stop.send(());
        }
    }
}

/// Opens the channel, asks for the agent when wanted, starts the command and carries the channel on a task of
/// its own.
async fn open(
    handle: &ssh::Handle<client::Client>,
    exec: Exec,
    gate: &AgentGate,
    has_agent: bool,
    link: &Link,
) -> Result<UnboundedSender<()>, String> {
    let channel = handle
        .channel_open_session()
        .await
        .map_err(|e| format!("The server refused a second channel: {e}"))?;
    let hold = if exec.agent {
        if !has_agent {
            return Err("No agent is running to sign on to the other server with".to_owned());
        }
        let hold = gate.hold();
        channel
            .agent_forward(false)
            .await
            .map_err(|e| e.to_string())?;
        Some(hold)
    } else {
        None
    };
    channel
        .exec(true, exec.command.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    link.log(format!("Running on the server: {}", exec.command));
    let (stop, mut stopped) = unbounded_channel();
    let link = link.clone();
    tokio::spawn(async move {
        let ending = carry(channel, exec.id, &mut stopped, &link).await;
        drop(hold);
        link.send(Event::JobEnded {
            id: exec.id,
            ending,
        });
    });
    Ok(stop)
}

/// Passes the channel's output on until it closes or the UI ends the job.
async fn carry(
    mut channel: russh::Channel<ssh::Msg>,
    id: JobId,
    stopped: &mut tokio::sync::mpsc::UnboundedReceiver<()>,
    link: &Link,
) -> JobEnding {
    let mut ending = JobEnding::Closed;
    let mut listening = true;
    loop {
        tokio::select! {
            message = channel.wait() => match message {
                Some(ChannelMsg::Data { data }) => link.send(Event::JobOutput { id, stream: Stream::Out, data: data.to_vec() }),
                Some(ChannelMsg::ExtendedData { data, .. }) => link.send(Event::JobOutput { id, stream: Stream::Err, data: data.to_vec() }),
                Some(ChannelMsg::Failure) => return JobEnding::Refused("The server refused to run the command".to_owned()),
                Some(ChannelMsg::ExitStatus { exit_status }) => ending = JobEnding::Exited(exit_status),
                Some(ChannelMsg::ExitSignal { signal_name, .. }) => ending = JobEnding::Signal(format!("{signal_name:?}")),
                Some(ChannelMsg::Close) | None => return ending,
                Some(_) => {}
            },
            stop = stopped.recv(), if listening => {
                listening = false;
                // `None`: the connection's list of jobs is gone, and nobody asked to stop.
                if stop.is_some() {
                    // Not every server takes signals (OpenSSH does since 7.9); closing ends the command anyway, as
                    // a hang-up. russh forgets a channel it closes, so the server's answer is not waited for.
                    let _ = channel.signal(russh::Sig::TERM).await;
                    let _ = channel.close().await;
                    return JobEnding::Stopped;
                }
            },
        }
    }
}
