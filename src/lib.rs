//! An undo-redo library with dynamic dispatch and automatic command merging.
//!
//! It uses the [command pattern] where the user modifies the receiver by
//! applying commands on it. Since each command knows how to undo and redo
//! the changes it applies to the receiver, the state of the receiver can
//! be rolled forwards or backwards by calling undo or redo in the correct order.
//!
//! The [Record] and [History] provides functionality to store and keep track
//! of the applied commands, and makes it easy to undo and redo changes.
//! The Record provides a stack based undo-redo functionality, while the
//! History provides a tree based undo-redo functionality where you can
//! jump between different branches.
//!
//! Commands can be merged using the [`merge!`] macro or the [`merge`] method.
//! When two commands are merged, undoing and redoing them are done in a single step.
//!
//! [command pattern]: https://en.wikipedia.org/wiki/Command_pattern
//! [Record]: struct.Record.html
//! [History]: struct.History.html
//! [`merge!`]: macro.merge.html
//! [`merge`]: trait.Command.html#method.merge

#![deny(
    bad_style,
    bare_trait_objects,
    missing_debug_implementations,
    unused_import_braces,
    unused_qualifications,
    unsafe_code,
    unstable_features,
)]

#[cfg(feature = "display")]
#[macro_use]
extern crate bitflags;
extern crate chrono;
#[cfg(feature = "display")]
extern crate colored;
extern crate fnv;

#[cfg(feature = "display")]
mod display;
mod history;
mod merge;
mod record;
mod signal;

use chrono::{DateTime, Local};
use std::{error::Error as StdError, fmt};

#[cfg(feature = "display")]
pub use display::Display;
pub use history::{History, HistoryBuilder};
pub use merge::{Merged, Merger};
pub use record::{Record, RecordBuilder};
pub use signal::Signal;

/// Base functionality for all commands.
#[cfg(not(feature = "display"))]
pub trait Command<R>: fmt::Debug + Send + Sync {
    /// Applies the command on the receiver and returns `Ok` if everything went fine,
    /// and `Err` if something went wrong.
    fn apply(&mut self, receiver: &mut R) -> Result<(), Box<dyn StdError + Send + Sync>>;

    /// Restores the state of the receiver as it was before the command was applied
    /// and returns `Ok` if everything went fine, and `Err` if something went wrong.
    fn undo(&mut self, receiver: &mut R) -> Result<(), Box<dyn StdError + Send + Sync>>;

    /// Reapplies the command on the receiver and return `Ok` if everything went fine,
    /// and `Err` if something went wrong.
    ///
    /// The default implementation uses the [`apply`] implementation.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    #[inline]
    fn redo(&mut self, receiver: &mut R) -> Result<(), Box<dyn StdError + Send + Sync>> {
        self.apply(receiver)
    }

    /// Used for automatic merging of commands.
    ///
    /// Two commands are merged together when a command is pushed, and it has
    /// the same id as the top command already on the stack or record. When commands are merged together,
    /// undoing and redoing them are done in one step.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # use undo::*;
    /// #[derive(Debug)]
    /// struct Add(char);
    ///
    /// impl Command<String> for Add {
    ///     fn apply(&mut self, s: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
    ///         s.push(self.0);
    ///         Ok(())
    ///     }
    ///
    ///     fn undo(&mut self, s: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
    ///         self.0 = s.pop().ok_or("`s` is empty")?;
    ///         Ok(())
    ///     }
    ///
    ///     fn merge(&self) -> Merge {
    ///         Merge::If(1)
    ///     }
    /// }
    ///
    /// fn main() -> Result<(), Box<dyn Error>> {
    ///     let mut record = Record::default();
    ///     // The `a`, `b`, and `c` commands are merged.
    ///     record.apply(Add('a'))?;
    ///     record.apply(Add('b'))?;
    ///     record.apply(Add('c'))?;
    ///     assert_eq!(record.len(), 1);
    ///     assert_eq!(record.as_receiver(), "abc");
    ///
    ///     // Calling `undo` once will undo all merged commands.
    ///     record.undo().unwrap()?;
    ///     assert_eq!(record.as_receiver(), "");
    ///
    ///     // Calling `redo` once will redo all merged commands.
    ///     record.redo().unwrap()?;
    ///     assert_eq!(record.as_receiver(), "abc");
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    fn merge(&self) -> Merge {
        Merge::Never
    }
}

#[cfg(feature = "display")]
pub trait Command<R>: fmt::Debug + fmt::Display + Send + Sync {
    fn apply(&mut self, receiver: &mut R) -> Result<(), Box<dyn StdError + Send + Sync>>;

    fn undo(&mut self, receiver: &mut R) -> Result<(), Box<dyn StdError + Send + Sync>>;

    #[inline]
    fn redo(&mut self, receiver: &mut R) -> Result<(), Box<dyn StdError + Send + Sync>> {
        self.apply(receiver)
    }

    #[inline]
    fn merge(&self) -> Merge {
        Merge::Never
    }
}

/// Says if the command should merge with another command.
#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum Merge {
    /// Always merges.
    Always,
    /// Merges if the two commands have the same value.
    If(u32),
    /// Never merges.
    Never,
}

struct Meta<R> {
    command: Box<dyn Command<R> + 'static>,
    timestamp: DateTime<Local>,
}

impl<R> Meta<R> {
    #[inline]
    fn new(command: impl Command<R> + 'static) -> Meta<R> {
        Meta {
            command: Box::new(command),
            timestamp: Local::now(),
        }
    }
}

impl<R> Command<R> for Meta<R> {
    #[inline]
    fn apply(&mut self, receiver: &mut R) -> Result<(), Box<dyn StdError + Send + Sync>> {
        self.command.apply(receiver)
    }

    #[inline]
    fn undo(&mut self, receiver: &mut R) -> Result<(), Box<dyn StdError + Send + Sync>> {
        self.command.undo(receiver)
    }

    #[inline]
    fn redo(&mut self, receiver: &mut R) -> Result<(), Box<dyn StdError + Send + Sync>> {
        self.command.redo(receiver)
    }

    #[inline]
    fn merge(&self) -> Merge {
        self.command.merge()
    }
}

impl<R> fmt::Debug for Meta<R> {
    #[inline]
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

/// An error which holds the command that caused it.
pub struct Error<R> {
    meta: Meta<R>,
    error: Box<dyn StdError + Send + Sync>,
}

impl<R> Error<R> {
    /// Returns a reference to the command that caused the error.
    #[inline]
    pub fn command(&self) -> &impl Command<R> {
        &self.meta
    }

    /// Returns the command that caused the error.
    #[inline]
    pub fn into_command(self) -> impl Command<R> {
        self.meta
    }
}

impl<R> fmt::Debug for Error<R> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Error")
            .field("meta", &self.meta)
            .field("error", &self.error)
            .finish()
    }
}

#[cfg(not(feature = "display"))]
impl<R> fmt::Display for Error<R> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.error as &dyn fmt::Display).fmt(f)
    }
}

#[cfg(feature = "display")]
impl<R> fmt::Display for Error<R> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "`{error}` caused by `{command}`",
            error = self.error,
            command = self.meta
        )
    }
}

impl<R> StdError for Error<R> {
    #[inline]
    fn description(&self) -> &str {
        self.error.description()
    }

    #[inline]
    fn cause(&self) -> Option<&dyn StdError> {
        self.error.cause()
    }
}
