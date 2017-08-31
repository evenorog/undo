//! An undo/redo library with dynamic dispatch and automatic command merging.
//! It uses the [Command Pattern](https://en.wikipedia.org/wiki/Command_pattern)
//! where the user modifies a receiver by applying `Command`s on it.
//!
//! The library has currently two data structures that can be used to modify the receiver:
//!
//! * A simple `Stack` that pushes and pops commands to modify the receiver.
//! * A `Record` that can roll the state of the receiver forwards and backwards.

#![forbid(unstable_features, bad_style)]
#![deny(missing_debug_implementations,
        unused_import_braces,
        unused_qualifications)]

mod group;
pub mod record;
mod stack;

use std::error;
use std::fmt::{self, Debug, Display, Formatter};

pub use group::Group;
pub use record::Record;
pub use stack::Stack;

/// Base functionality for all commands.
pub trait Command<R> {
    /// Executes the desired command and returns `Ok` if everything went fine, and `Err` if
    /// something went wrong.
    fn redo(&mut self, receiver: &mut R) -> Result<(), Box<error::Error>>;

    /// Restores the state as it was before [`redo`] was called and returns `Ok` if everything
    /// went fine, and `Err` if something went wrong.
    ///
    /// [`redo`]: trait.Command.html#tymethod.redo
    fn undo(&mut self, receiver: &mut R) -> Result<(), Box<error::Error>>;

    /// Used for automatic merging of `Command`s.
    ///
    /// Two commands are merged together when a command is pushed, and it has
    /// the same id as the top command already on the stack. When commands are merged together,
    /// undoing and redoing them are done in one step.
    #[inline]
    fn id(&self) -> Option<u32> {
        None
    }
}

impl<R> Debug for Command<R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self.id() {
            Some(id) => write!(f, "{}", id),
            None => write!(f, "_"),
        }
    }
}

impl<R> Command<R> for Box<Command<R>> {
    #[inline]
    fn redo(&mut self, receiver: &mut R) -> Result<(), Box<error::Error>> {
        (**self).redo(receiver)
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

/// An error kind that holds the error and the command that caused the error.
#[derive(Debug)]
pub struct Error<R>(pub Box<Command<R>>, pub Box<error::Error>);

impl<R> Display for Error<R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.1)
    }
}

impl<R> error::Error for Error<R>
    where R: Debug
{
    #[inline]
    fn description(&self) -> &str {
        self.1.description()
    }

    #[inline]
    fn cause(&self) -> Option<&error::Error> {
        self.1.cause()
    }
}

struct Merger<R> {
    cmd1: Box<Command<R>>,
    cmd2: Box<Command<R>>,
}

impl<R> Command<R> for Merger<R> {
    #[inline]
    fn redo(&mut self, receiver: &mut R) -> Result<(), Box<error::Error>> {
        self.cmd1.redo(receiver)?;
        self.cmd2.redo(receiver)
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
