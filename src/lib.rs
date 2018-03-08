//! An undo-redo library with dynamic dispatch and automatic command merging.
//! It uses the [command pattern](https://en.wikipedia.org/wiki/Command_pattern)
//! where the user modifies a receiver by applying commands on it.
//!
//! The library has currently two data structures that can be used to modify the receiver:
//!
//! * A stack that can push and pop commands to modify the receiver.
//! * A record that can roll the state of the receiver forwards and backwards.

#![forbid(unstable_features, bad_style)]
#![deny(missing_debug_implementations, unused_import_braces, unused_qualifications, unsafe_code)]

mod group;
mod record;
mod stack;

use std::error;
use std::fmt::{self, Debug, Display, Formatter};

pub use group::{Group, GroupBuilder};
pub use record::{Commands, Record, RecordBuilder, Signal};
pub use stack::Stack;

/// Base functionality for all commands.
#[cfg(not(feature = "display"))]
pub trait Command<R>: Debug + Send + Sync {
    /// Executes the desired command and returns `Ok` if everything went fine, and `Err` if
    /// something went wrong.
    fn exec(&mut self, receiver: &mut R) -> Result<(), Box<error::Error>>;

    /// Restores the state as it was before [`exec`] was called and returns `Ok` if everything
    /// went fine, and `Err` if something went wrong.
    ///
    /// [`exec`]: trait.Command.html#tymethod.exec
    fn undo(&mut self, receiver: &mut R) -> Result<(), Box<error::Error>>;

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
    ///     fn exec(&mut self, s: &mut String) -> Result<(), Box<Error>> {
    ///         s.push(self.0);
    ///         Ok(())
    ///     }
    ///
    ///     fn undo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
    ///         self.0 = s.pop().ok_or("`String` is unexpectedly empty")?;
    ///         Ok(())
    ///     }
    ///
    ///     fn id(&self) -> Option<u32> {
    ///         Some(1)
    ///     }
    /// }
    ///
    /// fn foo() -> Result<(), Box<Error>> {
    ///     let mut record = Record::default();
    ///
    ///     // 'a', 'b', and 'c' are merged.
    ///     record.exec(Add('a'))?;
    ///     record.exec(Add('b'))?;
    ///     record.exec(Add('c'))?;
    ///     assert_eq!(record.len(), 1);
    ///     assert_eq!(record.as_receiver(), "abc");
    ///
    ///     // Calling `undo` once will undo all merged commands.
    ///     record.undo().unwrap()?;
    ///     assert_eq!(record.as_receiver(), "");
    ///
    ///     // Calling `redo` once will redo all merged commands.
    ///     record.redo().unwrap()?;
    ///     assert_eq!(record.into_receiver(), "abc");
    ///
    ///     Ok(())
    /// }
    /// # foo().unwrap();
    /// ```
    #[inline]
    fn id(&self) -> Option<u32> {
        None
    }
}

#[cfg(feature = "display")]
pub trait Command<R>: Debug + Display + Send + Sync {
    fn exec(&mut self, receiver: &mut R) -> Result<(), Box<error::Error>>;

    fn undo(&mut self, receiver: &mut R) -> Result<(), Box<error::Error>>;

    #[inline]
    fn id(&self) -> Option<u32> {
        None
    }
}

impl<R, C: Command<R> + ?Sized> Command<R> for Box<C> {
    #[inline]
    fn exec(&mut self, receiver: &mut R) -> Result<(), Box<error::Error>> {
        (**self).exec(receiver)
    }

    #[inline]
    fn undo(&mut self, receiver: &mut R) -> Result<(), Box<error::Error>> {
        (**self).undo(receiver)
    }

    #[inline]
    fn id(&self) -> Option<u32> {
        (**self).id()
    }
}

struct Merger<R> {
    cmd1: Box<Command<R>>,
    cmd2: Box<Command<R>>,
}

impl<R> Command<R> for Merger<R> {
    #[inline]
    fn exec(&mut self, receiver: &mut R) -> Result<(), Box<error::Error>> {
        self.cmd1.exec(receiver)?;
        self.cmd2.exec(receiver)
    }

    #[inline]
    fn undo(&mut self, receiver: &mut R) -> Result<(), Box<error::Error>> {
        self.cmd2.undo(receiver)?;
        self.cmd1.undo(receiver)
    }

    #[inline]
    fn id(&self) -> Option<u32> {
        self.cmd1.id()
    }
}

impl<R> Debug for Merger<R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Merger")
            .field("cmd1", &self.cmd1)
            .field("cmd2", &self.cmd2)
            .finish()
    }
}

#[cfg(feature = "display")]
impl<R> Display for Merger<R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{} + {}", self.cmd1, self.cmd2)
    }
}

/// The error type.
///
/// The error contains the error itself and the command that caused the error.
#[derive(Debug)]
pub struct Error<R>(pub Box<Command<R>>, pub Box<error::Error + Send + Sync>);

impl<R> Display for Error<R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.1)
    }
}

impl<R: Debug> error::Error for Error<R> {
    #[inline]
    fn description(&self) -> &str {
        self.1.description()
    }

    #[inline]
    fn cause(&self) -> Option<&error::Error> {
        self.1.cause()
    }
}
