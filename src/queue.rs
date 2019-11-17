use crate::{Checkpoint, Command, History, Record, Result, Timeline};

/// A command queue wrapper.
///
/// Wraps a record or history and gives it batch queue functionality.
/// This allows the record or history to queue up commands and either cancel or apply them later.
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
/// let mut queue = record.queue();
/// queue.apply(Add('a'));
/// queue.apply(Add('b'));
/// queue.apply(Add('c'));
/// assert_eq!(queue.as_target(), "");
/// queue.commit()?;
/// assert_eq!(record.as_target(), "abc");
/// # Ok(())
/// # }
/// ```
#[cfg_attr(feature = "display", derive(Debug))]
pub struct Queue<'a, T: Timeline> {
    inner: &'a mut T,
    queue: Vec<Action<T::Target>>,
}

impl<'a, T: Timeline> Queue<'a, T> {
    /// Returns a queue.
    #[inline]
    pub fn new(inner: &'a mut T) -> Queue<'a, T> {
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

    /// Shrinks the capacity of the queue as much as possible.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.queue.shrink_to_fit();
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
    pub fn apply(&mut self, command: impl Command<T::Target>) {
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

    /// Queues an `apply` action for each command in the iterator.
    #[inline]
    pub fn extend(&mut self, commands: impl IntoIterator<Item = impl Command<T::Target>>) {
        for command in commands {
            self.apply(command);
        }
    }

    /// Cancels the queued actions.
    #[inline]
    pub fn cancel(self) {}
}

impl<T> Queue<'_, Record<T>> {
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
    pub fn commit(self) -> Result {
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

impl<T> Queue<'_, History<T>> {
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
    pub fn commit(self) -> Result {
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

impl<T: Timeline> Timeline for Queue<'_, T> {
    type Target = T::Target;

    #[inline]
    fn apply(&mut self, command: impl Command<T::Target>) -> Result {
        self.apply(command);
        Ok(())
    }

    #[inline]
    fn undo(&mut self) -> Option<Result> {
        self.undo();
        Some(Ok(()))
    }

    #[inline]
    fn redo(&mut self) -> Option<Result> {
        self.redo();
        Some(Ok(()))
    }
}

impl<'a, T: Timeline> From<&'a mut T> for Queue<'a, T> {
    #[inline]
    fn from(inner: &'a mut T) -> Self {
        Queue::new(inner)
    }
}

impl<T: Timeline + AsRef<U>, U> AsRef<U> for Queue<'_, T> {
    #[inline]
    fn as_ref(&self) -> &U {
        self.inner.as_ref()
    }
}

impl<T: Timeline + AsMut<U>, U> AsMut<U> for Queue<'_, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut U {
        self.inner.as_mut()
    }
}

/// An action that can be applied to a Record or History.
#[cfg_attr(feature = "display", derive(Debug))]
enum Action<T> {
    Apply(Box<dyn Command<T>>),
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
        assert_eq!(q3.as_target(), "");
        q3.commit().unwrap();
        assert_eq!(q2.as_target(), "abc");
        q2.commit().unwrap();
        assert_eq!(q1.as_target(), "");
        q1.commit().unwrap();
        assert_eq!(record.as_target(), "abc");
    }
}
