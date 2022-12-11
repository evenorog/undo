use core::fmt::{self, Debug, Display, Formatter};
use core::ops::IndexMut;
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

pub trait Entries: IndexMut<usize, Output = Entry<Self::Item>> {
    type Item;

    fn limit(&self) -> usize;
    fn len(&self) -> usize;
    fn back_mut(&mut self) -> Option<&mut Entry<Self::Item>>;
    fn push_back(&mut self, e: Entry<Self::Item>);
    fn pop_front(&mut self) -> Option<Entry<Self::Item>>;
    fn pop_back(&mut self) -> Option<Entry<Self::Item>>;
    fn split_off(&mut self, at: usize) -> Self;
    fn clear(&mut self);
}
