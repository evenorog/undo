//! Provides simple undo-redo functionality with dynamic dispatch.
//!
//! It is an implementation of the command pattern, where all modifications are done
//! by creating objects of commands that applies the modifications. All commands knows
//! how to undo the changes it applies, and by using the provided data structures
//! it is easy to apply, undo, and redo changes made to a target.
//!
//! # Features
//!
//! * [Command] provides the base functionality for all commands.
//! * [Record] provides linear undo-redo functionality.
//! * [Queue] wraps a [Record] and extends it with queue functionality.
//! * [Checkpoint] wraps a [Record] and extends it with checkpoint functionality.
//! * Commands can be merged after being applied to the data-structures by implementing the [merge] method on the command.
//!   This allows smaller changes made gradually to be merged into larger operations that can be undone and redone
//!   in a single step.
//! * The target can be marked as being saved to disk and the data-structures can track the saved state and notify
//!   when it changes.
//! * The amount of changes being tracked can be configured by the user so only the `N` most recent changes are stored.
//!
//! *If you need more advanced features, check out the [redo] crate.*
//!
//! # Examples
//!
//! Add this to `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! undo = "0.40"
//! ```
//!
//! And this to `main.rs`:
//!
//! ```
//! use undo::{Command, Record};
//!
//! #[derive(Debug)]
//! struct Add(char);
//!
//! impl Command<String> for Add {
//!     fn apply(&mut self, s: &mut String) -> undo::Result {
//!         s.push(self.0);
//!         Ok(())
//!     }
//!
//!     fn undo(&mut self, s: &mut String) -> undo::Result {
//!         self.0 = s.pop().ok_or("`s` is empty")?;
//!         Ok(())
//!     }
//! }
//!
//! fn main() -> undo::Result {
//!     let mut record = Record::default();
//!     record.apply(Add('a'))?;
//!     record.apply(Add('b'))?;
//!     record.apply(Add('c'))?;
//!     assert_eq!(record.target(), "abc");
//!     record.undo()?;
//!     record.undo()?;
//!     record.undo()?;
//!     assert_eq!(record.target(), "");
//!     record.redo()?;
//!     record.redo()?;
//!     record.redo()?;
//!     assert_eq!(record.target(), "abc");
//!     Ok(())
//! }
//! ```
//!
//! [Command]: trait.Command.html
//! [Record]: struct.Record.html
//! [Queue]: struct.Queue.html
//! [Checkpoint]: struct.Checkpoint.html
//! [Chain]: struct.Chain.html
//! [merge]: trait.Command.html#method.merge
//! [redo]: https://github.com/evenorog/redo

#![doc(html_root_url = "https://docs.rs/undo")]
#![deny(missing_docs)]

mod checkpoint;
mod command;
mod display;
mod queue;
mod record;

use chrono::{DateTime, Utc};
use std::error::Error;
use std::fmt;

pub use self::{
    checkpoint::Checkpoint,
    command::{Join, Merger, Text},
    display::Display,
    queue::Queue,
    record::{Record, RecordBuilder},
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
        "anonymous command".to_string()
    }
}

impl<T, C: Command<T> + ?Sized> Command<T> for Box<C> {
    fn apply(&mut self, target: &mut T) -> Result {
        (**self).apply(target)
    }

    fn undo(&mut self, target: &mut T) -> Result {
        (**self).undo(target)
    }

    fn redo(&mut self, target: &mut T) -> Result {
        (**self).redo(target)
    }

    fn merge(&self) -> Merge {
        (**self).merge()
    }

    fn text(&self) -> String {
        (**self).text()
    }
}

/// The signal used for communicating state changes.
///
/// For example, if the record can no longer redo any commands, it sends a `Redo(false)`
/// signal to tell the user.
#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum Signal {
    /// Says if the `Timeline` can undo.
    Undo(bool),
    /// Says if the `Timeline` can redo.
    Redo(bool),
    /// Says if the target is in a saved state.
    Saved(bool),
}

/// Says if the command should merge with another command.
#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum Merge {
    /// The command should merge.
    Yes,
    /// The command should merge if the two commands have the same value.
    If(u32),
    /// The command should not merge.
    No,
}

struct Entry<T> {
    command: Box<dyn Command<T>>,
    timestamp: DateTime<Utc>,
}

impl<T> Entry<T> {
    fn new(command: impl Command<T>) -> Entry<T> {
        Entry {
            command: Box::new(command),
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
