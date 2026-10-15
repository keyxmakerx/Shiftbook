//! Durations as whole minutes. Time is never stored as floating-point hours.

use std::iter::Sum;
use std::ops::Add;

use serde::{Deserialize, Serialize};

/// The longest a single time entry may be: 24 hours.
pub const MAX_ENTRY_MINUTES: u32 = 24 * 60;

/// A duration in whole minutes. Adding saturates instead of overflowing.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Minutes(pub u32);

impl Minutes {
    /// Zero minutes.
    pub const ZERO: Minutes = Minutes(0);

    /// Wrap a number of minutes.
    pub const fn new(minutes: u32) -> Self {
        Self(minutes)
    }

    /// From whole hours and minutes: `from_hours_minutes(1, 30)` is 90 minutes.
    pub const fn from_hours_minutes(hours: u32, minutes: u32) -> Self {
        Self(hours.saturating_mul(60).saturating_add(minutes))
    }

    /// The raw number of minutes.
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Add, stopping at the largest value instead of overflowing.
    pub const fn saturating_add(self, other: Minutes) -> Minutes {
        Minutes(self.0.saturating_add(other.0))
    }

    /// True when this is zero minutes.
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }
}

impl From<u32> for Minutes {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl Add for Minutes {
    type Output = Minutes;

    fn add(self, rhs: Minutes) -> Minutes {
        self.saturating_add(rhs)
    }
}

impl Sum for Minutes {
    fn sum<I: Iterator<Item = Minutes>>(iter: I) -> Minutes {
        iter.fold(Minutes::ZERO, Minutes::saturating_add)
    }
}

impl<'a> Sum<&'a Minutes> for Minutes {
    fn sum<I: Iterator<Item = &'a Minutes>>(iter: I) -> Minutes {
        iter.copied().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construct_and_add() {
        assert_eq!(Minutes::from_hours_minutes(1, 30), Minutes(90));
        assert_eq!(Minutes(30) + Minutes(45), Minutes(75));
        assert_eq!(Minutes(u32::MAX) + Minutes(1), Minutes(u32::MAX));
        assert!(Minutes::ZERO.is_zero());
        assert_eq!(MAX_ENTRY_MINUTES, 1440);
    }

    #[test]
    fn sums() {
        let list = [Minutes(10), Minutes(20), Minutes(30)];
        assert_eq!(list.iter().sum::<Minutes>(), Minutes(60));
        assert_eq!(list.into_iter().sum::<Minutes>(), Minutes(60));
        assert_eq!(
            Vec::<Minutes>::new().into_iter().sum::<Minutes>(),
            Minutes(0)
        );
    }
}
