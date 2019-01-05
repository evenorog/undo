//! Provides undo-redo functionality with dynamic dispatch and automatic command merging.
//!
//! # Contents
//!
//! * [Record] provides a stack based undo-redo functionality.
//! * [History] provides a tree based undo-redo functionality that allows you to jump between different branches.
//! * [Queue] wraps a [Record] or [History] and provides batch queue functionality.
//! * [Checkpoint] wraps a [Record] or [History] and provides checkpoint functionality.
//! * Commands can be merged using the [merge!] macro or the [merge] method.
//!   When two commands are merged, undoing and redoing them are done in a single step.
//! * Configurable display formatting is provided when the `display` feature is enabled.
//! * Time stamps and time travel is provided when the `chrono` feature is enabled.
//!
//! # Examples
//!
//! Add this to `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! undo = "0.30"
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
//! [Record]: struct.Record.html
//! [History]: struct.History.html
//! [Queue]: struct.Queue.html
//! [Checkpoint]: struct.Checkpoint.html
//! [merge!]: macro.merge.html
//! [merge]: trait.Command.html#method.merge

#![doc(html_root_url = "https://docs.rs/undo/0.30.0")]
#![deny(
    bad_style,
    bare_trait_objects,
    missing_debug_implementations,
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
use std::fmt;

pub use self::checkpoint::Checkpoint;
#[cfg(feature = "display")]
pub use self::display::Display;
pub use self::history::{History, HistoryBuilder};
pub use self::merge::Merged;
pub use self::queue::Queue;
pub use self::record::{Record, RecordBuilder};

/// A specialized Result type for undo-redo operations.
pub type Result = std::result::Result<(), Box<dyn Error + Send + Sync>>;

/// Base functionality for all commands.
#[cfg(not(feature = "display"))]
pub trait Command<R>: fmt::Debug + Send + Sync {
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
    /// # use undo::{Command, Merge, Record};
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
    ///         Merge::Always
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
        Merge::Never
    }
}

/// Base functionality for all commands.
#[cfg(feature = "display")]
pub trait Command<R>: fmt::Debug + fmt::Display + Send + Sync {
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
    ///         Merge::Always
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
        Merge::Never
    }
}

/// The signal sent when the record, the history, or the receiver changes.
///
/// When one of these states changes, they will send a corresponding signal to the user.
/// For example, if the record can no longer redo any commands, it sends a `Redo(false)`
/// signal to tell the user.
///
/// # Examples
/// ```
/// # use undo::{History, Signal};
/// # fn foo() -> History<String> {
/// let history = History::builder()
///     .connect(|signal| match signal {
///         Signal::Undo(on) => println!("undo: {}", on),
///         Signal::Redo(on) => println!("redo: {}", on),
///         Signal::Saved(on) => println!("saved: {}", on),
///         Signal::Cursor { old, new } => println!("cursor: {} -> {}", old, new),
///         Signal::Root { old, new } => println!("root: {} -> {}", old, new),
///     })
///     .default();
/// # history
/// # }
/// ```
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
    /// This signal will be emitted when the cursor has changed. This includes
    /// when two commands have been merged, in which case `old == new`.
    Cursor {
        /// The old cursor.
        old: usize,
        /// The new cursor.
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
    Always,
    /// Merges if the two commands have the same value.
    If(u32),
    /// Never merges.
    Never,
}

/// A position in a history tree.
#[derive(Copy, Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
struct At {
    branch: usize,
    cursor: usize,
}

struct Meta<R> {
    command: Box<dyn Command<R> + 'static>,
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

impl<R> From<Box<dyn Command<R> + 'static>> for Meta<R> {
    #[inline]
    fn from(command: Box<dyn Command<R> + 'static>) -> Self {
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
}

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
