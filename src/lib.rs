//! **Low-level undo-redo functionality.**
//!
//! It is an implementation of the command pattern, where all modifications are done
//! by creating objects of commands that applies the modifications. All commands knows
//! how to undo the changes it applies, and by using the provided data structures
//! it is easy to apply, undo, and redo changes made to a target.
//!
//! # Features
//!
//! * [Command](trait.Command.html) provides the base functionality for all commands.
//! * [Record](struct.Record.html) provides basic linear undo-redo functionality.
//! * [History](struct.History.html) provides non-linear undo-redo functionality that allows you to jump between different branches.
//! * Queue wraps a record or history and extends them with queue functionality.
//! * Checkpoint wraps a record or history and extends them with checkpoint functionality.
//! * Commands can be merged into a single command by implementing the
//!   [merge](trait.Command.html#method.merge) method on the command.
//!   This allows smaller commands to be used to build more complex operations, or smaller incremental changes to be
//!   merged into larger changes that can be undone and redone in a single step.
//! * The target can be marked as being saved to disk and the data-structures can track the saved state and notify
//!   when it changes.
//! * The amount of changes being tracked can be configured by the user so only the `N` most recent changes are stored.
//! * Configurable display formatting using the display structure.
//! * The library can be used as `no_std` by default.
//!
//! # Cargo Feature Flags
//!
//! * `chrono`: Enables time stamps and time travel.
//! * `serde`: Enables serialization and deserialization.
//! * `colored`: Enables colored output when visualizing the display structures.

#![no_std]
#![doc(html_root_url = "https://docs.rs/undo")]
#![deny(missing_docs)]
#![forbid(unsafe_code)]

extern crate alloc;

mod format;
pub mod history;
pub mod record;

#[cfg(feature = "chrono")]
use chrono::{DateTime, Utc};
use core::fmt;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub use self::{history::History, record::Record};

/// A specialized Result type for undo-redo operations.
pub type Result<C> = core::result::Result<(), <C as Command>::Error>;

/// Base functionality for all commands.
pub trait Command: Sized {
    /// The target type.
    type Target;
    /// The error type.
    type Error;

    /// Applies the command on the target and returns `Ok` if everything went fine,
    /// and `Err` if something went wrong.
    fn apply(&mut self, target: &mut Self::Target) -> Result<Self>;

    /// Restores the state of the target as it was before the command was applied
    /// and returns `Ok` if everything went fine, and `Err` if something went wrong.
    fn undo(&mut self, target: &mut Self::Target) -> Result<Self>;

    /// Reapplies the command on the target and return `Ok` if everything went fine,
    /// and `Err` if something went wrong.
    ///
    /// The default implementation uses the [`apply`](trait.Command.html#tymethod.apply) implementation.
    fn redo(&mut self, target: &mut Self::Target) -> Result<Self> {
        self.apply(target)
    }

    /// Used for manual merging of commands.
    fn merge(&mut self, command: Self) -> Merge<Self> {
        Merge::No(command)
    }
}

/// The signal used for communicating state changes.
///
/// For example, if the record can no longer redo any commands, it sends a `Redo(false)`
/// signal to tell the user.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Signal {
    /// Says if the structures can undo.
    Undo(bool),
    /// Says if the structures can redo.
    Redo(bool),
    /// Says if the target is in a saved state.
    Saved(bool),
}

/// Says if the command have been merged with another command.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Merge<C> {
    /// The commands have been merged.
    Yes,
    /// The commands have not been merged.
    No(C),
    /// The two commands cancels each other out.
    Annul,
}

/// A position in a history tree.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Default, Hash, Eq, PartialEq)]
struct At {
    branch: usize,
    current: usize,
}

impl At {
    fn new(branch: usize, current: usize) -> At {
        At { branch, current }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
struct Slot<F> {
    #[cfg_attr(feature = "serde", serde(default = "Option::default", skip))]
    f: Option<F>,
}

impl<F: FnMut(Signal)> Slot<F> {
    fn new(f: F) -> Slot<F> {
        Slot { f: Some(f) }
    }

    fn emit(&mut self, signal: Signal) {
        if let Some(ref mut f) = self.f {
            f(signal);
        }
    }

    fn emit_if(&mut self, cond: bool, signal: Signal) {
        if cond {
            self.emit(signal)
        }
    }
}

impl<F> Default for Slot<F> {
    fn default() -> Self {
        Slot { f: None }
    }
}

impl<F> fmt::Debug for Slot<F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.f {
            Some(_) => f.pad("Slot { .. }"),
            None => f.pad("Empty"),
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
struct Entry<C> {
    command: C,
    #[cfg(feature = "chrono")]
    timestamp: DateTime<Utc>,
}

impl<C> From<C> for Entry<C> {
    fn from(command: C) -> Self {
        Entry {
            command,
            #[cfg(feature = "chrono")]
            timestamp: Utc::now(),
        }
    }
}

impl<C: Command> Command for Entry<C> {
    type Target = C::Target;
    type Error = C::Error;

    fn apply(&mut self, target: &mut Self::Target) -> Result<Self> {
        self.command.apply(target)
    }

    fn undo(&mut self, target: &mut Self::Target) -> Result<Self> {
        self.command.undo(target)
    }

    fn redo(&mut self, target: &mut Self::Target) -> Result<Self> {
        self.command.redo(target)
    }

    fn merge(&mut self, command: Self) -> Merge<Self> {
        match self.command.merge(command.command) {
            Merge::Yes => Merge::Yes,
            Merge::No(command) => Merge::No(Entry::from(command)),
            Merge::Annul => Merge::Annul,
        }
    }
}

impl<C: fmt::Display> fmt::Display for Entry<C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.command as &dyn fmt::Display).fmt(f)
    }
}
