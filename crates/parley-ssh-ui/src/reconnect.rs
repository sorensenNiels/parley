//! Reconnecting by itself (R-SSH-51): when a connection whose shell ran is lost, tries again after 1, 2, 4, 8 …
//! seconds, at most 30 apart, until the time the settings give runs out. Pure: times are the UI's clock in seconds.

use parley_ssh_core::net::Ending;

/// The longest wait between two tries, in seconds.
const MAX_DELAY: f64 = 30.0;

/// Reconnecting, between tries.
#[derive(Debug, Clone, PartialEq)]
pub struct Reconnect {
    /// Tries made so far.
    pub attempt: u32,
    /// When the next try starts.
    pub next: f64,
    /// When trying stops.
    until: f64,
}

/// Whether an ending is a lost connection to come back from: the network or the server went away while the shell
/// ran. Not an exit, the user's disconnect, nor a refusal.
pub fn is_lost(ending: &Ending, started: bool) -> bool {
    match ending {
        Ending::Failed(_) => started,
        Ending::Exited(_)
        | Ending::Signal(_)
        | Ending::Closed
        | Ending::Disconnected
        | Ending::Refused(_) => false,
    }
}

/// The wait before try `attempt` (from 0).
fn delay(attempt: u32) -> f64 {
    2_f64
        .powi(i32::try_from(attempt.min(10)).unwrap_or(10))
        .min(MAX_DELAY)
}

impl Reconnect {
    /// After a connection was lost at `now`, trying for `minutes`.
    pub fn start(now: f64, minutes: u8) -> Self {
        Self {
            attempt: 0,
            next: now + delay(0),
            until: now + f64::from(minutes) * 60.0,
        }
    }

    /// A try failed at `now`: when the next one is, or `false` when time is up.
    pub fn failed(&mut self, now: f64) -> bool {
        self.attempt += 1;
        self.next = now + delay(self.attempt);
        self.next <= self.until
    }

    pub fn due(&self, now: f64) -> bool {
        now >= self.next
    }

    /// Whole seconds until the next try, rounded up.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "at most 30 seconds, and never negative"
    )]
    pub fn seconds_left(&self, now: f64) -> u64 {
        (self.next - now).max(0.0).ceil() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_connection_lost_while_its_shell_ran_comes_back_r_ssh_51() {
        assert!(is_lost(&Ending::Failed("reset".into()), true));
        assert!(
            !is_lost(&Ending::Failed("no route".into()), false),
            "never started"
        );
        for ending in [
            Ending::Exited(0),
            Ending::Signal("HUP".into()),
            Ending::Closed,
            Ending::Disconnected,
            Ending::Refused("sign-on".into()),
        ] {
            assert!(!is_lost(&ending, true), "{ending:?}");
        }
    }

    #[test]
    fn tries_wait_longer_each_time_up_to_thirty_seconds_until_time_is_up_r_ssh_51() {
        let mut r = Reconnect::start(100.0, 1);
        assert_eq!(r.next, 101.0);
        assert!(!r.due(100.5) && r.due(101.0));
        let mut waits = Vec::new();
        let mut now = 101.0;
        while r.failed(now) {
            waits.push(r.next - now);
            now = r.next;
        }
        assert_eq!(
            waits,
            [2.0, 4.0, 8.0, 16.0],
            "a minute holds 1+2+4+8+16 s, not 30 more"
        );
        let mut long = Reconnect::start(0.0, 10);
        for _ in 0..8 {
            assert!(long.failed(long.next));
        }
        assert_eq!(long.next - long.next.floor(), 0.0);
        assert_eq!(delay(8), 30.0);
        assert_eq!(long.seconds_left(long.next - 2.5), 3);
    }
}
