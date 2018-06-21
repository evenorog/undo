use std::marker::PhantomData;
use {Command, Error, Record};

#[derive(Debug)]
struct Branch<R> {
    parent: usize,
    cursor: usize,
    commands: Box<[Box<Command<R> + 'static>]>,
}

/// A history of commands.
#[derive(Debug)]
pub struct History<R> {
    id: usize,
    record: Record<R>,
    branches: Vec<Branch<R>>,
}

impl<R> History<R> {
    /// Returns a new history.
    #[inline]
    pub fn new(receiver: impl Into<R>) -> History<R> {
        History {
            id: 0,
            record: Record::new(receiver),
            branches: Vec::new(),
        }
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
    pub fn apply(&mut self, cmd: impl Command<R> + 'static) -> Result<(), Error<R>>
    where
        R: 'static,
    {
        let commands = self.record.apply(cmd)?;
        let commands: Vec<_> = commands.collect();
        if !commands.is_empty() {
            self.branches.push(Branch {
                parent: self.id,
                cursor: self.record.cursor(),
                commands: commands.into_boxed_slice(),
            });
        }
        Ok(())
    }

    /// Calls the [`undo`] method for the active command and sets the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned together with the command.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result<(), Error<R>>> {
        self.record.undo()
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
        self.record.redo()
    }
}

impl<R> From<R> for History<R> {
    #[inline]
    fn from(receiver: R) -> Self {
        History::new(receiver)
    }
}

/// Builder for a history.
#[allow(missing_debug_implementations)]
pub struct HistoryBuilder<R> {
    receiver: PhantomData<R>,
}
