use crate::{Checkpoint, Command, Entry, Record, Result};

/// A command queue wrapper.
///
/// Wraps a record or history and gives it batch queue functionality.
/// This allows the record or history to queue up commands and either cancel or apply them later.
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
/// let mut queue = record.queue();
/// queue.apply(Add('a'));
/// queue.apply(Add('b'));
/// queue.apply(Add('c'));
/// assert_eq!(queue.target(), "");
/// queue.commit()?;
/// assert_eq!(record.target(), "abc");
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Queue<'a, T: 'static> {
    record: &'a mut Record<T>,
    commands: Vec<QueueCommand<T>>,
}

impl<'a, T> Queue<'a, T> {
    /// Queues an `apply` action.
    pub fn apply(&mut self, command: impl Command<T>) {
        self.commands.push(QueueCommand::Apply(Entry::new(command)));
    }

    /// Queues an `undo` action.
    pub fn undo(&mut self) {
        self.commands.push(QueueCommand::Undo);
    }

    /// Queues a `redo` action.
    pub fn redo(&mut self) {
        self.commands.push(QueueCommand::Redo);
    }

    /// Applies the actions that is queued.
    ///
    /// # Errors
    /// If an error occurs, it stops applying the actions and returns the error.
    pub fn commit(self) -> Result {
        for command in self.commands {
            match command {
                QueueCommand::Apply(entry) => {
                    self.record.__apply(entry)?;
                }
                QueueCommand::Undo => self.record.undo()?,
                QueueCommand::Redo => self.record.redo()?,
            }
        }
        Ok(())
    }

    /// Cancels the queued actions.
    pub fn cancel(self) {}

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

impl<'a, T> From<&'a mut Record<T>> for Queue<'a, T> {
    fn from(record: &'a mut Record<T>) -> Self {
        Queue {
            record,
            commands: Vec::new(),
        }
    }
}

#[derive(Debug)]
enum QueueCommand<T> {
    Apply(Entry<T>),
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
        let mut q1 = record.queue();
        q1.redo();
        q1.redo();
        q1.redo();
        let mut q2 = q1.queue();
        q2.undo();
        q2.undo();
        q2.undo();
        let mut q3 = q2.queue();
        q3.apply(Add('a'));
        q3.apply(Add('b'));
        q3.apply(Add('c'));
        assert_eq!(q3.target(), "");
        q3.commit().unwrap();
        assert_eq!(q2.target(), "abc");
        q2.commit().unwrap();
        assert_eq!(q1.target(), "");
        q1.commit().unwrap();
        assert_eq!(record.target(), "abc");
    }
}
