//! Whether a new version is out (R-UI-11, D13), as in Tortuga: checked in the background
//! at start-up unless turned off, and from Settings → About; shown as a quiet link in the
//! menu bar. The `curl` command that downloads a package is on the About page (R-UI-12).

use std::sync::{Arc, Mutex, PoisonError};

use parley_core::update::{Found, Release};

/// Asks for the latest release: GitHub in the app, a fake in tests. Blocks: it runs on a
/// thread. Errors are display text.
pub type Checker = Arc<dyn Fn() -> Result<Found, String> + Send + Sync>;

/// What the last check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    /// No checker: this build doesn't look for new versions.
    Unsupported,
    NotChecked,
    Checking,
    UpToDate,
    Available(Release),
    Failed(String),
}

/// What started a check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    /// At start-up: a failure says nothing.
    AtStart,
    /// "Check now": the About page says what happened.
    Now,
}

/// A finished check: what started it and what it found.
type Finished = (Check, Result<Found, String>);

/// The last check, and where a running one's result arrives.
pub struct Updates {
    status: UpdateStatus,
    checker: Option<Checker>,
    result: Arc<Mutex<Option<Finished>>>,
}

impl std::fmt::Debug for Updates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Updates")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

impl Default for Updates {
    fn default() -> Self {
        Self {
            status: UpdateStatus::Unsupported,
            checker: None,
            result: Arc::default(),
        }
    }
}

impl Updates {
    pub fn set_checker(&mut self, checker: Checker) {
        self.checker = Some(checker);
        self.status = UpdateStatus::NotChecked;
    }

    pub const fn status(&self) -> &UpdateStatus {
        &self.status
    }

    /// The newer release, if one is out.
    pub const fn available(&self) -> Option<&Release> {
        match &self.status {
            UpdateStatus::Available(release) => Some(release),
            _ => None,
        }
    }

    /// Starts a check on a thread, unless one is running or there is no checker.
    pub fn start(&mut self, ctx: &egui::Context, check: Check) {
        let Some(checker) = self.checker.clone() else {
            return;
        };
        if self.status == UpdateStatus::Checking {
            return;
        }
        self.status = UpdateStatus::Checking;
        let result = Arc::clone(&self.result);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let found = checker();
            *result.lock().unwrap_or_else(PoisonError::into_inner) = Some((check, found));
            ctx.request_repaint();
        });
    }

    /// Takes the result of a check that finished.
    pub fn collect(&mut self) {
        let finished = self
            .result
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some((check, result)) = finished {
            self.status = match (result, check) {
                (Ok(Found::Newer(release)), _) => UpdateStatus::Available(release),
                (Ok(Found::UpToDate), _) => UpdateStatus::UpToDate,
                (Err(e), Check::Now) => UpdateStatus::Failed(e),
                (Err(_), Check::AtStart) => UpdateStatus::NotChecked,
            };
        }
    }
}
