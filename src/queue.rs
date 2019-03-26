use crate::{Checkpoint, Command, History, Record, Result};

/// A command queue wrapper.
///
/// Wraps a record or history and gives it batch queue functionality.
/// This allows the record or history to queue up commands and either cancel or apply them later.
///
/// # Examples
/// ```
/// # use undo::{Command, Record};
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
/// assert_eq!(queue.as_receiver(), "");
/// queue.commit()?;
/// assert_eq!(record.as_receiver(), "abc");
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Queue<'a, T, R> {
    inner: &'a mut T,
    queue: Vec<Action<R>>,
}

impl<'a, T, R> From<&'a mut T> for Queue<'a, T, R> {
    #[inline]
    fn from(inner: &'a mut T) -> Self {
        Queue {
            inner,
            queue: Vec::new(),
        }
    }
}

impl<'a, T, R> Queue<'a, T, R> {
    /// Returns a queue.
    #[inline]
    pub fn new(inner: &'a mut T) -> Queue<'a, T, R> {
        Queue {
            inner,
            queue: Vec::new(),
        }
    }

    /// Reserves capacity for at least `additional` more commands in the queue.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.queue.reserve(additional);
    }

    /// Returns the capacity of the queue.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.queue.capacity()
    }

    /// Returns the number of commands in the queue.
    #[inline]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Returns `true` if the queue is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Queues an `apply` action.
    #[inline]
    pub fn apply(&mut self, command: impl Command<R> + 'static) {
        self.queue.push(Action::Apply(Box::new(command)));
    }

    /// Queues an `undo` action.
    #[inline]
    pub fn undo(&mut self) {
        self.queue.push(Action::Undo);
    }

    /// Queues a `redo` action.
    #[inline]
    pub fn redo(&mut self) {
        self.queue.push(Action::Redo);
    }

    /// Cancels the queued actions.
    #[inline]
    pub fn cancel(self) {}
}

impl<T, R: 'static, C: Command<R> + 'static> Extend<C> for Queue<'_, T, R> {
    #[inline]
    fn extend<I: IntoIterator<Item = C>>(&mut self, commands: I) {
        for command in commands {
            self.apply(command);
        }
    }
}

impl<R> Queue<'_, Record<R>, R> {
    /// Queues a `go_to` action.
    #[inline]
    pub fn go_to(&mut self, current: usize) {
        self.queue.push(Action::GoTo(0, current));
    }

    /// Applies the actions that is queued.
    ///
    /// # Errors
    /// If an error occurs, it stops applying the actions and returns the error.
    #[inline]
    pub fn commit(self) -> Result
    where
        R: 'static,
    {
        for action in self.queue {
            match action {
                Action::Apply(command) => self.inner.apply(command)?,
                Action::Undo => {
                    if let Some(Err(error)) = self.inner.undo() {
                        return Err(error);
                    }
                }
                Action::Redo => {
                    if let Some(Err(error)) = self.inner.redo() {
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

impl<R> AsRef<R> for Queue<'_, Record<R>, R> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.inner.as_ref()
    }
}

impl<R> AsMut<R> for Queue<'_, Record<R>, R> {
    #[inline]
    fn as_mut(&mut self) -> &mut R {
        self.inner.as_mut()
    }
}

impl<R> Queue<'_, History<R>, R> {
    /// Queues a `go_to` action.
    #[inline]
    pub fn go_to(&mut self, branch: usize, current: usize) {
        self.queue.push(Action::GoTo(branch, current));
    }

    /// Applies the actions that is queued.
    ///
    /// # Errors
    /// If an error occurs, it stops applying the actions and returns the error.
    #[inline]
    pub fn commit(self) -> Result
    where
        R: 'static,
    {
        for action in self.queue {
            match action {
                Action::Apply(command) => self.inner.apply(command)?,
                Action::Undo => {
                    if let Some(Err(error)) = self.inner.undo() {
                        return Err(error);
                    }
                }
                Action::Redo => {
                    if let Some(Err(error)) = self.inner.redo() {
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

impl<R> AsRef<R> for Queue<'_, History<R>, R> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.inner.as_ref()
    }
}

impl<R> AsMut<R> for Queue<'_, History<R>, R> {
    #[inline]
    fn as_mut(&mut self) -> &mut R {
        self.inner.as_mut()
    }
}

/// An action that can be applied to a Record or History.
#[derive(Debug)]
enum Action<R> {
    Apply(Box<dyn Command<R>>),
    Undo,
    Redo,
    GoTo(usize, usize),
}

#[cfg(all(test, not(feature = "display")))]
mod tests {
    use crate::{Command, Record};

    #[derive(Debug)]
    struct Add(char);

    impl Command<String> for Add {
        fn apply(&mut self, s: &mut String) -> crate::Result {
            s.push(self.0);
            Ok(())
        }

        fn undo(&mut self, s: &mut String) -> crate::Result {
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
        assert_eq!(q3.as_receiver(), "");
        q3.commit().unwrap();
        assert_eq!(q2.as_receiver(), "abc");
        q2.commit().unwrap();
        assert_eq!(q1.as_receiver(), "");
        q1.commit().unwrap();
        assert_eq!(record.as_receiver(), "abc");
    }
}
