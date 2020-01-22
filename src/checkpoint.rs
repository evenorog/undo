use crate::{Command, Entry, Queue, Record, Result};
use std::collections::VecDeque;
#[cfg(feature = "display")]
use std::fmt;

/// A checkpoint wrapper.
///
/// Wraps a record or history and gives it checkpoint functionality.
/// This allows the record or history to cancel all changes made since creating the checkpoint.
///
/// # Examples
/// ```
/// # use undo::*;
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
pub struct Checkpoint<'a, T: 'static> {
    record: &'a mut Record<T>,
    actions: Vec<Action<T>>,
}

impl<T> Checkpoint<'_, T> {
    /// Reserves capacity for at least `additional` more commands in the checkpoint.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    pub fn reserve(&mut self, additional: usize) {
        self.actions.reserve(additional);
    }

    /// Returns the capacity of the checkpoint.
    pub fn capacity(&self) -> usize {
        self.actions.capacity()
    }

    /// Shrinks the capacity of the checkpoint as much as possible.
    pub fn shrink_to_fit(&mut self) {
        self.actions.shrink_to_fit();
    }

    /// Returns the number of commands in the checkpoint.
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Returns `true` if the checkpoint is empty.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Calls the [`apply`] method.
    ///
    /// [`apply`]: struct.Record.html#method.apply
    pub fn apply(&mut self, command: impl Command<T>) -> Result {
        let saved = self.record.saved;
        let (_, tail) = self.record.__apply(Entry::new(command))?;
        self.actions.push(Action::Apply(saved, tail));
        Ok(())
    }

    /// Calls the `undo` method.
    pub fn undo(&mut self) -> Option<Result> {
        let undo = self.record.undo();
        if let Some(Ok(_)) = undo {
            self.actions.push(Action::Undo);
        }
        undo
    }

    /// Calls the `redo` method.
    pub fn redo(&mut self) -> Option<Result> {
        let redo = self.record.redo();
        if let Some(Ok(_)) = redo {
            self.actions.push(Action::Redo);
        }
        redo
    }

    /// Commits the changes and consumes the checkpoint.
    pub fn commit(self) {}

    /// Cancels the changes and consumes the checkpoint.
    ///
    /// # Errors
    /// If an error occur when canceling the changes, the error is returned
    /// and the remaining commands are not canceled.
    pub fn cancel(self) -> Result {
        for action in self.actions.into_iter().rev() {
            match action {
                Action::Apply(saved, mut entries) => {
                    self.record.undo().unwrap()?;
                    self.record.entries.pop_back();
                    self.record.entries.append(&mut entries);
                    self.record.saved = saved;
                }
                Action::Undo => self.record.redo().unwrap()?,
                Action::Redo => self.record.undo().unwrap()?,
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
            actions: Vec::new(),
        }
    }
}

#[cfg(feature = "display")]
impl<T: fmt::Debug> fmt::Debug for Checkpoint<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Checkpoint")
            .field("record", &self.record)
            .field("actions", &self.actions)
            .finish()
    }
}

enum Action<T> {
    Apply(Option<usize>, VecDeque<Entry<T>>),
    Undo,
    Redo,
}

#[cfg(feature = "display")]
impl<T> fmt::Debug for Action<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Action::Apply(_, _) => f.debug_struct("Apply").finish(),
            Action::Undo => f.debug_struct("Undo").finish(),
            Action::Redo => f.debug_struct("Redo").finish(),
        }
    }
}

#[cfg(all(test, not(feature = "display")))]
mod tests {
    use crate::*;

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
        record.undo().unwrap().unwrap();
        record.undo().unwrap().unwrap();
        record.undo().unwrap().unwrap();
        let mut cp = record.checkpoint();
        cp.apply(Add('d')).unwrap();
        cp.apply(Add('e')).unwrap();
        cp.apply(Add('f')).unwrap();
        assert_eq!(cp.target(), "def");
        cp.cancel().unwrap();
        assert_eq!(record.target(), "");
        record.redo().unwrap().unwrap();
        record.redo().unwrap().unwrap();
        record.redo().unwrap().unwrap();
        assert!(record.is_saved());
        assert_eq!(record.target(), "abc");
    }
}
