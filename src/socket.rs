//! Module used to communicate changes in the data structures.

use core::mem;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

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
    pub fn emit(&mut self, signal: Signal) {
        if let Some(slot) = &mut self.0 {
            slot.emit(signal);
        }
    }

    pub fn emit_if(&mut self, cond: bool, signal: Signal) {
        if cond {
            self.emit(signal);
        }
    }
}

/// Use this to receive signals from [`History`](crate::History) or [`Record`](crate::Record).
pub trait Slot {
    /// Receives a signal that describes the state change done to the data structures.
    fn emit(&mut self, signal: Signal);
}

impl<F: FnMut(Signal)> Slot for F {
    fn emit(&mut self, signal: Signal) {
        self(signal)
    }
}

/// The signal used for communicating state changes.
///
/// For example, if the history tree can no longer redo any edits,
/// it sends a `Redo(false)` signal to tell the user.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Signal {
    /// Says if the structures can undo.
    Undo(bool),
    /// Says if the structures can redo.
    Redo(bool),
    /// Says if the target is in a saved state.
    Saved(bool),
}

/// Default slot that does nothing.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Nop;

impl Slot for Nop {
    fn emit(&mut self, _: Signal) {}
}
