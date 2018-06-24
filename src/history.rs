use std::collections::HashMap;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::ops::RangeFrom;
use {Command, Error, Record};

const ORIGIN: usize = 0;

/// A history of commands.
#[derive(Debug)]
pub struct History<R> {
    id: usize,
    parent: usize,
    next: RangeFrom<usize>,
    record: Record<R>,
    branches: HashMap<usize, Branch<R>>,
}

impl<R> History<R> {
    /// Returns a new history.
    #[inline]
    pub fn new(receiver: impl Into<R>) -> History<R> {
        History {
            id: ORIGIN,
            parent: ORIGIN,
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
        let cursor = self.record.cursor();
        let commands = self.record.apply(cmd)?;
        let commands: Vec<_> = commands.collect();
        if !commands.is_empty() {
            let id = self.next.next().unwrap();
            self.branches.insert(
                id,
                Branch {
                    parent: self.id,
                    cursor,
                    commands,
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
    pub fn jump_to(&mut self, mut branch: usize, cursor: usize) -> Option<Result<(), Error<R>>>
    where
        R: 'static,
    {
        if self.id == branch {
            return self.record.jump_to(cursor);
        }

        // Find the path from `dest` to `ORIGIN`.
        let mut dest = self.branches.remove(&branch)?;
        branch = dest.parent;
        let mut path = vec![dest];
        while branch != ORIGIN {
            dest = self.branches.remove(&branch).unwrap();
            path.push(dest);
        }

        // Find the path from `start` to `ORIGIN`.
        if self.id != ORIGIN {
            let mut start = self.branches.remove(&self.parent).unwrap();
            branch = start.parent;
            let mut tmp = vec![start];
            while branch != ORIGIN {
                start = self.branches.remove(&branch).unwrap();
                tmp.push(start);
            }
            tmp.reverse();
            path.append(&mut tmp);
        }

        // Walk the path from `start` to `dest`.
        while let Some(dest) = path.pop() {
            // Move to `dest.cursor` either by undoing or redoing.
            if let Err(err) = self.record.jump_to(dest.cursor).unwrap() {
                return Some(Err(err));
            }
            // Apply the commands in the branch and move older commands into their own branch.
            for cmd in dest.commands {
                if let Err(err) = self.apply(cmd) {
                    return Some(Err(err));
                }
            }
        }

        Some(Ok(()))
    }
}

impl<R: Default> Default for History<R> {
    #[inline]
    fn default() -> History<R> {
        History::new(R::default())
    }
}

impl<R> From<R> for History<R> {
    #[inline]
    fn from(receiver: R) -> Self {
        History::new(receiver)
    }
}

struct Branch<R> {
    parent: usize,
    cursor: usize,
    commands: Vec<Box<dyn Command<R> + 'static>>,
}

impl<R> Debug for Branch<R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Branch")
            .field("parent", &self.parent)
            .field("cursor", &self.cursor)
            .field("commands", &self.commands)
            .finish()
    }
}

/// Builder for a history.
#[derive(Debug)]
pub struct HistoryBuilder<R> {
    receiver: PhantomData<R>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[derive(Debug)]
    struct Add(char);

    impl Command<String> for Add {
        fn apply(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error>> {
            receiver.push(self.0);
            Ok(())
        }

        fn undo(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error>> {
            self.0 = receiver.pop().ok_or("`receiver` is empty")?;
            Ok(())
        }
    }

    #[test]
    fn jump_to() {
        let mut history = History::default();
        history.apply(Add('a')).unwrap();
        history.apply(Add('b')).unwrap();
        history.apply(Add('c')).unwrap();
        history.apply(Add('d')).unwrap();
        history.apply(Add('e')).unwrap();

        history.undo().unwrap().unwrap();
        history.undo().unwrap().unwrap();

        let b = history.apply(Add('f')).unwrap().unwrap();
        history.apply(Add('g')).unwrap();

        history.jump_to(b, 5).unwrap().unwrap();
    }
}
