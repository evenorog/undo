use crate::{Command, Entry, History, Queue, Record, Result};
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
/// assert_eq!(cp.as_receiver(), "abc");
/// cp.cancel()?;
/// assert_eq!(record.as_receiver(), "");
/// # Ok(())
/// # }
/// ```
#[cfg_attr(feature = "display", derive(Debug))]
pub struct Checkpoint<'a, T, R> {
    inner: &'a mut T,
    stack: Vec<Action<R>>,
}

impl<'a, T, R> From<&'a mut T> for Checkpoint<'a, T, R> {
    #[inline]
    fn from(inner: &'a mut T) -> Self {
        Checkpoint {
            inner,
            stack: Vec::new(),
        }
    }
}

impl<'a, T, R> Checkpoint<'a, T, R> {
    /// Returns a checkpoint.
    #[inline]
    pub fn new(inner: &'a mut T) -> Checkpoint<'a, T, R> {
        Checkpoint {
            inner,
            stack: Vec::new(),
        }
    }

    /// Reserves capacity for at least `additional` more commands in the checkpoint.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.stack.reserve(additional);
    }

    /// Returns the capacity of the checkpoint.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.stack.capacity()
    }

    /// Shrinks the capacity of the checkpoint as much as possible.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.stack.shrink_to_fit();
    }

    /// Returns the number of commands in the checkpoint.
    #[inline]
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// Returns `true` if the checkpoint is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// Commits the changes and consumes the checkpoint.
    #[inline]
    pub fn commit(self) {}
}

impl<R> Checkpoint<'_, Record<R>, R> {
    /// Calls the [`apply`] method.
    ///
    /// [`apply`]: struct.Record.html#method.apply
    #[inline]
    pub fn apply(&mut self, command: impl Command<R> + 'static) -> Result
    where
        R: 'static,
    {
        let (_, v) = self.inner.__apply(Entry::new(command))?;
        self.stack.push(Action::Apply(v));
        Ok(())
    }

    /// Calls the [`undo`] method.
    ///
    /// [`undo`]: struct.Record.html#method.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result> {
        match self.inner.undo() {
            Some(Ok(_)) => {
                self.stack.push(Action::Undo);
                Some(Ok(()))
            }
            undo => undo,
        }
    }

    /// Calls the [`redo`] method.
    ///
    /// [`redo`]: struct.Record.html#method.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result> {
        match self.inner.redo() {
            Some(Ok(_)) => {
                self.stack.push(Action::Redo);
                Some(Ok(()))
            }
            redo => redo,
        }
    }

    /// Calls the [`go_to`] method.
    ///
    /// [`go_to`]: struct.Record.html#method.go_to
    #[inline]
    pub fn go_to(&mut self, current: usize) -> Option<Result> {
        let old = self.inner.current();
        match self.inner.go_to(current) {
            Some(Ok(_)) => {
                self.stack.push(Action::GoTo(0, old));
                Some(Ok(()))
            }
            go_to => go_to,
        }
    }

    /// Calls the [`extend`] method.
    ///
    /// [`extend`]: struct.Record.html#method.extend
    #[inline]
    pub fn extend<C: Command<R> + 'static>(
        &mut self,
        commands: impl IntoIterator<Item = C>,
    ) -> Result
    where
        R: 'static,
    {
        for command in commands {
            self.apply(command)?;
        }
        Ok(())
    }

    /// Cancels the changes and consumes the checkpoint.
    ///
    /// # Errors
    /// If an error occur when canceling the changes, the error is returned
    /// and the remaining commands are not canceled.
    #[inline]
    pub fn cancel(self) -> Result {
        for action in self.stack.into_iter().rev() {
            match action {
                Action::Apply(mut v) => {
                    if let Some(Err(error)) = self.inner.undo() {
                        return Err(error);
                    }
                    let current = self.inner.current();
                    self.inner.commands.truncate(current);
                    self.inner.commands.append(&mut v);
                }
                Action::Undo => {
                    if let Some(Err(error)) = self.inner.redo() {
                        return Err(error);
                    }
                }
                Action::Redo => {
                    if let Some(Err(error)) = self.inner.undo() {
                        return Err(error);
                    }
                }
                Action::GoTo(_, current) => {
                    if let Some(Err(error)) = self.inner.go_to(current) {
                        return Err(error);
                    }
                }
            }
        }
        Ok(())
    }

    /// Returns a checkpoint.
    #[inline]
    pub fn checkpoint(&mut self) -> Checkpoint<Record<R>, R> {
        self.inner.checkpoint()
    }

    /// Returns a queue.
    #[inline]
    pub fn queue(&mut self) -> Queue<Record<R>, R> {
        self.inner.queue()
    }

    /// Returns a reference to the `receiver`.
    #[inline]
    pub fn as_receiver(&self) -> &R {
        self.inner.as_receiver()
    }

    /// Returns a mutable reference to the `receiver`.
    ///
    /// This method should **only** be used when doing changes that should not be able to be undone.
    #[inline]
    pub fn as_mut_receiver(&mut self) -> &mut R {
        self.inner.as_mut_receiver()
    }
}

impl<R> AsRef<R> for Checkpoint<'_, Record<R>, R> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.inner.as_ref()
    }
}

