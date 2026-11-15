//! Running the SQL helper (R-SQL-11): a Java process Parley starts for an SQL window, with
//! requests on its stdin and answers on its stdout ([`super::protocol`]). The password goes
//! only through stdin. Closing the helper, or dropping it, ends the process.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{Error, Result};

use super::protocol::{Answer, Request};

/// The jars the helper runs from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperFiles {
    /// Parley's helper, `parley-sql.jar`.
    pub helper: PathBuf,
    /// `JTOpen`, `jt400.jar` (R-SQL-13).
    pub jtopen: PathBuf,
}

impl HelperFiles {
    /// The jars in `folder`, as the build and the installers lay them out.
    pub fn in_folder(folder: &Path) -> Self {
        Self {
            helper: folder.join("parley-sql.jar"),
            jtopen: folder.join("jt400.jar"),
        }
    }

    fn class_path(&self) -> Result<std::ffi::OsString> {
        for jar in [&self.helper, &self.jtopen] {
            if !jar.is_file() {
                return Err(Error::SqlHelper(format!(
                    "{} is missing; Parley's installation is not complete",
                    jar.display()
                )));
            }
        }
        std::env::join_paths([&self.helper, &self.jtopen])
            .map_err(|e| Error::SqlHelper(format!("the helper's class path: {e}")))
    }
}

/// How long the helper may take to answer before Parley gives up on it. Long: a query or a
/// connection to a slow host may take a while, and Cancel is there for the operator.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(600);

/// A running helper.
#[derive(Debug)]
pub struct Helper {
    child: Child,
    /// Shared with [`Canceller`]s, which write while a call waits; `None` once closed.
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    answers: Receiver<std::io::Result<String>>,
    /// What the helper wrote to stderr, shown only when it fails.
    stderr: Arc<Mutex<String>>,
    /// Told when stderr has been read to its end, so a helper that stopped is named with all
    /// it said.
    stderr_read: Receiver<()>,
    next_id: u64,
}

impl Helper {
    /// Starts the helper with `java`.
    pub fn start(java: &Path, files: &HelperFiles) -> Result<Self> {
        let mut child = Command::new(java)
            .arg("-cp")
            .arg(files.class_path()?)
            .arg("parley.sql.Helper")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::SqlHelper(format!("cannot start {}: {e}", java.display())))?;
        let stdout = child.stdout.take().expect("stdout was piped above");
        let stderr = child.stderr.take().expect("stderr was piped above");
        let (send, answers) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if send.send(line).is_err() {
                    return;
                }
            }
        });
        let errors = Arc::new(Mutex::new(String::new()));
        let kept = Arc::clone(&errors);
        let (read, stderr_read) = mpsc::channel();
        thread::spawn(move || {
            let mut text = String::new();
            // Whatever could be read is kept; a broken pipe only ends the reading.
            let _ = BufReader::new(stderr)
                .take(64 * 1024)
                .read_to_string(&mut text);
            kept.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push_str(&text);
            // Nobody waits once the helper is gone; that is fine.
            let _ = read.send(());
        });
        Ok(Self {
            stdin: Arc::new(Mutex::new(child.stdin.take())),
            child,
            answers,
            stderr: errors,
            stderr_read,
            next_id: 1,
        })
    }

    /// Sends `request` and waits for its answer; for [`Request::Cancel`], only sends.
    pub fn call(&mut self, request: &Request) -> Result<Option<Answer>> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_line(&request.line(id))?;
        if !request.answered() {
            return Ok(None);
        }
        receive(&self.answers, id, ANSWER_TIMEOUT)
            .map(Some)
            .map_err(|no_answer| {
                if no_answer == NoAnswer::Stopped {
                    // Its stdout ended before its stderr was read: wait a little for the
                    // reason. A timeout leaves the error without it, which is all it costs.
                    let _ = self.stderr_read.recv_timeout(Duration::from_secs(2));
                }
                self.failed(&no_answer.to_string())
            })
    }

    fn send_line(&self, line: &str) -> Result<()> {
        write_line(&self.stdin, line)
            .map_err(|e| self.failed(&format!("it cannot be written to: {e}")))
    }

    /// A handle that cancels the statement running, from another thread (R-SQL-4).
    pub fn canceller(&self) -> Canceller {
        Canceller(Arc::clone(&self.stdin))
    }

    /// An error for a helper that failed, with what it wrote to stderr.
    fn failed(&self, what: &str) -> Error {
        let detail = {
            let stderr = self.stderr.lock().unwrap_or_else(PoisonError::into_inner);
            stderr.lines().take(5).collect::<Vec<_>>().join(" / ")
        };
        if detail.is_empty() {
            Error::SqlHelper(format!("the SQL helper failed: {what}"))
        } else {
            Error::SqlHelper(format!("the SQL helper failed: {what} ({detail})"))
        }
    }

    /// Asks the helper to close the connection and end, then waits for it.
    pub fn close(mut self) {
        if self.call(&Request::Close).is_err() {
            // It is ending or gone either way; drop makes sure.
        }
    }
}

