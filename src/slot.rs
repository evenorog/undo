//! Module used to communicate changes in the data structures.

use core::mem;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Trait for emitting signals.
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
/// For example, if the history tree can no longer redo any actions,
/// it sends a `Redo(false)` signal to tell the user.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
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
pub struct NoOp;

impl Slot for NoOp {
    fn emit(&mut self, _: Signal) {}
}

/// Slot wrapper that adds some additional functionality.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(transparent))]
#[derive(Clone, Debug)]
pub(crate) struct SW<S>(Option<S>);

impl<S> SW<S> {
    pub const fn new(slot: S) -> SW<S> {
        SW(Some(slot))
    }

    pub fn connect(&mut self, slot: Option<S>) -> Option<S> {
        mem::replace(&mut self.0, slot)
    }

    pub fn disconnect(&mut self) -> Option<S> {
        self.0.take()
    }
}

impl<S> Default for SW<S> {
    fn default() -> Self {
        SW(None)
    }
}

impl<S: Slot> SW<S> {
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