impl<R> AsMut<R> for Checkpoint<'_, Record<R>, R> {
    #[inline]
    fn as_mut(&mut self) -> &mut R {
        self.inner.as_mut()
    }
}

impl<R> Checkpoint<'_, History<R>, R> {
    /// Calls the [`apply`] method.
    ///
    /// [`apply`]: struct.History.html#method.apply
    #[inline]
    pub fn apply(&mut self, command: impl Command<R> + 'static) -> Result
    where
        R: 'static,
    {
        let root = self.inner.branch();
        let old = self.inner.current();
        self.inner.apply(command)?;
        self.stack.push(Action::GoTo(root, old));
        Ok(())
    }

    /// Calls the [`undo`] method.
    ///
    /// [`undo`]: struct.History.html#method.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result> {
        match self.inner.undo() {
            Some(Ok(_)) => {
                self.stack.push(Action::Undo);
                Some(Ok(()))
            }
            undo => undo,
        }
    }

    /// Calls the [`redo`] method.
    ///
    /// [`redo`]: struct.History.html#method.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result> {
        match self.inner.redo() {
            Some(Ok(_)) => {
                self.stack.push(Action::Redo);
                Some(Ok(()))
            }
            redo => redo,
        }
    }

    /// Calls the [`go_to`] method.
    ///
    /// [`go_to`]: struct.History.html#method.go_to
    #[inline]
    pub fn go_to(&mut self, branch: usize, current: usize) -> Option<Result>
    where
        R: 'static,
    {
        let root = self.inner.branch();
        let old = self.inner.current();
        match self.inner.go_to(branch, current) {
            Some(Ok(_)) => {
                self.stack.push(Action::GoTo(root, old));
                Some(Ok(()))
            }
            go_to => go_to,
        }
    }

    /// Calls the [`extend`] method.
    ///
    /// [`extend`]: struct.History.html#method.extend
    #[inline]
    pub fn extend<C: Command<R> + 'static>(
        &mut self,
        commands: impl IntoIterator<Item = C>,
    ) -> Result
    where
        R: 'static,
    {
        for command in commands {
            self.apply(command)?;
        }
        Ok(())
    }

    /// Cancels the changes and consumes the checkpoint.
    ///
    /// # Errors
    /// If an error occur when canceling the changes, the error is returned
    /// and the remaining commands are not canceled.
    #[inline]
    pub fn cancel(self) -> Result
    where
        R: 'static,
    {
        for action in self.stack.into_iter().rev() {
            match action {
                Action::Apply(_) => unreachable!(),
                Action::Undo => {
                    if let Some(Err(error)) = self.inner.redo() {
                        return Err(error);
                    }
                }
                Action::Redo => {
                    if let Some(Err(error)) = self.inner.undo() {
                        return Err(error);
                    }
                }
                Action::GoTo(branch, current) => {
                    if let Some(Err(error)) = self.inner.go_to(branch, current) {
                        return Err(error);
                    }
                }
            }
        }
        Ok(())
    }

    /// Returns a checkpoint.
    #[inline]
    pub fn checkpoint(&mut self) -> Checkpoint<History<R>, R> {
        self.inner.checkpoint()
    }

    /// Returns a queue.
    #[inline]
    pub fn queue(&mut self) -> Queue<History<R>, R> {
        self.inner.queue()
    }

    /// Returns a reference to the `receiver`.
    #[inline]
    pub fn as_receiver(&self) -> &R {
        self.inner.as_receiver()
    }

    /// Returns a mutable reference to the `receiver`.
    ///
    /// This method should **only** be used when doing changes that should not be able to be undone.
    #[inline]
    pub fn as_mut_receiver(&mut self) -> &mut R {
        self.inner.as_mut_receiver()
    }
}

impl<R> AsRef<R> for Checkpoint<'_, History<R>, R> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.inner.as_ref()
    }
}

impl<R> AsMut<R> for Checkpoint<'_, History<R>, R> {
    #[inline]
    fn as_mut(&mut self) -> &mut R {
        self.inner.as_mut()
    }
}

/// An action that can be applied to a Record or History.
#[cfg_attr(feature = "display", derive(Debug))]
enum Action<R> {
    Apply(VecDeque<Entry<R>>),
    Undo,
    Redo,
    GoTo(usize, usize),
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
        assert_eq!(cp1.as_receiver(), "abc");
        let mut cp2 = cp1.checkpoint();
        cp2.apply(Add('d')).unwrap();
        cp2.apply(Add('e')).unwrap();
        cp2.apply(Add('f')).unwrap();
        assert_eq!(cp2.as_receiver(), "abcdef");
        let mut cp3 = cp2.checkpoint();
        cp3.apply(Add('g')).unwrap();
        cp3.apply(Add('h')).unwrap();
        cp3.apply(Add('i')).unwrap();
        assert_eq!(cp3.as_receiver(), "abcdefghi");
        cp3.commit();
        cp2.commit();
        cp1.commit();
        assert_eq!(record.as_receiver(), "abcdefghi");
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
        assert_eq!(cp3.as_receiver(), "abcdefghi");
        cp3.cancel().unwrap();
        assert_eq!(cp2.as_receiver(), "abcdef");
        cp2.cancel().unwrap();
        assert_eq!(cp1.as_receiver(), "abc");
        cp1.cancel().unwrap();
        assert_eq!(record.as_receiver(), "");
    }
}
