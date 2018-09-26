use std::mem;
use {Checkpoint, Command, Error, History, Meta, Record};

/// An action that can be applied to a Record or History.
#[derive(Debug)]
enum Action<R> {
    Apply(Box<dyn Command<R> + 'static>),
    Undo,
    Redo,
    GoTo(usize, usize),
    None,
}

/// A command queue.
#[derive(Debug)]
pub struct Queue<'a, T: 'a, R> {
    inner: &'a mut T,
    queue: Vec<Action<R>>,
}

impl<'a, T: 'a, R> From<&'a mut T> for Queue<'a, T, R> {
    #[inline]
    fn from(inner: &'a mut T) -> Self {
        Queue {
            inner,
            queue: Vec::new(),
        }
    }
}

impl<'a, T: 'a, R> Queue<'a, T, R> {
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

impl<'a, R> Queue<'a, Record<R>, R> {
    /// Queues a `go_to` action.
    #[inline]
    pub fn go_to(&mut self, cursor: usize) {
        self.queue.push(Action::GoTo(0, cursor));
    }

    /// Applies the actions that is queued.
    ///
    /// If an error occurs, it stops applying the actions and returns the error.
    #[inline]
    pub fn commit(self) -> Result<(), Error<R>>
    where
        R: 'static,
    {
        for action in self.queue {
            match action {
                Action::Apply(command) => {
                    let _ = self.inner.__apply(Meta::from(command))?;
                }
                Action::Undo => if let Some(Err(error)) = self.inner.undo() {
                    return Err(error);
                },
                Action::Redo => if let Some(Err(error)) = self.inner.redo() {
                    return Err(error);
                },
                Action::GoTo(_, cursor) => if let Some(Err(error)) = self.inner.go_to(cursor) {
                    return Err(error);
                },
                Action::None => unreachable!(),
            }
        }
        Ok(())
    }

    /// Applies the actions that is queued.
    ///
    /// # Errors
    /// If an error occurs, it tries to rewind the receiver back to the original state and returns the error.
    /// If the rewind fails, the error will hold the rewind error in its `fault` method.
    #[inline]
    pub fn commit_with_rewind(mut self) -> Result<(), Error<R>>
    where
        R: 'static,
    {
        let mut error_and_index = None;
        for (index, action) in self.queue.iter_mut().enumerate() {
            *action = match mem::replace(action, Action::None) {
                Action::Apply(command) => match self.inner.__apply(Meta::from(command)) {
                    Ok(_) => Action::Undo,
                    Err(error) => {
                        error_and_index = Some((error, index));
                        break;
                    }
                },
                Action::Undo => match self.inner.undo() {
                    Some(Ok(_)) => Action::Redo,
                    Some(Err(error)) => {
                        error_and_index = Some((error, index));
                        break;
                    }
                    None => Action::None,
                },
                Action::Redo => match self.inner.redo() {
                    Some(Ok(_)) => Action::Undo,
                    Some(Err(error)) => {
                        error_and_index = Some((error, index));
                        break;
                    }
                    None => Action::None,
                },
                Action::GoTo(_, cursor) => {
                    let old = self.inner.cursor();
                    match self.inner.go_to(cursor) {
                        Some(Ok(_)) => Action::GoTo(0, old),
                        Some(Err(error)) => {
                            error_and_index = Some((error, index));
                            break;
                        }
                        None => Action::None,
                    }
                }
                Action::None => unreachable!(),
            };
        }
        match error_and_index {
            Some((mut error, index)) => {
                let len = self.queue.len();
                for action in self.queue.into_iter().rev().skip(len - index) {
                    match action {
                        Action::Apply(_) => unreachable!(),
                        Action::Undo => if let Some(Err(fault)) = self.inner.undo() {
                            error.fault = Some(Box::new(fault));
                            break;
                        },
                        Action::Redo => if let Some(Err(fault)) = self.inner.redo() {
                            error.fault = Some(Box::new(fault));
                            break;
                        },
                        Action::GoTo(_, cursor) => {
                            if let Some(Err(fault)) = self.inner.go_to(cursor) {
                                error.fault = Some(Box::new(fault));
                                break;
                            }
                        }
                        Action::None => (),
                    }
                }
                Err(error)
            }
            None => Ok(()),
        }
    }

    /// Returns a checkpoint.
    #[inline]
    pub fn checkpoint(&mut self) -> Checkpoint<Record<R>> {
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
}

impl<'a, R> AsRef<R> for Queue<'a, Record<R>, R> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.inner.as_ref()
    }
}

impl<'a, R> Queue<'a, History<R>, R> {
    /// Queues a `go_to` action.
    #[inline]
    pub fn go_to(&mut self, branch: usize, cursor: usize) {
        self.queue.push(Action::GoTo(branch, cursor));
    }

