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
/// assert_eq!(queue.target(), "");
/// queue.commit()?;
/// assert_eq!(record.target(), "abc");
/// # Ok(())
/// # }
/// ```
#[cfg_attr(feature = "display", derive(Debug))]
pub struct Queue<'a, T: Timeline + ?Sized> {
    inner: &'a mut T,
    actions: Vec<Action<T::Target>>,
}

impl<'a, T: Timeline> Queue<'a, T> {
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
    pub fn apply(&mut self, command: impl Command<T::Target>) {
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
            }
        }
        Ok(())
    }

    /// Cancels the queued actions.
    pub fn cancel(self) {}
}

impl<T> Queue<'_, Record<T>> {
    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<Record<T>> {
        self.inner.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<Record<T>> {
        self.inner.checkpoint()
    }

    /// Returns a reference to the `target`.
    pub fn target(&self) -> &T {
        self.inner.target()
    }
}

impl<T> Queue<'_, History<T>> {
    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<History<T>> {
        self.inner.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<History<T>> {
        self.inner.checkpoint()
    }

    /// Returns a reference to the `target`.
    pub fn target(&self) -> &T {
        self.inner.target()
    }
}

impl<T: Timeline> Timeline for Queue<'_, T> {
    type Target = T::Target;

    fn apply(&mut self, command: impl Command<T::Target>) -> Result {
        self.apply(command);
        Ok(())
    }

    fn undo(&mut self) -> Option<Result> {
        self.undo();
        Some(Ok(()))
    }

    fn redo(&mut self) -> Option<Result> {
        self.redo();
        Some(Ok(()))
    }
}

impl<'a, T: Timeline> From<&'a mut T> for Queue<'a, T> {
    fn from(inner: &'a mut T) -> Self {
        Queue {
            inner,
            actions: Vec::new(),
        }
    }
}

/// An action that can be applied to a Record or History.
#[cfg_attr(feature = "display", derive(Debug))]
enum Action<T> {
    Apply(Box<dyn Command<T>>),
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
