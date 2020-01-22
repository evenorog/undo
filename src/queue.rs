use crate::{Checkpoint, Command, Record, Result};
use std::fmt;

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
/// #         self.0 = s.pop().ok_or("`s` is empty")?;
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
pub struct Queue<'a, T: 'static> {
    record: &'a mut Record<T>,
    actions: Vec<Action<T>>,
}

impl<'a, T> Queue<'a, T> {
    /// Reserves capacity for at least `additional` more commands in the queue.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    pub fn reserve(&mut self, additional: usize) {
        self.actions.reserve(additional);
    }

    /// Returns the capacity of the queue.
    pub fn capacity(&self) -> usize {
        self.actions.capacity()
    }

    /// Shrinks the capacity of the queue as much as possible.
    pub fn shrink_to_fit(&mut self) {
        self.actions.shrink_to_fit();
    }

    /// Returns the number of commands in the queue.
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Returns `true` if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Queues an `apply` action.
    pub fn apply(&mut self, command: impl Command<T>) {
        self.actions.push(Action::Apply(Box::new(command)));
    }

    /// Queues an `undo` action.
    pub fn undo(&mut self) {
        self.actions.push(Action::Undo);
    }

    /// Queues a `redo` action.
    pub fn redo(&mut self) {
        self.actions.push(Action::Redo);
    }

    /// Applies the actions that is queued.
    ///
    /// # Errors
    /// If an error occurs, it stops applying the actions and returns the error.
    pub fn commit(self) -> Result {
        for action in self.actions {
            match action {
                Action::Apply(command) => self.record.apply(command)?,
                Action::Undo => self.record.undo()?,
                Action::Redo => self.record.redo()?,
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
    fn from(inner: &'a mut Record<T>) -> Self {
        Queue {
            record: inner,
            actions: Vec::new(),
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for Queue<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Queue")
            .field("record", &self.record)
            .field("actions", &self.actions)
            .finish()
    }
}

enum Action<T> {
    Apply(Box<dyn Command<T>>),
    Undo,
    Redo,
}

impl<T> fmt::Debug for Action<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Action::Apply(_) => f.debug_struct("Apply").finish(),
            Action::Undo => f.debug_struct("Undo").finish(),
            Action::Redo => f.debug_struct("Redo").finish(),
        }
    }
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
            self.0 = s.pop().ok_or("`s` is empty")?;
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
