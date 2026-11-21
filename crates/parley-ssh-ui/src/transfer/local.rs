//! Programs run on the desktop for a transfer (R-SSH-55): rsync and its check. Their output and how they ended come
//! back as the same events a session's jobs give, from [`super::DESKTOP`].

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};

use parley_ssh_core::net::{Event, JobEnding, JobId, Stream};

/// The desktop's running programs.
pub struct Locals {
    running: HashMap<JobId, Arc<Mutex<Child>>>,
    /// Those ended on request: they end as stopped.
    ended: Arc<Mutex<HashSet<JobId>>>,
    send: Sender<Event>,
    events: Receiver<Event>,
}

impl Default for Locals {
    fn default() -> Self {
        let (send, events) = mpsc::channel();
        Self {
            running: HashMap::new(),
            ended: Arc::default(),
            send,
            events,
        }
    }
}

impl std::fmt::Debug for Locals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Locals")
            .field("running", &self.running.len())
            .finish_non_exhaustive()
    }
}

impl Locals {
    /// Starts `argv` (its name first) as job `id`; one that cannot start ends at once as refused. `wake` is called
    /// whenever there is news.
    pub fn spawn(&mut self, id: JobId, argv: &[String], wake: impl Fn() + Send + Sync + 'static) {
        self.running.retain(|_, child| {
            child
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .try_wait()
                .is_ok_and(|done| done.is_none())
        });
        let Some((program, rest)) = argv.split_first() else {
            return;
        };
        let mut command = Command::new(program);
        command
            .args(rest)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: rsync is a console program; no console window should flash up.
            command.creation_flags(0x0800_0000);
        }
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                // The window is there to read it: a failure to send means it has closed.
                let _ = self.send.send(Event::JobEnded {
                    id,
                    ending: JobEnding::Refused(format!("{program} could not be started: {e}")),
                });
                wake();
                return;
            }
        };
        let (out, err) = (child.stdout.take(), child.stderr.take());
        let child = Arc::new(Mutex::new(child));
        self.running.insert(id, Arc::clone(&child));
        let wake = Arc::new(wake);
        let readers = [
            out.map(|o| {
                read(
                    id,
                    Stream::Out,
                    Pipe::Out(o),
                    self.send.clone(),
                    Arc::clone(&wake),
                )
            }),
            err.map(|e| {
                read(
                    id,
                    Stream::Err,
                    Pipe::Err(e),
                    self.send.clone(),
                    Arc::clone(&wake),
                )
            }),
        ];
        let (send, ended) = (self.send.clone(), Arc::clone(&self.ended));
        std::thread::spawn(move || {
            for reader in readers.into_iter().flatten() {
                // A reader that panicked has nothing more to say.
                let _ = reader.join();
            }
            let status = child.lock().unwrap_or_else(PoisonError::into_inner).wait();
            let stopped = ended
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&id);
            let ending = match status {
                _ if stopped => JobEnding::Stopped,
                Ok(status) => status.code().map_or_else(
                    || JobEnding::Signal("a signal".to_owned()),
                    |code| JobEnding::Exited(u32::from_ne_bytes(code.to_ne_bytes())),
                ),
                Err(e) => JobEnding::Refused(e.to_string()),
            };
            let _ = send.send(Event::JobEnded { id, ending });
            wake();
        });
    }

    /// Ends job `id`; one that has ended already is not there, which is harmless.
    pub fn end(&mut self, id: JobId) {
        if let Some(child) = self.running.remove(&id) {
            self.ended
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(id);
            // It may have ended meanwhile; then there is nothing to end.
            let _ = child.lock().unwrap_or_else(PoisonError::into_inner).kill();
        }
    }

    /// What the programs said since the last call.
    pub fn take(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}

enum Pipe {
    Out(ChildStdout),
    Err(ChildStderr),
}

fn read(
    id: JobId,
    stream: Stream,
    mut pipe: Pipe,
    send: Sender<Event>,
    wake: Arc<impl Fn() + Send + Sync + 'static>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        loop {
            let read = match &mut pipe {
                Pipe::Out(o) => o.read(&mut buffer),
                Pipe::Err(e) => e.read(&mut buffer),
            };
            match read {
                Ok(0) | Err(_) => return,
                Ok(n) => {
                    let data = buffer[..n].to_vec();
                    if send.send(Event::JobOutput { id, stream, data }).is_err() {
                        return;
                    }
                    wake();
                }
            }
        }
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn until_ended(locals: &Locals, id: JobId) -> (String, JobEnding) {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut out = String::new();
        loop {
            for event in locals.take() {
                match event {
                    Event::JobOutput { id: of, data, .. } if of == id => {
                        out.push_str(&String::from_utf8_lossy(&data));
                    }
                    Event::JobEnded { id: of, ending } if of == id => return (out, ending),
                    _ => {}
                }
            }
            assert!(Instant::now() < deadline, "no end within ten seconds");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn a_program_on_the_desktop_reports_its_output_and_exit_status_r_ssh_55() {
        let mut locals = Locals::default();
        locals.spawn(
            JobId(1),
            &argv(&["sh", "-c", "printf 'æøå'; exit 3"]),
            || {},
        );
        assert_eq!(
            until_ended(&locals, JobId(1)),
            ("æøå".to_owned(), JobEnding::Exited(3))
        );
    }

    #[test]
    fn a_missing_program_is_refused_and_an_ended_one_stopped_r_ssh_55() {
        let mut locals = Locals::default();
        locals.spawn(JobId(2), &argv(&["/no/such/rsync", "--version"]), || {});
        assert!(
            matches!(until_ended(&locals, JobId(2)).1, JobEnding::Refused(why) if why.contains("/no/such/rsync"))
        );
        locals.spawn(JobId(3), &argv(&["sleep", "30"]), || {});
        locals.end(JobId(3));
        assert_eq!(until_ended(&locals, JobId(3)).1, JobEnding::Stopped);
    }
}
