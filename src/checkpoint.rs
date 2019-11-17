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
/// assert_eq!(cp.as_target(), "abc");
/// cp.cancel()?;
/// assert_eq!(record.as_target(), "");
/// # Ok(())
/// # }
/// ```
#[cfg_attr(feature = "display", derive(Debug))]
pub struct Checkpoint<'a, T: Timeline> {
    inner: &'a mut T,
    stack: Vec<Action<T::Target>>,
}

impl<'a, T: Timeline> Checkpoint<'a, T> {
    /// Returns a checkpoint.
    #[inline]
    pub fn new(inner: &'a mut T) -> Checkpoint<'a, T> {
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

    /// Commits the changes and consumes the checkpoint.
    #[inline]
    pub fn commit(self) {}
}

impl<T: 'static> Checkpoint<'_, Record<T>> {
    /// Calls the [`apply`] method.
    ///
    /// [`apply`]: struct.Record.html#method.apply
    #[inline]
    pub fn apply(&mut self, command: impl Command<T> + 'static) -> Result {
        let (_, v) = self.inner.__apply(Entry::new(command))?;
        self.stack.push(Action::Apply(v));
        Ok(())
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
    pub fn extend<C: Command<T> + 'static>(
        &mut self,
        commands: impl IntoIterator<Item = C>,
    ) -> Result {
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
    pub fn checkpoint(&mut self) -> Checkpoint<Record<T>> {
        self.inner.checkpoint()
    }

    /// Returns a queue.
    #[inline]
    pub fn queue(&mut self) -> Queue<Record<T>> {
        self.inner.queue()
    }

    /// Returns a reference to the `target`.
    #[inline]
    pub fn as_target(&self) -> &T {
        self.inner.as_target()
    }

    /// Returns a mutable reference to the `target`.
    ///
    /// This method should **only** be used when doing changes that should not be able to be undone.
    #[inline]
    pub fn as_mut_target(&mut self) -> &mut T {
        self.inner.as_mut_target()
    }
}

impl<T: 'static> Checkpoint<'_, History<T>> {
    /// Calls the [`apply`] method.
    ///
    /// [`apply`]: struct.History.html#method.apply
    #[inline]
    pub fn apply(&mut self, command: impl Command<T> + 'static) -> Result {
        let root = self.inner.branch();
        let old = self.inner.current();
        self.inner.apply(command)?;
        self.stack.push(Action::GoTo(root, old));
        Ok(())
    }

    /// Calls the [`go_to`] method.
    ///
    /// [`go_to`]: struct.History.html#method.go_to
    #[inline]
    pub fn go_to(&mut self, branch: usize, current: usize) -> Option<Result> {
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
    pub fn extend<C: Command<T> + 'static>(
        &mut self,
        commands: impl IntoIterator<Item = C>,
    ) -> Result {
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
    pub fn checkpoint(&mut self) -> Checkpoint<History<T>> {
        self.inner.checkpoint()
    }

    /// Returns a queue.
    #[inline]
    pub fn queue(&mut self) -> Queue<History<T>> {
        self.inner.queue()
    }

    /// Returns a reference to the `target`.
    #[inline]
    pub fn as_target(&self) -> &T {
        self.inner.as_target()
    }

    /// Returns a mutable reference to the `target`.
    ///
    /// This method should **only** be used when doing changes that should not be able to be undone.
    #[inline]
    pub fn as_mut_target(&mut self) -> &mut T {
        self.inner.as_mut_target()
    }
}

impl<T: 'static> Timeline for Checkpoint<'_, Record<T>> {
    type Target = T;

    #[inline]
    fn apply(&mut self, command: impl Command<T> + 'static) -> Result {
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

impl<T: 'static> Timeline for Checkpoint<'_, History<T>> {
    type Target = T;

    #[inline]
    fn apply(&mut self, command: impl Command<T> + 'static) -> Result {
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
        Checkpoint::new(inner)
    }
}

impl<T: Timeline + AsRef<U>, U> AsRef<U> for Checkpoint<'_, T> {
    #[inline]
    fn as_ref(&self) -> &U {
        self.inner.as_ref()
    }
}

impl<T: Timeline + AsMut<U>, U> AsMut<U> for Checkpoint<'_, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut U {
        self.inner.as_mut()
    }
}

/// An action that can be applied to a Record or History.
#[cfg_attr(feature = "display", derive(Debug))]
enum Action<T> {
    Apply(VecDeque<Entry<T>>),
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
        assert_eq!(cp1.as_target(), "abc");
        let mut cp2 = cp1.checkpoint();
        cp2.apply(Add('d')).unwrap();
        cp2.apply(Add('e')).unwrap();
        cp2.apply(Add('f')).unwrap();
        assert_eq!(cp2.as_target(), "abcdef");
        let mut cp3 = cp2.checkpoint();
        cp3.apply(Add('g')).unwrap();
        cp3.apply(Add('h')).unwrap();
        cp3.apply(Add('i')).unwrap();
        assert_eq!(cp3.as_target(), "abcdefghi");
        cp3.commit();
        cp2.commit();
        cp1.commit();
        assert_eq!(record.as_target(), "abcdefghi");
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
        assert_eq!(cp3.as_target(), "abcdefghi");
        cp3.cancel().unwrap();
        assert_eq!(cp2.as_target(), "abcdef");
        cp2.cancel().unwrap();
        assert_eq!(cp1.as_target(), "abc");
        cp1.cancel().unwrap();
        assert_eq!(record.as_target(), "");
    }
}