    /// Applies the actions that is queued.
    ///
    /// # Errors
    /// If an error occurs, it stops applying the actions and returns the error.
    #[inline]
    pub fn commit(self) -> Result<(), Error<R>>
    where
        R: 'static,
    {
        for action in self.queue {
            match action {
                Action::Apply(command) => {
                    let _ = self.inner.__apply(Meta::from(command))?;
                }
                Action::Undo => if let Some(Err(error)) = self.inner.undo() {
                    return Err(error);
                },
                Action::Redo => if let Some(Err(error)) = self.inner.redo() {
                    return Err(error);
                },
                Action::GoTo(branch, cursor) => {
                    if let Some(Err(error)) = self.inner.go_to(branch, cursor) {
                        return Err(error);
                    }
                }
                Action::None => unreachable!(),
            }
        }
        Ok(())
    }

    /// Applies the actions that is queued.
    ///
    /// # Errors
    /// If an error occurs, it tries to rewind the receiver back to the original state and returns the error.
    /// If the rewind fails, the error will hold the rewind error in its `fault` method.
    #[inline]
    pub fn commit_with_rewind(mut self) -> Result<(), Error<R>>
    where
        R: 'static,
    {
        let mut error_and_index = None;
        for (index, action) in self.queue.iter_mut().enumerate() {
            *action = match mem::replace(action, Action::None) {
                Action::Apply(command) => match self.inner.__apply(Meta::from(command)) {
                    Ok(_) => Action::Undo,
                    Err(error) => {
                        error_and_index = Some((error, index));
                        break;
                    }
                },
                Action::Undo => match self.inner.undo() {
                    Some(Ok(_)) => Action::Redo,
                    Some(Err(error)) => {
                        error_and_index = Some((error, index));
                        break;
                    }
                    None => Action::None,
                },
                Action::Redo => match self.inner.redo() {
                    Some(Ok(_)) => Action::Undo,
                    Some(Err(error)) => {
                        error_and_index = Some((error, index));
                        break;
                    }
                    None => Action::None,
                },
                Action::GoTo(branch, cursor) => {
                    let root = self.inner.root();
                    let old = self.inner.cursor();
                    match self.inner.go_to(branch, cursor) {
                        Some(Ok(_)) => Action::GoTo(root, old),
                        Some(Err(error)) => {
                            error_and_index = Some((error, index));
                            break;
                        }
                        None => Action::None,
                    }
                }
                Action::None => unreachable!(),
            };
        }
        match error_and_index {
            Some((mut error, index)) => {
                let len = self.queue.len();
                for action in self.queue.into_iter().rev().skip(len - index) {
                    match action {
                        Action::Apply(_) => unreachable!(),
                        Action::Undo => if let Some(Err(fault)) = self.inner.undo() {
                            error.fault = Some(Box::new(fault));
                            break;
                        },
                        Action::Redo => if let Some(Err(fault)) = self.inner.redo() {
                            error.fault = Some(Box::new(fault));
                            break;
                        },
                        Action::GoTo(branch, cursor) => {
                            if let Some(Err(fault)) = self.inner.go_to(branch, cursor) {
                                error.fault = Some(Box::new(fault));
                                break;
                            }
                        }
                        Action::None => (),
                    }
                }
                Err(error)
            }
            None => Ok(()),
        }
    }

    /// Returns a checkpoint.
    #[inline]
    pub fn checkpoint(&mut self) -> Checkpoint<History<R>> {
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
}

impl<'a, R> AsRef<R> for Queue<'a, History<R>, R> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.inner.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use {Command, Record};

    #[derive(Debug)]
    struct Add(char);

    impl Command<String> for Add {
        fn apply(&mut self, s: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
            s.push(self.0);
            Ok(())
        }

        fn undo(&mut self, s: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
            self.0 = s.pop().ok_or("`s` is empty")?;
            Ok(())
        }
    }

    #[test]
    fn commit() {
        let mut record = Record::default();
        {
            let mut queue = record.queue();
            queue.redo();
            queue.redo();
            queue.redo();
            {
                let mut queue = queue.queue();
                queue.undo();
                queue.undo();
                queue.undo();
                {
                    let mut queue = queue.queue();
                    queue.apply(Add('a'));
                    queue.apply(Add('b'));
                    queue.apply(Add('c'));
                    assert_eq!(queue.as_receiver(), "");
                    queue.commit().unwrap();
                }
                assert_eq!(queue.as_receiver(), "abc");
                queue.commit().unwrap();
            }
            assert_eq!(queue.as_receiver(), "");
            queue.commit().unwrap();
        }
        assert_eq!(record.as_receiver(), "abc");
    }
}
