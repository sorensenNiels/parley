//! Connecting again when the connection drops (R-NET-10): when to try, and how long to wait.

use std::time::{Duration, Instant};

/// The waits before each try, in seconds; the last repeats.
const WAITS: [u64; 6] = [2, 4, 8, 16, 32, 60];

fn wait(attempt: usize) -> Duration {
    let last = WAITS.len() - 1;
    Duration::from_secs(WAITS.get(attempt).copied().unwrap_or(WAITS[last]))
}

/// Where reconnecting is: waiting for the next try, or trying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reconnect {
    Waiting { attempt: usize, at: Instant },
    Trying { attempt: usize },
}

impl Reconnect {
    /// The connection dropped: the first try comes after the first wait.
    pub fn after_drop(now: Instant) -> Self {
        Self::Waiting {
            attempt: 0,
            at: now + wait(0),
        }
    }

    /// A try failed: wait longer before the next one.
    #[must_use]
    pub fn after_failure(self, now: Instant) -> Self {
        let next = match self {
            Self::Waiting { attempt, .. } | Self::Trying { attempt } => attempt + 1,
        };
        Self::Waiting {
            attempt: next,
            at: now + wait(next),
        }
    }

    /// Time for the next try: what to set while it runs.
    pub fn due(self, now: Instant) -> Option<Self> {
        match self {
            Self::Waiting { attempt, at } if now >= at => Some(Self::Trying { attempt }),
            Self::Waiting { .. } | Self::Trying { .. } => None,
        }
    }

    /// How long until the next try, while waiting for it.
    pub fn left(self, now: Instant) -> Option<Duration> {
        match self {
            Self::Waiting { at, .. } => Some(at.saturating_duration_since(now)),
            Self::Trying { .. } => None,
        }
    }
}

/// "Reconnecting in 8 s", counting whole seconds up so it never says 0 s while waiting.
pub fn countdown(left: Duration) -> String {
    let seconds = left.as_millis().div_ceil(1000).max(1);
    format!("Reconnecting in {seconds} s")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_waits_double_from_2_s_up_to_60_s_r_net_10() {
        let now = Instant::now();
        let mut reconnect = Reconnect::after_drop(now);
        let mut waits = Vec::new();
        for _ in 0..8 {
            waits.push(reconnect.left(now).map(|d| d.as_secs()));
            reconnect = reconnect.after_failure(now);
        }
        let expected = [2, 4, 8, 16, 32, 60, 60, 60].map(Some);
        assert_eq!(waits, expected);
    }

    #[test]
    fn a_try_is_due_only_when_its_wait_is_over_r_net_10() {
        let now = Instant::now();
        let reconnect = Reconnect::after_drop(now);
        assert_eq!(reconnect.due(now + Duration::from_secs(1)), None);
        let trying = reconnect.due(now + Duration::from_secs(2));
        assert_eq!(trying, Some(Reconnect::Trying { attempt: 0 }));
        assert_eq!(trying.and_then(|t| t.left(now)), None);
        assert_eq!(trying.and_then(|t| t.due(now)), None);
    }

    #[test]
    fn the_countdown_rounds_up_to_whole_seconds_r_net_10() {
        assert_eq!(
            countdown(Duration::from_millis(7_200)),
            "Reconnecting in 8 s"
        );
        assert_eq!(countdown(Duration::ZERO), "Reconnecting in 1 s");
    }
}
