use core::fmt::{self, Debug, Display, Formatter};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "time")]
use time::OffsetDateTime;

/// Wrapper around an action that contains additional metadata.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Entry<A> {
    pub action: A,
    #[cfg(feature = "time")]
    pub timestamp: OffsetDateTime,
}

impl<A> From<A> for Entry<A> {
    fn from(action: A) -> Self {
        Entry {
            action,
            #[cfg(feature = "time")]
            timestamp: OffsetDateTime::now_utc(),
        }
    }
}

impl<A: Display> Display for Entry<A> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        (&self.action as &dyn Display).fmt(f)
    }
}
