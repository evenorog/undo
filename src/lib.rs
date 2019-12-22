//! Provides undo-redo functionality with dynamic dispatch and automatic command merging.
//!
//! It is an implementation of the command pattern, where all modifications are done
//! by creating objects of commands that applies the modifications. All commands knows
//! how to undo the changes it applies, and by using the provided data structures
//! it is easy to apply, undo, and redo changes made to a target.
//! Both linear and non-linear undo-redo functionality is provided through
//! the [Record] and [History] data structures.
//!
//! # Contents
//!
//! * [Command] provides the base functionality for all commands.
//! * [Record] provides linear undo-redo functionality.
//! * [History] provides non-linear undo-redo functionality that allows you to jump between different branches.
//! * [Queue] wraps a [Record] or [History] and extends them with queue functionality.
//! * [Checkpoint] wraps a [Record] or [History] and extends them with checkpoint functionality.
//! * Configurable display formatting is provided when the `display` feature is enabled.
//! * Time stamps and time travel is provided when the `chrono` feature is enabled.
//!
//! # Concepts
//!
//! * Commands can be chained before they are applied using the [Chain] structure.
//!   This makes it easy to build complex operations from smaller ones by combining them into a single command
//!   that can be applied, undone, and redone in a single step.
//! * Commands can be merged after being applied to the data-structures by implementing the [merge] method on the command.
//!   This allows smaller changes made gradually to be merged into larger operations that can be undone and redone
//!   in a single step.
//! * The target can be marked as being saved to disk and the data-structures can track the saved state and tell the user
//!   when it changes.
//! * The amount of changes being tracked can be configured by the user so only the `n` most recent changes are stored.
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
//!     record.undo().unwrap()?;
//!     record.undo().unwrap()?;
//!     record.undo().unwrap()?;
//!     assert_eq!(record.target(), "");
//!     record.redo().unwrap()?;
//!     record.redo().unwrap()?;
//!     record.redo().unwrap()?;
//!     assert_eq!(record.target(), "abc");
//!     Ok(())
//! }
//! ```
//!
//! [Command]: trait.Command.html
//! [Record]: struct.Record.html
//! [History]: struct.History.html
//! [Queue]: struct.Queue.html
//! [Checkpoint]: struct.Checkpoint.html
//! [Chain]: struct.Chain.html
//! [merge]: trait.Command.html#method.merge

#![doc(html_root_url = "https://docs.rs/undo/latest")]
#![deny(
    bad_style,
    bare_trait_objects,
    missing_docs,
    unused_import_braces,
    unused_qualifications,
    unsafe_code,
    unstable_features
)]

mod chain;
mod checkpoint;
#[cfg(feature = "display")]
mod display;
mod history;
mod queue;
mod record;

#[cfg(feature = "chrono")]
use chrono::{DateTime, Utc};
use std::error::Error;
#[cfg(feature = "display")]
use std::fmt;

#[cfg(feature = "display")]
pub use self::display::Display;
pub use self::{
    chain::Chain,
    checkpoint::Checkpoint,
    history::{History, HistoryBuilder},
    queue::Queue,
    record::{Record, RecordBuilder},
};

/// A specialized Result type for undo-redo operations.
pub type Result = std::result::Result<(), Box<dyn Error>>;

/// Base functionality for data structures that can use commands.
pub trait Timeline {
    /// The target type used.
    type Target;

    /// Applies the command to the record.
    fn apply(&mut self, command: impl Command<Self::Target>) -> Result;

    /// Calls the undo method on the current command.
    fn undo(&mut self) -> Option<Result>;

    /// Calls the redo method on the current command.
    fn redo(&mut self) -> Option<Result>;
}

/// Base functionality for all commands.
#[cfg(not(feature = "display"))]
pub trait Command<T>: 'static {
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
    #[inline]
    fn redo(&mut self, target: &mut T) -> Result {
        self.apply(target)
    }

    /// Used for automatic merging of commands.
    ///
    /// When commands are merged together, undoing and redoing them are done in one step.
    ///
    /// # Examples
    /// ```
    /// # use undo::*;
    /// struct Add(char);
    ///
    /// impl Command<String> for Add {
    ///     fn apply(&mut self, s: &mut String) -> undo::Result {
    ///         s.push(self.0);
    ///         Ok(())
    ///     }
    ///
    ///     fn undo(&mut self, s: &mut String) -> undo::Result {
    ///         self.0 = s.pop().ok_or("`s` is empty")?;
    ///         Ok(())
    ///     }
    ///
    ///     fn merge(&self) -> Merge {
    ///         Merge::Yes
    ///     }
    /// }
    ///
    /// fn main() -> undo::Result {
    ///     let mut record = Record::default();
    ///     // The `a`, `b`, and `c` commands are merged.
    ///     record.apply(Add('a'))?;
    ///     record.apply(Add('b'))?;
    ///     record.apply(Add('c'))?;
    ///     assert_eq!(record.target(), "abc");
    ///     // Calling `undo` once will undo all merged commands.
    ///     record.undo().unwrap()?;
    ///     assert_eq!(record.target(), "");
    ///     // Calling `redo` once will redo all merged commands.
    ///     record.redo().unwrap()?;
    ///     assert_eq!(record.target(), "abc");
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    fn merge(&self) -> Merge {
        Merge::No
    }
}

