use super::Socket;
use crate::Record;
use alloc::collections::VecDeque;
use core::marker::PhantomData;
use core::num::NonZeroUsize;

/// Builder for a [`Record`].
///
/// # Examples
/// ```
/// # fn main() {
/// # use undo::{Add, Record};
/// # let mut target = String::new();
/// let mut record = Record::builder()
///     .limit(100)
///     .capacity(100)
///     .connect(|s| { dbg!(s); })
///     .build();
/// # record.edit(&mut target, Add('a'));
/// # }
/// ```
#[derive(Debug)]
pub struct Builder<E, S = ()> {
    capacity: usize,
    limit: NonZeroUsize,
    saved: bool,
    socket: Socket<S>,
    pd: PhantomData<E>,
}

impl<E, S> Builder<E, S> {
    /// Sets the capacity for the record.
    pub fn capacity(mut self, capacity: usize) -> Builder<E, S> {
        self.capacity = capacity;
        self
    }

    /// Sets the `limit` of the record.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    pub fn limit(mut self, limit: usize) -> Builder<E, S> {
        self.limit = NonZeroUsize::new(limit).expect("limit can not be `0`");
        self
    }

    /// Sets if the target is initially in a saved state.
    /// By default the target is in a saved state.
    pub fn saved(mut self, saved: bool) -> Builder<E, S> {
        self.saved = saved;
        self
    }

    /// Connects the slot.
    pub fn connect(mut self, slot: S) -> Builder<E, S> {
        self.socket = Socket::new(slot);
        self
    }

    /// Builds the record.
    pub fn build(self) -> Record<E, S> {
        Record {
            entries: VecDeque::with_capacity(self.capacity),
            limit: self.limit,
            index: 0,
            saved: self.saved.then_some(0),
            socket: self.socket,
        }
    }
}

impl<E, S> Default for Builder<E, S> {
    fn default() -> Self {
        Builder {
            capacity: 0,
            limit: NonZeroUsize::new(usize::MAX).unwrap(),
            saved: true,
            socket: Socket::default(),
            pd: PhantomData,
        }
    }
}
