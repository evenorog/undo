//! Module used to communicate changes in the data structures.

use core::mem;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use std::sync::mpsc::{Sender, SyncSender};

/// Slot wrapper that adds some additional functionality.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(transparent)]
#[derive(Clone, Debug)]
pub(crate) struct Socket<S>(Option<S>);

impl<S> Socket<S> {
    pub const fn new(slot: S) -> Socket<S> {
        Socket(Some(slot))
    }

    pub fn connect(&mut self, slot: Option<S>) -> Option<S> {
        mem::replace(&mut self.0, slot)
    }

    pub fn disconnect(&mut self) -> Option<S> {
        self.0.take()
    }
}

impl<S> Default for Socket<S> {
    fn default() -> Self {
        Socket(None)
    }
}

impl<S: Slot> Socket<S> {
    pub fn emit(&mut self, event: impl FnOnce() -> Event) {
        if let Some(slot) = &mut self.0 {
            slot.on_emit(event());
        }
    }

    pub fn emit_if(&mut self, cond: bool, event: impl FnOnce() -> Event) {
        if cond {
            self.emit(event);
        }
    }
}

/// Describes an event on the structures.
///
/// See [`Slot`] for more information.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Event {
    /// Emitted when the structures ability to undo has changed.
    Undo(bool),
    /// Emitted when the structures ability to redo has changed.
    Redo(bool),
    /// Emitted when the saved state has changed.
    Saved(bool),
    /// Emitted when the index has changed.
    Index(usize),
}

/// Handles events.
///
/// # Examples
/// ```
/// # use std::sync::mpsc;
/// # use undo::{Add, Record, Event};
/// # fn main() {
/// let (sender, receiver) = mpsc::channel();
/// let mut iter = receiver.try_iter();
///
/// let mut target = String::new();
/// let mut record = Record::builder()
///     .connect(sender)
///     .build();
///
/// record.edit(&mut target, Add('a'));
/// assert_eq!(iter.next(), Some(Event::Undo(true)));
/// assert_eq!(iter.next(), Some(Event::Saved(false)));
/// assert_eq!(iter.next(), Some(Event::Index(1)));
/// assert_eq!(iter.next(), None);
///
/// record.undo(&mut target);
/// assert_eq!(iter.next(), Some(Event::Undo(false)));
/// assert_eq!(iter.next(), Some(Event::Redo(true)));
/// assert_eq!(iter.next(), Some(Event::Saved(true)));
/// assert_eq!(iter.next(), Some(Event::Index(0)));
/// assert_eq!(iter.next(), None);
/// # }
/// ```
pub trait Slot {
    /// Receives an event that describes the state change done to the structures.
    fn on_emit(&mut self, event: Event);
}

impl Slot for () {
    fn on_emit(&mut self, _: Event) {}
}

impl<F: FnMut(Event)> Slot for F {
    fn on_emit(&mut self, event: Event) {
        self(event)
    }
}

#[cfg(feature = "std")]
impl Slot for Sender<Event> {
    fn on_emit(&mut self, event: Event) {
        self.send(event).ok();
    }
}

#[cfg(feature = "std")]
impl Slot for SyncSender<Event> {
    fn on_emit(&mut self, event: Event) {
        self.send(event).ok();
    }
}