/// Why a call got no answer.
#[derive(Debug, PartialEq, Eq)]
enum NoAnswer {
    /// Reading the helper's stdout failed.
    Unreadable(String),
    Timeout,
    Stopped,
}

impl std::fmt::Display for NoAnswer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable(e) => write!(f, "its answer could not be read: {e}"),
            Self::Timeout => f.write_str("it did not answer in time"),
            Self::Stopped => f.write_str("it stopped"),
        }
    }
}

/// Waits on the helper's stdout `answers` for the answer to request `id`. An answer with
/// another id is to an earlier call that gave up waiting, and is passed over. An answer without
/// one is the helper saying it could not read a line; only this call's line is waiting, so it
/// is this call's answer. A line that is not an answer (something the JVM or a library printed
/// to stdout) is passed over too, not taken for the helper failing.
fn receive(
    answers: &Receiver<std::io::Result<String>>,
    id: u64,
    timeout: Duration,
) -> std::result::Result<Answer, NoAnswer> {
    let deadline = Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let line = match answers.recv_timeout(left) {
            Ok(Ok(line)) => line,
            Ok(Err(e)) => return Err(NoAnswer::Unreadable(e.to_string())),
            Err(RecvTimeoutError::Timeout) => return Err(NoAnswer::Timeout),
            Err(RecvTimeoutError::Disconnected) => return Err(NoAnswer::Stopped),
        };
        let Ok(answer) = Answer::parse(&line) else {
            continue;
        };
        if answer.id.is_none_or(|answered| answered == id) {
            return Ok(answer);
        }
    }
}

/// Writes `line` and a newline to the helper, whole: the lock keeps lines from mixing.
fn write_line(stdin: &Mutex<Option<ChildStdin>>, line: &str) -> std::io::Result<()> {
    let mut guard = stdin.lock().unwrap_or_else(PoisonError::into_inner);
    let written = match guard.as_mut() {
        Some(pipe) => pipe
            .write_all(line.as_bytes())
            .and_then(|()| pipe.write_all(b"\n"))
            .and_then(|()| pipe.flush()),
        None => Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "the helper is closed",
        )),
    };
    // Held until the whole line is written, so a cancel never lands inside a request.
    drop(guard);
    written
}

/// Cancels the statement a [`Helper`] is running, from any thread.
#[derive(Debug, Clone)]
pub struct Canceller(Arc<Mutex<Option<ChildStdin>>>);

