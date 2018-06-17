#![allow(dead_code)]

use std::collections::vec_deque::IntoIter;
use {Command, Error, Record};

/// A history of commands.
#[derive(Debug, Default)]
pub struct History<R> {
    record: Record<R>,
    branch: Vec<IntoIter<Box<Command<R> + 'static>>>,
}

impl<R> History<R> {
    /// Returns a new history.
    #[inline]
    pub fn new(_: impl Into<R>) -> History<R> {
        unimplemented!()
    }

    /// Pushes the command to the top of the history and executes its [`apply`] method.
    /// The command is merged with the previous top command if they have the same [`id`].
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned together with the command.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    /// [`id`]: trait.Command.html#method.id
    #[inline]
    pub fn apply(&mut self, _: impl Command<R> + 'static) -> Result<(), Error<R>>
    where
        R: 'static,
    {
        unimplemented!()
    }

    /// Calls the [`undo`] method for the active command and sets the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned together with the command.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result<(), Error<R>>> {
        unimplemented!()
    }

    /// Calls the [`redo`] method for the active command and sets the next one as the
    /// new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the error is returned together with the command.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result<(), Error<R>>> {
        unimplemented!()
    }
}

impl<R> From<R> for History<R> {
    #[inline]
    fn from(receiver: R) -> Self {
        History::new(receiver)
    }
}
