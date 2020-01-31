use crate::{Command, Entry, Queue, Record, Result};
use std::collections::VecDeque;

/// A checkpoint wrapper.
///
/// Wraps a record or history and gives it checkpoint functionality.
/// This allows the record or history to cancel all changes made since creating the checkpoint.
///
/// # Examples
/// ```
/// # use undo::*;
/// # #[derive(Debug)]
/// # struct Add(char);
/// # impl Command<String> for Add {
/// #     fn apply(&mut self, s: &mut String) -> undo::Result {
/// #         s.push(self.0);
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self, s: &mut String) -> undo::Result {
/// #         self.0 = s.pop().ok_or("s is empty")?;
/// #         Ok(())
/// #     }
/// # }
/// # fn main() -> undo::Result {
/// let mut record = Record::default();
/// let mut cp = record.checkpoint();
/// cp.apply(Add('a'))?;
/// cp.apply(Add('b'))?;
/// cp.apply(Add('c'))?;
/// assert_eq!(cp.target(), "abc");
/// cp.cancel()?;
/// assert_eq!(record.target(), "");
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Checkpoint<'a, T: 'static> {
    record: &'a mut Record<T>,
    commands: Vec<CheckpointCommand<T>>,
}

impl<T> Checkpoint<'_, T> {
    /// Calls the [`apply`] method.
    ///
    /// [`apply`]: struct.Record.html#method.apply
    pub fn apply(&mut self, command: impl Command<T>) -> Result {
        let saved = self.record.saved;
        let (_, tail) = self.record.__apply(command)?;
        self.commands.push(CheckpointCommand::Apply(saved, tail));
        Ok(())
    }

    /// Calls the `undo` method.
    pub fn undo(&mut self) -> Result {
        if self.record.can_undo() {
            self.record.undo()?;
            self.commands.push(CheckpointCommand::Undo);
        }
        Ok(())
    }

    /// Calls the `redo` method.
    pub fn redo(&mut self) -> Result {
        if self.record.can_redo() {
            self.record.redo()?;
            self.commands.push(CheckpointCommand::Redo);
        }
        Ok(())
    }

    /// Commits the changes and consumes the checkpoint.
    pub fn commit(self) {}

    /// Cancels the changes and consumes the checkpoint.
    ///
    /// # Errors
    /// If an error occur when canceling the changes, the error is returned
    /// and the remaining commands are not canceled.
    pub fn cancel(self) -> Result {
        for command in self.commands.into_iter().rev() {
            match command {
                CheckpointCommand::Apply(saved, mut entries) => {
                    self.record.undo()?;
                    self.record.entries.pop_back();
                    self.record.entries.append(&mut entries);
                    self.record.saved = saved;
                }
                CheckpointCommand::Undo => self.record.redo()?,
                CheckpointCommand::Redo => self.record.undo()?,
            }
        }
        Ok(())
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<T> {
        self.record.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<T> {
        self.record.checkpoint()
    }

    /// Returns a reference to the `target`.
    pub fn target(&self) -> &T {
        self.record.target()
    }
}

impl<'a, T> From<&'a mut Record<T>> for Checkpoint<'a, T> {
    fn from(record: &'a mut Record<T>) -> Self {
        Checkpoint {
            record,
            commands: Vec::new(),
        }
    }
}

#[derive(Debug)]
enum CheckpointCommand<T> {
    Apply(Option<usize>, VecDeque<Entry<T>>),
    Undo,
    Redo,
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[derive(Debug)]
    struct Add(char);

    impl Command<String> for Add {
        fn apply(&mut self, s: &mut String) -> Result {
            s.push(self.0);
            Ok(())
        }

        fn undo(&mut self, s: &mut String) -> Result {
            self.0 = s.pop().ok_or("s is empty")?;
            Ok(())
        }
    }

    #[test]
    fn commit() {
        let mut record = Record::default();
        let mut cp1 = record.checkpoint();
        cp1.apply(Add('a')).unwrap();
        cp1.apply(Add('b')).unwrap();
        cp1.apply(Add('c')).unwrap();
        assert_eq!(cp1.target(), "abc");
        let mut cp2 = cp1.checkpoint();
        cp2.apply(Add('d')).unwrap();
        cp2.apply(Add('e')).unwrap();
        cp2.apply(Add('f')).unwrap();
        assert_eq!(cp2.target(), "abcdef");
        let mut cp3 = cp2.checkpoint();
        cp3.apply(Add('g')).unwrap();
        cp3.apply(Add('h')).unwrap();
        cp3.apply(Add('i')).unwrap();
        assert_eq!(cp3.target(), "abcdefghi");
        cp3.commit();
        cp2.commit();
        cp1.commit();
        assert_eq!(record.target(), "abcdefghi");
    }

    #[test]
    fn cancel() {
        let mut record = Record::default();
        let mut cp1 = record.checkpoint();
        cp1.apply(Add('a')).unwrap();
        cp1.apply(Add('b')).unwrap();
        cp1.apply(Add('c')).unwrap();
        let mut cp2 = cp1.checkpoint();
        cp2.apply(Add('d')).unwrap();
        cp2.apply(Add('e')).unwrap();
        cp2.apply(Add('f')).unwrap();
        let mut cp3 = cp2.checkpoint();
        cp3.apply(Add('g')).unwrap();
        cp3.apply(Add('h')).unwrap();
        cp3.apply(Add('i')).unwrap();
        assert_eq!(cp3.target(), "abcdefghi");
        cp3.cancel().unwrap();
        assert_eq!(cp2.target(), "abcdef");
        cp2.cancel().unwrap();
        assert_eq!(cp1.target(), "abc");
        cp1.cancel().unwrap();
        assert_eq!(record.target(), "");
    }

    #[test]
    fn saved() {
        let mut record = Record::default();
        record.apply(Add('a')).unwrap();
        record.apply(Add('b')).unwrap();
        record.apply(Add('c')).unwrap();
        record.set_saved(true);
        record.undo().unwrap();
        record.undo().unwrap();
        record.undo().unwrap();
        let mut cp = record.checkpoint();
        cp.apply(Add('d')).unwrap();
        cp.apply(Add('e')).unwrap();
        cp.apply(Add('f')).unwrap();
        assert_eq!(cp.target(), "def");
        cp.cancel().unwrap();
        assert_eq!(record.target(), "");
        record.redo().unwrap();
        record.redo().unwrap();
        record.redo().unwrap();
        assert!(record.is_saved());
        assert_eq!(record.target(), "abc");
    }
}
