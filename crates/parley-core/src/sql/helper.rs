//! Running the SQL helper (R-SQL-11): a Java process Parley starts for an SQL window, with
//! requests on its stdin and answers on its stdout ([`super::protocol`]). The password goes
//! only through stdin. Closing the helper, or dropping it, ends the process.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

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
        thread::spawn(move || {
            let mut text = String::new();
            // Whatever could be read is kept; a broken pipe only ends the reading.
            let _ = BufReader::new(stderr)
                .take(64 * 1024)
                .read_to_string(&mut text);
            kept.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push_str(&text);
        });
        Ok(Self {
            stdin: Arc::new(Mutex::new(child.stdin.take())),
            child,
            answers,
            stderr: errors,
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
        loop {
            let line = match self.answers.recv_timeout(ANSWER_TIMEOUT) {
                Ok(Ok(line)) => line,
                Ok(Err(e)) => {
                    return Err(self.failed(&format!("its answer could not be read: {e}")));
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err(self.failed("it did not answer in time"));
                }
                Err(RecvTimeoutError::Disconnected) => return Err(self.failed("it stopped")),
            };
            let answer = Answer::parse(&line)?;
            // An answer to an unreadable line has no id; ours always has one.
            if answer.id == Some(id) {
                return Ok(Some(answer));
            }
        }
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
    use crate::sql::protocol::Hello;

    /// The jars `make helper` builds, and a Java to run them; run by `make helper` with
    /// `--ignored`, since they need a JDK.
    fn built() -> (crate::sql::java::Java, HelperFiles) {
        let java = crate::sql::java::find(None).expect("make helper needs Java 11");
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
        };
        let answer = helper.call(&run).expect("answered").expect("an answer");
        assert!(answer.outcome.is_err());
    }

    #[test]
    fn missing_jars_are_named_r_sql_11() {
        let files = HelperFiles::in_folder(Path::new("/no/such/folder"));
        let error = files.class_path().expect_err("missing");
        assert!(error.to_string().contains("parley-sql.jar"), "{error}");
    }
}
