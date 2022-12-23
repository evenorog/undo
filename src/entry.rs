use alloc::collections::VecDeque;
use core::fmt::{self, Debug, Display, Formatter};
use core::num::NonZeroUsize;
use core::ops::{Index, IndexMut};
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

/// A deque that holds a limit of how many items it can hold.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub(crate) struct LimitDeque<T> {
    pub deque: VecDeque<Entry<T>>,
    pub limit: NonZeroUsize,
}

impl<T> LimitDeque<T> {
    pub fn new(capacity: usize, limit: NonZeroUsize) -> LimitDeque<T> {
        LimitDeque {
            deque: VecDeque::with_capacity(capacity),
            limit,
        }
    }
}

impl<T> Entries for LimitDeque<T> {
    type Item = T;

    fn limit(&self) -> usize {
        self.limit.get()
    }

    fn len(&self) -> usize {
        self.deque.len()
    }

    fn back_mut(&mut self) -> Option<&mut Entry<T>> {
        self.deque.back_mut()
    }

    fn push_back(&mut self, t: Entry<T>) {
        self.deque.push_back(t)
    }

    fn pop_front(&mut self) -> Option<Entry<T>> {
        self.deque.pop_front()
    }

    fn pop_back(&mut self) -> Option<Entry<T>> {
        self.deque.pop_back()
    }

    fn split_off(&mut self, at: usize) -> Self {
        LimitDeque {
            deque: self.deque.split_off(at),
            limit: self.limit,
        }
    }

    fn clear(&mut self) {
        self.deque.clear();
    }
}

impl<T> Index<usize> for LimitDeque<T> {
    type Output = Entry<T>;

    fn index(&self, index: usize) -> &Self::Output {
        self.deque.index(index)
    }
}

impl<T> IndexMut<usize> for LimitDeque<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.deque.index_mut(index)
    }
}
