use crate::record::Builder as RecordBuilder;
use crate::History;

/// Builder for a [`History`].
///
/// # Examples
/// ```
/// # fn main() {
/// # use undo::{Add, History};
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
pub struct Builder<E, S = ()>(RecordBuilder<E, S>);

impl<E, S> Builder<E, S> {
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

    /// Connects the slot.
    pub fn connect(self, slot: S) -> Builder<E, S> {
        Builder(self.0.connect(slot))
    }

    /// Builds the history.
    pub fn build(self) -> History<E, S> {
        History::from(self.0.build())
    }
}

impl<E, S> Default for Builder<E, S> {
    fn default() -> Self {
        Builder(RecordBuilder::default())
    }
}
