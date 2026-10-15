//! Delay after repeated failed unlocks. Kept in memory only: it's a speed bump
//! for someone guessing at the keyboard, not the real defence (Argon2's cost is).
//! Nothing is ever wiped, however many attempts fail.

use std::time::Duration;

/// Failures allowed before any delay.
pub const FREE_ATTEMPTS: u32 = 5;
/// Longest delay.
pub const MAX_DELAY: Duration = Duration::from_secs(60);

/// Counts failed unlock attempts since the last success.
#[derive(Debug, Default, Clone)]
pub struct Backoff {
    failures: u32,
}

impl Backoff {
    /// Record a failure and return how long to wait before reporting it:
    /// nothing for the first five, then 1 s, 2 s, 4 s ... up to 60 s.
    pub fn record_failure(&mut self) -> Duration {
        self.failures = self.failures.saturating_add(1);
        if self.failures <= FREE_ATTEMPTS {
            return Duration::ZERO;
        }
        let exponent = (self.failures - FREE_ATTEMPTS - 1).min(16);
        Duration::from_secs(1u64 << exponent).min(MAX_DELAY)
    }

    /// A successful unlock clears the count.
    pub fn record_success(&mut self) {
        self.failures = 0;
    }

    /// Failures since the last success.
    #[cfg(test)]
    pub fn failures(&self) -> u32 {
        self.failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grows_after_five_failures_and_caps() {
        let mut b = Backoff::default();
        let delays: Vec<u64> = (0..14).map(|_| b.record_failure().as_secs()).collect();
        assert_eq!(delays, [0, 0, 0, 0, 0, 1, 2, 4, 8, 16, 32, 60, 60, 60]);
        assert_eq!(b.failures(), 14);
    }

    #[test]
    fn success_resets() {
        let mut b = Backoff::default();
        for _ in 0..8 {
            b.record_failure();
        }
        b.record_success();
        assert_eq!(b.failures(), 0);
        assert_eq!(b.record_failure(), Duration::ZERO);
    }

    #[test]
    fn never_overflows() {
        let mut b = Backoff {
            failures: u32::MAX - 1,
        };
        assert_eq!(b.record_failure(), MAX_DELAY);
        assert_eq!(b.record_failure(), MAX_DELAY);
    }
}
