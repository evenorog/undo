use crate::record::Builder as RecordBuilder;
use crate::{History, Nop, Slot};

/// Builder for a History.
///
/// # Examples
/// ```
/// # include!("../doctest.rs");
/// # fn main() {
/// # use undo::History;
/// # let mut target = String::new();
/// let mut history = History::builder()
///     .limit(100)
///     .capacity(100)
///     .connect(|s| { dbg!(s); })
///     .build();
/// # history.edit(&mut target, Add('a'));
/// # }
/// ```
#[derive(Debug)]
pub struct Builder<E, S = Nop>(RecordBuilder<E, S>);

impl<E, S> Builder<E, S> {
    /// Returns a builder for a history.
    pub fn new() -> Builder<E, S> {
        Builder(RecordBuilder::new())
    }

    /// Sets the capacity for the history.
    pub fn capacity(self, capacity: usize) -> Builder<E, S> {
        Builder(self.0.capacity(capacity))
    }

    /// Sets the `limit` for the history.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    pub fn limit(self, limit: usize) -> Builder<E, S> {
        Builder(self.0.limit(limit))
    }

    /// Sets if the target is initially in a saved state.
    /// By default the target is in a saved state.
    pub fn saved(self, saved: bool) -> Builder<E, S> {
        Builder(self.0.saved(saved))
    }

    /// Builds the history.
    pub fn build(self) -> History<E, S> {
        History::from(self.0.build())
    }
}

impl<E, S: Slot> Builder<E, S> {
    /// Connects the slot.
    pub fn connect(self, slot: S) -> Builder<E, S> {
        Builder(self.0.connect(slot))
    }
}

impl<E> Default for Builder<E> {
    fn default() -> Self {
        Builder::new()
    }
}
