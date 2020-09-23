//! **Provides simple undo-redo functionality with dynamic dispatch.**
//!
//! It is an implementation of the command pattern, where all modifications are done
//! by creating objects of commands that applies the modifications. All commands knows
//! how to undo the changes it applies, and by using the provided data structures
//! it is easy to apply, undo, and redo changes made to a target.
//!
//! # Features
//!
//! * [Command](trait.Command.html) provides the base functionality for all commands.
//! * [Record](struct.Record.html) is a collection of commands and provides the undo-redo functionality.
//! * [Queue](struct.Queue.html) wraps a record and extends it with queue functionality.
//! * [Checkpoint](struct.Checkpoint.html) wraps a record and extends it with checkpoint functionality.
//! * Commands can be merged after being applied to the data-structures by implementing the
//!   [merge](trait.Command.html#method.merge) method on the command.
//!   This allows smaller changes made gradually to be merged into larger operations that can be undone and redone
//!   in a single step.
//! * Configurable display formatting using [Display](struct.Display.html).
//! * The target can be marked as being saved to disk and the record can track the saved state and notify
//!   when it changes.
//! * The amount of changes being tracked can be configured by the user so only the `N` most recent changes are stored.
//!
//! *If you need more advanced features, check out the [redo](https://github.com/evenorog/redo) crate.*

#![doc(html_root_url = "https://docs.rs/undo")]
#![deny(missing_docs)]

mod checkpoint;
mod command;
mod display;
mod queue;
mod record;

use chrono::{DateTime, Utc};
use std::{error::Error, fmt};

pub use self::{
    checkpoint::Checkpoint,
    command::{from_fn, join, with_merge, with_text, FromFn, Join, WithMerge, WithText},
    display::Display,
    queue::Queue,
    record::{Builder, Record},
};

/// A specialized Result type for undo-redo operations.
pub type Result = std::result::Result<(), Box<dyn Error>>;

/// Base functionality for all commands.
pub trait Command<T>: 'static + fmt::Debug {
    /// Applies the command on the target and returns `Ok` if everything went fine,
    /// and `Err` if something went wrong.
    fn apply(&mut self, target: &mut T) -> Result;

    /// Restores the state of the target as it was before the command was applied
    /// and returns `Ok` if everything went fine, and `Err` if something went wrong.
    fn undo(&mut self, target: &mut T) -> Result;

    /// Reapplies the command on the target and return `Ok` if everything went fine,
    /// and `Err` if something went wrong.
    ///
    /// The default implementation uses the [`apply`] implementation.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    fn redo(&mut self, target: &mut T) -> Result {
        self.apply(target)
    }

    /// Used for automatic merging of commands.
    ///
    /// When commands are merged together, undoing and redoing them are done in one step.
    fn merge(&self) -> Merge {
        Merge::No
    }

    /// Returns the text of the command.
    fn text(&self) -> String {
        format!("command @ {:?}", self as *const _)
    }
}

/// The signal used for communicating state changes.
///
/// For example, if the record can no longer redo any commands, it sends a `Redo(false)`
/// signal to tell the user.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Signal {
    /// Says if the record can undo.
    Undo(bool),
    /// Says if the record can redo.
    Redo(bool),
    /// Says if the target is in a saved state.
    Saved(bool),
}

/// Says if the command should merge with another command.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Merge {
    /// The command should merge.
    Yes,
    /// The command should merge if the two commands have the same value.
    If(u32),
    /// The command should not merge.
    No,
}

#[derive(Default)]
struct Slot {
    f: Option<Box<dyn FnMut(Signal)>>,
}

impl Slot {
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

impl fmt::Debug for Slot {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.f {
            Some(_) => f.pad("Slot { .. }"),
            None => f.pad("Empty"),
        }
    }
}

struct Entry<T> {
    command: Box<dyn Command<T>>,
    timestamp: DateTime<Utc>,
}

impl<T> Entry<T> {
    fn new(command: Box<dyn Command<T>>) -> Entry<T> {
        Entry {
            command,
            timestamp: Utc::now(),
        }
    }
}

impl<T: 'static> Command<T> for Entry<T> {
    fn apply(&mut self, target: &mut T) -> Result {
        self.command.apply(target)
    }

    fn undo(&mut self, target: &mut T) -> Result {
        self.command.undo(target)
    }

    fn redo(&mut self, target: &mut T) -> Result {
        self.command.redo(target)
    }

    fn merge(&self) -> Merge {
        self.command.merge()
    }

    fn text(&self) -> String {
        self.command.text()
    }
}

impl<T> fmt::Debug for Entry<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Entry")
            .field("command", &self.command)
            .field("timestamp", &self.timestamp)
            .finish()
    }
}
