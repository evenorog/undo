use super::Socket;
use crate::{Nop, Record, Slot};
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::num::NonZeroUsize;

/// Builder for a record.
///
/// # Examples
/// ```
/// # include!("../doctest.rs");
/// # fn main() {
/// # use undo::Record;
/// # let mut target = String::new();
/// let mut record = Record::builder()
///     .limit(100)
///     .capacity(100)
///     .connect(|s| { dbg!(s); })
///     .build();
/// # record.apply(&mut target, Push('a'));
/// # }
/// ```
#[derive(Debug)]
pub struct Builder<A, S = Nop> {
    capacity: usize,
    limit: NonZeroUsize,
    saved: bool,
    socket: Socket<S>,
    pd: PhantomData<A>,
}

impl<A, S> Builder<A, S> {
    /// Returns a builder for a record.
    pub fn new() -> Builder<A, S> {
        Builder {
            capacity: 0,
            limit: NonZeroUsize::new(usize::MAX).unwrap(),
            saved: true,
            socket: Socket::default(),
            pd: PhantomData,
        }
    }

    /// Sets the capacity for the record.
    pub fn capacity(mut self, capacity: usize) -> Builder<A, S> {
        self.capacity = capacity;
        self
    }

    /// Sets the `limit` of the record.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    pub fn limit(mut self, limit: usize) -> Builder<A, S> {
        self.limit = NonZeroUsize::new(limit).expect("limit can not be `0`");
        self
    }

    /// Sets if the target is initially in a saved state.
    /// By default the target is in a saved state.
    pub fn saved(mut self, saved: bool) -> Builder<A, S> {
        self.saved = saved;
        self
    }

    /// Builds the record.
    pub fn build(self) -> Record<A, S> {
        Record {
            entries: VecDeque::with_capacity(self.capacity),
            limit: self.limit,
            current: 0,
            saved: self.saved.then_some(0),
            socket: self.socket,
        }
    }
}

impl<A, S: Slot> Builder<A, S> {
    /// Connects the slot.
    pub fn connect(mut self, slot: S) -> Builder<A, S> {
        self.socket = Socket::new(slot);
        self
    }
}

impl<A> Default for Builder<A> {
    fn default() -> Self {
        Builder::new()
    }
}
