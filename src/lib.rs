//! Provides undo-redo functionality with dynamic dispatch and automatic command merging.
//!
//! It is an implementation of the command pattern, where all modifications are done
//! by creating objects of commands that applies the modifications. All commands knows
//! how to undo the changes it applies, and by using the provided data structures
//! it is easy to apply, undo, and redo changes made to a receiver.
//! Both linear and non-linear undo-redo functionality is provided through
//! the [Record] and [History] data structures.
//! This library provides more or less the same functionality as the [redo] library
//! but is more focused on ease of use instead of performance and control.
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
//! * Commands can be merged before it is applied to the data-structures by using the [merge!] macro.
//!   This makes it easy to build complex operations from smaller ones by combining them into a single command
//!   that can be applied, undone, and redone in a single step.
//! * Commands can be merged after being applied to the data-structures by implementing the [merge] method on the command.
//!   This allows smaller changes made gradually to be merged into larger operations that can be undone and redone
//!   in a single step.
//! * The receiver can be marked as being saved to disk and the data-structures can track the saved state and tell the user
//!   when it changes.
//! * The amount of changes being tracked can be configured by the user so only the `n` most recent changes are stored.
//!
//! # Examples
//!
//! Add this to `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! undo = "0.33"
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
//!     assert_eq!(record.as_receiver(), "abc");
//!     record.undo().unwrap()?;
//!     record.undo().unwrap()?;
//!     record.undo().unwrap()?;
//!     assert_eq!(record.as_receiver(), "");
//!     record.redo().unwrap()?;
//!     record.redo().unwrap()?;
//!     record.redo().unwrap()?;
//!     assert_eq!(record.as_receiver(), "abc");
//!     Ok(())
//! }
//! ```
//!
//! [Command]: trait.Command.html
//! [Record]: struct.Record.html
//! [History]: struct.History.html
//! [Queue]: struct.Queue.html
//! [Checkpoint]: struct.Checkpoint.html
//! [merge!]: macro.merge.html
//! [merge]: trait.Command.html#method.merge
//! [redo]: https://github.com/evenorog/redo

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

mod checkpoint;
#[cfg(feature = "display")]
mod display;
mod history;
mod merge;
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
    checkpoint::Checkpoint,
    history::{History, HistoryBuilder},
    merge::Merged,
    queue::Queue,
    record::{Record, RecordBuilder},
};

/// A specialized Result type for undo-redo operations.
pub type Result = std::result::Result<(), Box<dyn Error>>;

/// Base functionality for all commands.
#[cfg(not(feature = "display"))]
pub trait Command<R> {
    /// Applies the command on the receiver and returns `Ok` if everything went fine,
    /// and `Err` if something went wrong.
    fn apply(&mut self, receiver: &mut R) -> Result;

    /// Restores the state of the receiver as it was before the command was applied
    /// and returns `Ok` if everything went fine, and `Err` if something went wrong.
    fn undo(&mut self, receiver: &mut R) -> Result;

    /// Reapplies the command on the receiver and return `Ok` if everything went fine,
    /// and `Err` if something went wrong.
    ///
    /// The default implementation uses the [`apply`] implementation.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    #[inline]
    fn redo(&mut self, receiver: &mut R) -> Result {
        self.apply(receiver)
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
    ///     assert_eq!(record.as_receiver(), "abc");
    ///     // Calling `undo` once will undo all merged commands.
    ///     record.undo().unwrap()?;
    ///     assert_eq!(record.as_receiver(), "");
    ///     // Calling `redo` once will redo all merged commands.
    ///     record.redo().unwrap()?;
    ///     assert_eq!(record.as_receiver(), "abc");
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    fn merge(&self) -> Merge {
        Merge::No
    }

    /// Says if the command is dead.
    ///
    /// A dead command will be removed the next time it becomes the current command.
    /// This can be used to remove command if for example executing it caused an error,
    /// and it needs to be removed.
    #[inline]
    fn is_dead(&self) -> bool {
        false
    }
}

/// Base functionality for all commands.
#[cfg(feature = "display")]
pub trait Command<R>: fmt::Debug + fmt::Display {
    /// Applies the command on the receiver and returns `Ok` if everything went fine,
    /// and `Err` if something went wrong.
    fn apply(&mut self, receiver: &mut R) -> Result;