/// Base functionality for all commands.
#[cfg(feature = "display")]
pub trait Command<T>: 'static + fmt::Debug + fmt::Display {
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
    #[inline]
    fn redo(&mut self, target: &mut T) -> Result {
        self.apply(target)
    }

    /// Used for automatic merging of commands.
    ///
    /// When commands are merged together, undoing and redoing them are done in one step.
    ///
    /// # Examples
    /// ```
    /// # use undo::*;
    /// #[derive(Debug)]
    /// struct Add(char);
    ///
    /// impl Command<String> for Add {
    ///     fn apply(&mut self, s: &mut String) -> undo::Result {
    ///         s.push(self.0);
    ///         Ok(())
    ///     }
    ///
    ///     fn undo(&mut self, s: &mut String) -> undo::Result {
    ///         self.0 = s.pop().ok_or("`s` is empty")?;
    ///         Ok(())
    ///     }
    ///
    ///     fn merge(&self) -> Merge {
    ///         Merge::Yes
    ///     }
    /// }
    ///
    /// fn main() -> undo::Result {
    ///     let mut record = Record::default();
    ///     // The `a`, `b`, and `c` commands are merged.
    ///     record.apply(Add('a'))?;
    ///     record.apply(Add('b'))?;
    ///     record.apply(Add('c'))?;
    ///     assert_eq!(record.target(), "abc");
    ///     // Calling `undo` once will undo all merged commands.
    ///     record.undo().unwrap()?;
    ///     assert_eq!(record.target(), "");
    ///     // Calling `redo` once will redo all merged commands.
    ///     record.redo().unwrap()?;
    ///     assert_eq!(record.target(), "abc");
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    fn merge(&self) -> Merge {
        Merge::No
    }
}

impl<T, C: Command<T> + ?Sized> Command<T> for Box<C> {
    #[inline]
    fn apply(&mut self, target: &mut T) -> Result {
        (**self).apply(target)
    }

    #[inline]
    fn undo(&mut self, target: &mut T) -> Result {
        (**self).undo(target)
    }

    #[inline]
    fn redo(&mut self, target: &mut T) -> Result {
        (**self).redo(target)
    }

    #[inline]
    fn merge(&self) -> Merge {
        (**self).merge()
    }
}

/// The signal sent when the record, the history, or the target changes.
///
/// When one of these states changes, they will send a corresponding signal to the user.
/// For example, if the record can no longer redo any commands, it sends a `Redo(false)`
/// signal to tell the user.
#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum Signal {
    /// Says if the record can undo.
    ///
    /// This signal will be emitted when the records ability to undo changes.
    Undo(bool),
    /// Says if the record can redo.
    ///
    /// This signal will be emitted when the records ability to redo changes.
    Redo(bool),
    /// Says if the target is in a saved state.
    ///
    /// This signal will be emitted when the record enters or leaves its targets saved state.
    Saved(bool),
    /// Says if the current command has changed.
    ///
    /// This signal will be emitted when the current command has changed. This includes
    /// when two commands have been merged, in which case `old == new`.
    Current {
        /// The old current command.
        old: usize,
        /// The new current command.
        new: usize,
    },
    /// Says if the current branch has changed.
    ///
    /// This is currently only emitted from `History`.
    Branch {
        /// The old branch.
        old: usize,
        /// The new branch.
        new: usize,
    },
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

/// A position in a history tree.
#[derive(Copy, Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
struct At {
    branch: usize,
    current: usize,
}

impl At {
    pub fn new(branch: usize, current: usize) -> At {
        At { branch, current }
    }
}

struct Entry<T> {
    command: Box<dyn Command<T>>,
    #[cfg(feature = "chrono")]
    timestamp: DateTime<Utc>,
}

impl<T> Entry<T> {
    #[inline]
    fn new(command: impl Command<T>) -> Entry<T> {
        Entry {
            command: Box::new(command),
            #[cfg(feature = "chrono")]
            timestamp: Utc::now(),
        }
    }
}

impl<T: 'static> Command<T> for Entry<T> {
    #[inline]
    fn apply(&mut self, target: &mut T) -> Result {
        self.command.apply(target)
    }

    #[inline]
    fn undo(&mut self, target: &mut T) -> Result {
        self.command.undo(target)
    }

    #[inline]
    fn redo(&mut self, target: &mut T) -> Result {
        self.command.redo(target)
    }

    #[inline]
    fn merge(&self) -> Merge {
        self.command.merge()
    }
}

#[cfg(feature = "display")]
impl<T> fmt::Debug for Entry<T> {
    #[inline]
    #[cfg(not(feature = "chrono"))]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Entry")
            .field("command", &self.command)
            .finish()
    }

    #[inline]
    #[cfg(feature = "chrono")]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Entry")
            .field("command", &self.command)
            .field("timestamp", &self.timestamp)
            .finish()
    }
}

#[cfg(feature = "display")]
impl<T> fmt::Display for Entry<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.command as &dyn fmt::Display).fmt(f)
    }
}
