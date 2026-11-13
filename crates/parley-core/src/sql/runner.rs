//! The SQL helper on a thread of its own (R-SQL-6), so the SQL window never waits for the host:
//! the window sends requests and reads the answers as they come, and is woken for each.

use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use crate::error::{Error, Result};

use super::helper::{Canceller, Helper, HelperFiles};
use super::protocol::{Answer, Request};

/// What came back for the request sent as `tag`.
#[derive(Debug)]
pub struct Event {
    pub tag: u64,
    pub result: Result<Answer>,
}

/// A helper running requests one after another on its own thread.
#[derive(Debug)]
pub struct Runner {
    jobs: Option<Sender<(u64, Request)>>,
    events: Receiver<Event>,
    canceller: Canceller,
    next_tag: u64,
}

impl Runner {
    /// Starts the helper; `wake` is called after each answer (the UI asks for a frame).
    pub fn start(
        java: &Path,
        files: &HelperFiles,
        wake: impl Fn() + Send + 'static,
    ) -> Result<Self> {
        let mut helper = Helper::start(java, files)?;
        let canceller = helper.canceller();
        let (jobs, inbox) = mpsc::channel::<(u64, Request)>();
        let (outbox, events) = mpsc::channel();
        thread::spawn(move || {
            for (tag, request) in inbox {
                let closing = matches!(request, Request::Close);
                let result = helper.call(&request).and_then(|answer| {
                    answer.ok_or_else(|| Error::SqlHelper("the helper did not answer".to_owned()))
                });
                if outbox.send(Event { tag, result }).is_err() {
                    break;
                }
                wake();
                if closing {
                    break;
                }
            }
            // Dropping the helper ends its process.
        });
        Ok(Self {
            jobs: Some(jobs),
            events,
            canceller,
            next_tag: 1,
        })
    }

    /// Queues `request`; its answer comes as an [`Event`] with the tag returned.
    pub fn send(&mut self, request: Request) -> u64 {
        let tag = self.next_tag;
        self.next_tag += 1;
        if let Some(jobs) = &self.jobs
            && jobs.send((tag, request)).is_err()
        {
            // The thread ended (the helper failed); the next try_event says so.
            self.jobs = None;
        }
        tag
    }

    /// The next answer, if one has come; an error event when the helper's thread is gone.
    pub fn try_event(&self) -> Option<Event> {
        match self.events.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Event {
                tag: 0,
                result: Err(Error::SqlHelper("the SQL helper has stopped".to_owned())),
            }),
        }
    }

    /// Stops the statement running now (R-SQL-4); its request then answers with an error.
    pub fn cancel(&self) {
        self.canceller.cancel();
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        // No more jobs: the thread ends after the one it runs, and the helper with it.
        self.jobs = None;
        self.canceller.cancel();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    use super::*;
    use crate::sql::protocol::Hello;

    #[test]
    #[ignore = "needs Java and the jars from make helper"]
    fn the_runner_answers_on_its_thread_and_wakes_the_window_r_sql_6() {
        let java = crate::sql::java::find(None).expect("make helper needs Java 11");
        let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/helpers");
        let woken = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&woken);
        let mut runner = Runner::start(&java.path, &HelperFiles::in_folder(&folder), move || {
            count.fetch_add(1, Ordering::SeqCst);
        })
        .expect("started");
        let tag = runner.send(Request::Hello);
        let deadline = Instant::now() + Duration::from_secs(30);
        let event = loop {
            if let Some(event) = runner.try_event() {
                break event;
            }
            assert!(Instant::now() < deadline, "no answer in 30 s");
            thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(event.tag, tag);
        let hello: Hello = event
            .result
            .expect("answered")
            .into_result()
            .expect("hello");
        assert_eq!(hello.protocol, crate::sql::protocol::PROTOCOL);
        assert_eq!(woken.load(Ordering::SeqCst), 1);
        runner.cancel(); // nothing runs: harmless
    }
}