    /// Restores the state of the receiver as it was before the command was applied
    /// and returns `Ok` if everything went fine, and `Err` if something went wrong.
    fn undo(&mut self, receiver: &mut R) -> Result;

    /// Reapplies the command on the receiver and return `Ok` if everything went fine,
    /// and `Err` if something went wrong.
    ///
    /// The default implementation uses the [`apply`] implementation.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    #[inline]
    fn redo(&mut self, receiver: &mut R) -> Result {
        self.apply(receiver)
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
    ///     assert_eq!(record.as_receiver(), "abc");
    ///     // Calling `undo` once will undo all merged commands.
    ///     record.undo().unwrap()?;
    ///     assert_eq!(record.as_receiver(), "");
    ///     // Calling `redo` once will redo all merged commands.
    ///     record.redo().unwrap()?;
    ///     assert_eq!(record.as_receiver(), "abc");
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    fn merge(&self) -> Merge {
        Merge::No
    }

    /// Says if the command is dead.
    ///
    /// A dead command will be removed the next time it becomes the current command.
    /// This can be used to remove command if for example executing it caused an error,
    /// and it needs to be removed.
    #[inline]
    fn is_dead(&self) -> bool {
        false
    }
}

impl<R, C: Command<R> + ?Sized> Command<R> for Box<C> {
    #[inline]
    fn apply(&mut self, receiver: &mut R) -> Result {
        (**self).apply(receiver)
    }

    #[inline]
    fn undo(&mut self, receiver: &mut R) -> Result {
        (**self).undo(receiver)
    }

    #[inline]
    fn redo(&mut self, receiver: &mut R) -> Result {
        (**self).redo(receiver)
    }

    #[inline]
    fn merge(&self) -> Merge {
        (**self).merge()
    }

    #[inline]
    fn is_dead(&self) -> bool {
        (**self).is_dead()
    }
}

/// The signal sent when the record, the history, or the receiver changes.
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
    /// Says if the receiver is in a saved state.
    ///
    /// This signal will be emitted when the record enters or leaves its receivers saved state.
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
    /// Says if the current branch, or root, has changed.
    ///
    /// This is only emitted from `History`.
    Root {
        /// The old root.
        old: usize,
        /// The new root.
        new: usize,
    },
}

/// Says if the command should merge with another command.
#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum Merge {
    /// Always merges.
    Yes,
    /// Merges if the two commands have the same value.
    If(u32),
    /// Never merges.
    No,
}

/// A position in a history tree.
#[derive(Copy, Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
struct At {
    branch: usize,
    current: usize,
}

struct Meta<R> {
    command: Box<dyn Command<R>>,
    #[cfg(feature = "chrono")]
    timestamp: DateTime<Utc>,
}

impl<R> Meta<R> {
    #[inline]
    fn new(command: impl Command<R> + 'static) -> Meta<R> {
        Meta {
            command: Box::new(command),
            #[cfg(feature = "chrono")]
            timestamp: Utc::now(),
        }
    }
}

impl<R> From<Box<dyn Command<R>>> for Meta<R> {
    #[inline]
    fn from(command: Box<dyn Command<R>>) -> Self {
        Meta {
            command,
            #[cfg(feature = "chrono")]
            timestamp: Utc::now(),
        }
    }
}

impl<R> Command<R> for Meta<R> {
    #[inline]
    fn apply(&mut self, receiver: &mut R) -> Result {
        self.command.apply(receiver)
    }

    #[inline]
    fn undo(&mut self, receiver: &mut R) -> Result {
        self.command.undo(receiver)
    }

    #[inline]
    fn redo(&mut self, receiver: &mut R) -> Result {
        self.command.redo(receiver)
    }

    #[inline]
    fn merge(&self) -> Merge {
        self.command.merge()
    }

    #[inline]
    fn is_dead(&self) -> bool {
        self.command.is_dead()
    }
}

#[cfg(feature = "display")]
impl<R> fmt::Debug for Meta<R> {
    #[inline]
    #[cfg(not(feature = "chrono"))]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Meta")
            .field("command", &self.command)
            .finish()
    }

    #[inline]
    #[cfg(feature = "chrono")]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Meta")
            .field("command", &self.command)
            .field("timestamp", &self.timestamp)
            .finish()
    }
}

#[cfg(feature = "display")]
impl<R> fmt::Display for Meta<R> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.command as &dyn fmt::Display).fmt(f)
    }
}
