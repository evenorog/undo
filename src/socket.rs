//! Module used to communicate changes in the data structures.

use core::mem;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use std::sync::mpsc::{Sender, SyncSender};

/// Slot wrapper that adds some additional functionality.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
    pub fn emit(&mut self, signal: impl FnOnce() -> Signal) {
        if let Some(slot) = &mut self.0 {
            slot.on_emit(signal());
        }
    }

    pub fn emit_if(&mut self, cond: bool, signal: impl FnOnce() -> Signal) {
        if cond {
            self.emit(signal);
        }
    }
}

/// The `Signal` describes the state change done to the data structures.
///
/// See [`Slot`] for more information.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Signal {
    /// Emitted when the structures ability to undo has changed.
    Undo(bool),
    /// Emitted when the structures ability to redo has changed.
    Redo(bool),
    /// Emitted when the saved state has changed.
    Saved(bool),
    /// Emitted when the index has changed.
    Index(usize),
}

/// Use this to handle signals emitted.
///
/// This allows you to trigger events on certain state changes.
///
/// # Examples
/// ```
/// # include!("doctest.rs");
/// # use std::sync::mpsc;
/// # use undo::{FromFn, Record, Signal};
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
/// assert_eq!(iter.next(), Some(Signal::Undo(true)));
/// assert_eq!(iter.next(), Some(Signal::Saved(false)));
/// assert_eq!(iter.next(), Some(Signal::Index(1)));
/// assert_eq!(iter.next(), None);
///
/// record.undo(&mut target);
/// assert_eq!(iter.next(), Some(Signal::Undo(false)));
/// assert_eq!(iter.next(), Some(Signal::Redo(true)));
/// assert_eq!(iter.next(), Some(Signal::Saved(true)));
/// assert_eq!(iter.next(), Some(Signal::Index(0)));
/// assert_eq!(iter.next(), None);
/// # }
/// ```
pub trait Slot {
    /// Receives a signal that describes the state change done to the data structures.
    fn on_emit(&mut self, signal: Signal);
}

impl Slot for () {
    fn on_emit(&mut self, _: Signal) {}
}

impl<F: FnMut(Signal)> Slot for F {
    fn on_emit(&mut self, signal: Signal) {
        self(signal)
    }
}

#[cfg(feature = "std")]
impl Slot for Sender<Signal> {
    fn on_emit(&mut self, signal: Signal) {
        self.send(signal).ok();
    }
}

#[cfg(feature = "std")]
impl Slot for SyncSender<Signal> {
    fn on_emit(&mut self, signal: Signal) {
        self.send(signal).ok();
    }
}
