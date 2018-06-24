use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::RangeFrom;
use {Command, Error, Record};

#[derive(Debug)]
struct Branch<R> {
    parent: usize,
    cursor: usize,
    commands: Box<[Box<dyn Command<R> + 'static>]>,
}

/// A history of commands.
#[derive(Debug)]
pub struct History<R> {
    id: usize,
    next: RangeFrom<usize>,
    record: Record<R>,
    branches: HashMap<usize, Branch<R>>,
}

impl<R> History<R> {
    /// Returns a new history.
    #[inline]
    pub fn new(receiver: impl Into<R>) -> History<R> {
        History {
            id: 0,
            next: 1..,
            record: Record::new(receiver),
            branches: HashMap::new(),
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
    pub fn apply(&mut self, cmd: impl Command<R> + 'static) -> Result<Option<usize>, Error<R>>
    where
        R: 'static,
    {
        let commands = self.record.apply(cmd)?;
        let commands: Vec<_> = commands.collect();
        if !commands.is_empty() {
            let id = self.next.next().unwrap();
            self.branches.insert(
                id,
                Branch {
                    parent: self.id,
                    cursor: self.record.cursor(),
                    commands: commands.into_boxed_slice(),
                },
            );
            Ok(Some(id))
        } else {
            Ok(None)
        }
    }

    /// Calls the [`undo`] method for the active command and sets the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned together with the command.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    #[inline]
    #[must_use]
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
    #[must_use]
    pub fn redo(&mut self) -> Option<Result<(), Error<R>>> {
        self.record.redo()
    }

    /// Jumps to the command in `branch` at `cursor`.
    #[inline]
    #[must_use]
    pub fn jump_to(&mut self, branch: usize, cursor: usize) -> Option<Result<(), Error<R>>> {
        if self.id == branch {
            return self.record.set_cursor(cursor);
        }

        let mut dst = self.branches.get(&branch)?;
        let mut path = vec![dst];
        while dst.parent != self.id {
            dst = &self.branches[&dst.parent];
            path.push(dst);
        }

        while let Some(last) = path.pop() {
            let _ = last.parent;
            let _ = last.cursor;
        }

        unimplemented!()
    }
}

impl<R> From<R> for History<R> {
    #[inline]
    fn from(receiver: R) -> Self {
        History::new(receiver)
    }
}

/// Builder for a history.
#[derive(Debug)]
pub struct HistoryBuilder<R> {
    receiver: PhantomData<R>,
}
