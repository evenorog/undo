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
//! Commands can be merged using the [`merge!`] macro or the [`id`] method.
//! When two commands are merged, undoing and redoing them are done in a single step.
//!
//! [command pattern]: https://en.wikipedia.org/wiki/Command_pattern
//! [Record]: struct.Record.html
//! [History]: struct.History.html
//! [`merge!`]: macro.merge.html
//! [`id`]: trait.Command.html#method.id

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
#[cfg(feature = "display")]
extern crate colored;
extern crate fnv;

#[cfg(feature = "display")]
mod display;
mod history;
mod merge;
mod record;
mod signal;

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
    ///     fn id(&self) -> Option<u32> {
    ///         Some(1)
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
    fn id(&self) -> Option<u32> {
        None
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
    fn id(&self) -> Option<u32> {
        None
    }
}

impl<R, C: Command<R> + ?Sized> Command<R> for Box<C> {
    #[inline]
    fn apply(&mut self, receiver: &mut R) -> Result<(), Box<dyn StdError + Send + Sync>> {
        (**self).apply(receiver)
    }

    #[inline]
    fn undo(&mut self, receiver: &mut R) -> Result<(), Box<dyn StdError + Send + Sync>> {
        (**self).undo(receiver)
    }

    #[inline]
    fn redo(&mut self, receiver: &mut R) -> Result<(), Box<dyn StdError + Send + Sync>> {
        (**self).redo(receiver)
    }

    #[inline]
    fn id(&self) -> Option<u32> {
        (**self).id()
    }
}

/// An error which holds the command that caused it.
pub struct Error<R>(
    pub Box<dyn Command<R> + 'static>,
    pub Box<dyn StdError + Send + Sync>,
);

impl<R> fmt::Debug for Error<R> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Error")
            .field(&self.0)
            .field(&self.1)
            .finish()
    }
}

#[cfg(not(feature = "display"))]
impl<R> fmt::Display for Error<R> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.1 as &dyn fmt::Display).fmt(f)
    }
}

#[cfg(feature = "display")]
impl<R> fmt::Display for Error<R> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "`{error}` caused by `{command}`",
            error = self.1,
            command = self.0
        )
    }
}

impl<R> StdError for Error<R> {
    #[inline]
    fn description(&self) -> &str {
        self.1.description()
    }

    #[inline]
    fn cause(&self) -> Option<&dyn StdError> {
        self.1.cause()
    }
}