impl Canceller {
    pub fn cancel(&self) {
        // A helper that is gone has nothing to cancel.
        let _ = write_line(&self.0, &Request::Cancel.line(0));
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        // Closing stdin ends the helper (it cancels what runs); kill it if it lingers.
        *self.stdin.lock().unwrap_or_else(PoisonError::into_inner) = None;
        for _ in 0..50 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        // Killing a process that already ended is harmless.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Hello;

    /// The jars `make helper` builds, and a Java to run them; run by `make helper` with
    /// `--ignored`, since they need a JDK.
    fn built() -> (crate::java::Java, HelperFiles) {
        let java = crate::java::find(None).expect("make helper needs Java 11");
        let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/helpers");
        (java, HelperFiles::in_folder(&folder))
    }

    #[test]
    #[ignore = "needs Java and the jars from make helper"]
    fn the_built_helper_says_hello_in_this_protocol_r_sql_11() {
        let (java, files) = built();
        let mut helper = Helper::start(&java.path, &files).expect("started");
        let hello: Hello = helper
            .call(&Request::Hello)
            .expect("answered")
            .expect("an answer")
            .into_result()
            .expect("hello");
        assert_eq!(hello.protocol, super::super::protocol::PROTOCOL);
        assert!(hello.jtopen.contains("JTOpen"), "{}", hello.jtopen);
        helper.close();
    }

    #[test]
    #[ignore = "needs Java and the jars from make helper"]
    fn a_request_before_connecting_is_an_sql_error_not_a_failure_r_sql_11() {
        let (java, files) = built();
        let mut helper = Helper::start(&java.path, &files).expect("started");
        let run = Request::Run {
            sql: "VALUES 1".to_owned(),
            max_rows: 10,
            timeout: 0,
            params: Vec::new(),
        };
        let answer = helper.call(&run).expect("answered").expect("an answer");
        assert!(answer.outcome.is_err());
    }

    /// Sends `lines` as the helper's stdout and ends it.
    fn stdout(lines: &[&str]) -> Receiver<std::io::Result<String>> {
        let (send, answers) = mpsc::channel();
        for line in lines {
            send.send(Ok((*line).to_owned()))
                .expect("the receiver is alive");
        }
        answers
    }

    #[test]
    fn an_answer_without_an_id_is_the_answer_to_the_call_waiting_r_sql_11() {
        let answers = stdout(&[
            r#"{"id":null,"error":{"message":"invalid JSON at character 0","sqlstate":null,"sqlcode":0}}"#,
        ]);
        let answer = receive(&answers, 7, Duration::from_secs(5)).expect("answered at once");
        assert_eq!(answer.id, None);
        assert!(answer.outcome.is_err());
    }

    #[test]
    fn stray_lines_and_earlier_answers_are_passed_over_r_sql_11() {
        let answers = stdout(&[
            "Picked up JAVA_TOOL_OPTIONS: -Dfile.encoding=UTF-8",
            "",
            r#"{"id":6,"ok":{"kind":"count","count":1}}"#,
            "æøå {",
            r#"{"id":7,"ok":{"kind":"count","count":2}}"#,
        ]);
        let answer = receive(&answers, 7, Duration::from_secs(5)).expect("answered");
        assert_eq!(answer.id, Some(7));
        assert_eq!(
            receive(&answers, 8, Duration::from_secs(5)),
            Err(NoAnswer::Stopped)
        );
    }

    #[test]
    fn a_helper_that_says_nothing_times_out_and_one_that_ends_has_stopped_r_sql_11() {
        let (send, answers) = mpsc::channel();
        assert_eq!(
            receive(&answers, 1, Duration::from_millis(20)),
            Err(NoAnswer::Timeout)
        );
        send.send(Err(std::io::Error::other("bad pipe")))
            .expect("the receiver is alive");
        assert!(matches!(
            receive(&answers, 1, Duration::from_secs(5)),
            Err(NoAnswer::Unreadable(_))
        ));
        drop(send);
        assert_eq!(
            receive(&answers, 1, Duration::from_secs(5)),
            Err(NoAnswer::Stopped)
        );
    }

    /// A stand-in for Java (R-SQL-11): `script`, run by `sh`, as `java -cp … parley.sql.Helper`,
    /// with empty jars beside it, in a folder of its own named `name`.
    #[cfg(unix)]
    fn fake_helper(name: &str, script: &str) -> (std::path::PathBuf, HelperFiles) {
        use std::os::unix::fs::PermissionsExt;
        let folder =
            std::env::temp_dir().join(format!("parley-sql-core-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("a temporary folder");
        let java = folder.join("java");
        std::fs::write(&java, format!("#!/bin/sh\n{script}\n")).expect("the script");
        std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o755))
            .expect("made runnable");
        let files = HelperFiles::in_folder(&folder);
        for jar in [&files.helper, &files.jtopen] {
            std::fs::write(jar, b"").expect("an empty jar");
        }
        (java, files)
    }

    #[cfg(unix)]
    #[test]
    fn a_helper_that_ends_mid_call_is_an_error_naming_what_it_wrote_to_stderr_r_sql_11() {
        let (java, files) = fake_helper(
            "ends",
            "read line\necho 'Exception in thread main: æøå broke' >&2\nexit 3",
        );
        let mut helper = Helper::start(&java, &files).expect("started");
        let error = helper.call(&Request::Hello).expect_err("it stopped");
        let said = error.to_string();
        assert!(said.contains("it stopped"), "{said}");
        assert!(said.contains("æøå broke"), "{said}");
        drop(helper);
        std::fs::remove_dir_all(java.parent().expect("its folder")).ok();
    }

    #[cfg(unix)]
    #[test]
    fn dropping_the_helper_ends_its_process_r_sql_11() {
        let (java, files) = fake_helper(
            "drop",
            "while read line; do echo '{\"id\":1,\"ok\":{\"protocol\":0,\"java\":\"sh\",\"jtopen\":\"none\"}}'; done",
        );
        let mut helper = Helper::start(&java, &files).expect("started");
        let answer = helper.call(&Request::Hello).expect("answered");
        assert_eq!(answer.and_then(|a| a.id), Some(1));
        let pid = helper.child.id().to_string();
        drop(helper);
        let alive = Command::new("kill")
            .args(["-0", &pid])
            .stderr(Stdio::null())
            .status()
            .expect("kill runs");
        assert!(!alive.success(), "process {pid} still runs");
        std::fs::remove_dir_all(java.parent().expect("its folder")).ok();
    }

    #[test]
    fn missing_jars_are_named_r_sql_11() {
        let files = HelperFiles::in_folder(Path::new("/no/such/folder"));
        let error = files.class_path().expect_err("missing");
        assert!(error.to_string().contains("parley-sql.jar"), "{error}");
    }
}
