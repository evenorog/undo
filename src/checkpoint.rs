use crate::{Command, Entry, History, Queue, Record, Result, Timeline};
use std::collections::VecDeque;

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
#[cfg_attr(feature = "display", derive(Debug))]
pub struct Checkpoint<'a, T: Timeline> {
    inner: &'a mut T,
    actions: Vec<Action<T::Target>>,
}

impl<'a, T: Timeline> Checkpoint<'a, T> {
    /// Reserves capacity for at least `additional` more commands in the checkpoint.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.actions.reserve(additional);
    }

    /// Returns the capacity of the checkpoint.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.actions.capacity()
    }

    /// Shrinks the capacity of the checkpoint as much as possible.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.actions.shrink_to_fit();
    }

    /// Returns the number of commands in the checkpoint.
    #[inline]
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Returns `true` if the checkpoint is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Calls the `undo` method.
    #[inline]
    pub fn undo(&mut self) -> Option<Result> {
        let undo = self.inner.undo();
        if let Some(Ok(_)) = undo {
            self.actions.push(Action::Undo);
        }
        undo
    }

    /// Calls the `redo` method.
    #[inline]
    pub fn redo(&mut self) -> Option<Result> {
        let redo = self.inner.redo();
        if let Some(Ok(_)) = redo {
            self.actions.push(Action::Redo);
        }
        redo
    }

    /// Commits the changes and consumes the checkpoint.
    #[inline]
    pub fn commit(self) {}
}

impl<T> Checkpoint<'_, Record<T>> {
    /// Calls the [`apply`] method.
    ///
    /// [`apply`]: struct.Record.html#method.apply
    #[inline]
    pub fn apply(&mut self, command: impl Command<T>) -> Result {
        let saved = self.inner.saved;
        let (_, tail) = self.inner.__apply(Entry::new(command))?;
        self.actions.push(Action::Apply(saved, tail));
        Ok(())
    }

    /// Cancels the changes and consumes the checkpoint.
    ///
    /// # Errors
    /// If an error occur when canceling the changes, the error is returned
    /// and the remaining commands are not canceled.
    #[inline]
    pub fn cancel(self) -> Result {
        for action in self.actions.into_iter().rev() {
            match action {
                Action::Apply(saved, mut entries) => {
                    self.inner.undo().unwrap()?;
                    self.inner.entries.pop_back();
                    self.inner.entries.append(&mut entries);
                    self.inner.saved = saved;
                }
                Action::Branch(_, _) => unreachable!(),
                Action::Undo => self.inner.redo().unwrap()?,
                Action::Redo => self.inner.undo().unwrap()?,
            }
        }
        Ok(())
    }

    /// Returns a queue.
    #[inline]
    pub fn queue(&mut self) -> Queue<Record<T>> {
        self.inner.queue()
    }

    /// Returns a checkpoint.
    #[inline]
    pub fn checkpoint(&mut self) -> Checkpoint<Record<T>> {
        self.inner.checkpoint()
    }

    /// Returns a reference to the `target`.
    #[inline]
    pub fn target(&self) -> &T {
        self.inner.target()
    }
}

impl<T> Checkpoint<'_, History<T>> {
    /// Calls the [`apply`] method.
    ///
    /// [`apply`]: struct.History.html#method.apply
    #[inline]
    pub fn apply(&mut self, command: impl Command<T>) -> Result {
        let branch = self.inner.branch();
        let current = self.inner.current();
        self.inner.apply(command)?;
        self.actions.push(Action::Branch(branch, current));
        Ok(())
    }

    /// Cancels the changes and consumes the checkpoint.
    ///
    /// # Errors
    /// If an error occur when canceling the changes, the error is returned
    /// and the remaining commands are not canceled.
    #[inline]
    pub fn cancel(self) -> Result {
        for action in self.actions.into_iter().rev() {
            match action {
                Action::Apply(_, _) => unreachable!(),
                Action::Branch(branch, current) => {
                    let root = self.inner.branch();
                    self.inner.go_to(branch, current).unwrap()?;
                    if root == branch {
                        self.inner.record.entries.pop_back();
                    } else {
                        self.inner.branches.remove(&root).unwrap();
                    }
                }
                Action::Undo => self.inner.redo().unwrap()?,
                Action::Redo => self.inner.undo().unwrap()?,
            }
        }
        Ok(())
    }

    /// Returns a queue.
    #[inline]
    pub fn queue(&mut self) -> Queue<History<T>> {
        self.inner.queue()
    }

    /// Returns a checkpoint.
    #[inline]
    pub fn checkpoint(&mut self) -> Checkpoint<History<T>> {
        self.inner.checkpoint()
    }

    /// Returns a reference to the `target`.
    #[inline]
    pub fn target(&self) -> &T {
        self.inner.target()
    }
}

impl<T> Timeline for Checkpoint<'_, Record<T>> {
    type Target = T;

    #[inline]
    fn apply(&mut self, command: impl Command<T>) -> Result {
        self.apply(command)
    }

    #[inline]
    fn undo(&mut self) -> Option<Result> {
        self.undo()
    }

    #[inline]
    fn redo(&mut self) -> Option<Result> {
        self.redo()
    }
}

impl<T> Timeline for Checkpoint<'_, History<T>> {
    type Target = T;

    #[inline]
    fn apply(&mut self, command: impl Command<T>) -> Result {
        self.apply(command)
    }

    #[inline]
    fn undo(&mut self) -> Option<Result> {
        self.undo()
    }

    #[inline]
    fn redo(&mut self) -> Option<Result> {
        self.redo()
    }
}

impl<'a, T: Timeline> From<&'a mut T> for Checkpoint<'a, T> {
    #[inline]
    fn from(inner: &'a mut T) -> Self {
        Checkpoint {
            inner,
            actions: Vec::new(),
        }
    }
}

/// An action that can be applied to a Record or History.
#[cfg_attr(feature = "display", derive(Debug))]
enum Action<T> {
    Apply(Option<usize>, VecDeque<Entry<T>>),
    Branch(usize, usize),
    Undo,
    Redo,
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
