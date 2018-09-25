use {At, Command, Error, History, Meta, Record};

/// A checkpoint wrapper.
///
/// Wraps a Record or History and gives it checkpoint functionality.
#[derive(Debug)]
pub struct Checkpoint<'a, T: 'a> {
    inner: &'a mut T,
    at: Option<At>,
}

impl<'a, R> From<&'a mut Record<R>> for Checkpoint<'a, Record<R>> {
    #[inline]
    fn from(record: &'a mut Record<R>) -> Self {
        let cursor = record.cursor();
        Checkpoint {
            inner: record,
            at: Some(At { branch: 0, cursor }),
        }
    }
}

impl<'a, R> Checkpoint<'a, Record<R>> {
    #[inline]
    pub fn apply(
        &mut self,
        command: impl Command<R> + 'static,
    ) -> Result<impl Iterator<Item = impl Command<R> + 'static>, Error<R>>
    where
        R: 'static,
    {
        let cursor = self.inner.cursor();
        self.at = self.at.filter(|at| at.cursor <= cursor);
        let (merged, v) = self.inner.__apply(Meta::new(command))?;
        if !merged && cursor == self.inner.cursor() {
            self.at = self.at.and_then(|at| {
                at.cursor
                    .checked_sub(1)
                    .map(|cursor| At { branch: 0, cursor })
            });
        }
        Ok(v.into_iter())
    }

    #[inline]
    #[must_use]
    pub fn undo(&mut self) -> Option<Result<(), Error<R>>> {
        self.inner.undo()
    }

    #[inline]
    #[must_use]
    pub fn redo(&mut self) -> Option<Result<(), Error<R>>> {
        self.inner.redo()
    }

    #[inline]
    #[must_use]
    pub fn go_to(&mut self, cursor: usize) -> Option<Result<(), Error<R>>> {
        self.inner.go_to(cursor)
    }

    #[inline]
    pub fn commit(self) {}

    #[inline]
    pub fn cancel(self) -> Option<Result<(), Error<R>>> {
        let at = self.at?;
        self.inner.go_to(at.cursor)
    }

    #[inline]
    pub fn checkpoint(&mut self) -> Checkpoint<Record<R>> {
        self.inner.checkpoint()
    }

    #[inline]
    pub fn as_receiver(&self) -> &R {
        self.inner.as_receiver()
    }
}

impl<'a, R> AsRef<R> for Checkpoint<'a, Record<R>> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.inner.as_ref()
    }
}

impl<'a, R> From<&'a mut History<R>> for Checkpoint<'a, History<R>> {
    #[inline]
    fn from(history: &'a mut History<R>) -> Self {
        let branch = history.root();
        let cursor = history.cursor();
        Checkpoint {
            inner: history,
            at: Some(At { branch, cursor }),
        }
    }
}

impl<'a, R> Checkpoint<'a, History<R>> {
    #[inline]
    pub fn apply(&mut self, command: impl Command<R> + 'static) -> Result<(), Error<R>>
    where
        R: 'static,
    {
        let root = self.inner.root();
        let cursor = self.inner.cursor();
        let merged = self.inner.__apply(Meta::new(command))?;
        if !merged && cursor == self.inner.cursor() {
            self.at = self.at.and_then(|at| {
                let branch = at.branch;
                at.cursor.checked_sub(1).map(|cursor| At { branch, cursor })
            });
        }
        self.at = match self.at {
            Some(at) if at.branch == root && at.cursor > cursor => Some(at),
            Some(at) if at.branch == root && at.cursor <= cursor => Some(At {
                branch: self.inner.root(),
                cursor: at.cursor,
            }),
            at => at,
        };
        Ok(())
    }

    #[inline]
    #[must_use]
    pub fn undo(&mut self) -> Option<Result<(), Error<R>>> {
        self.inner.undo()
    }

    #[inline]
    #[must_use]
    pub fn redo(&mut self) -> Option<Result<(), Error<R>>> {
        self.inner.redo()
    }

    #[inline]
    #[must_use]
    pub fn go_to(&mut self, branch: usize, cursor: usize) -> Option<Result<(), Error<R>>>
    where
        R: 'static,
    {
        let root = self.inner.root();
        let old = self.inner.cursor();
        let ok = self.inner.go_to(branch, cursor);
        // TODO: Test case when branch is not equal to root, but is part of the path to branch.
        self.at = match self.at {
            Some(at) if at.branch == root && at.cursor > old => Some(at),
            Some(at) if at.branch == root && at.cursor <= old => Some(At {
                branch: self.inner.root(),
                cursor: at.cursor,
            }),
            at => at,
        };
        ok
    }

    #[inline]
    pub fn commit(self) {}

    #[inline]
    pub fn cancel(self) -> Option<Result<(), Error<R>>>
    where
        R: 'static,
    {
        let at = self.at?;
        self.inner.go_to(at.branch, at.cursor)
    }

    #[inline]
    pub fn checkpoint(&mut self) -> Checkpoint<History<R>> {
        self.inner.checkpoint()
    }

    #[inline]
    pub fn as_receiver(&self) -> &R {
        self.inner.as_receiver()
    }
}

impl<'a, R> AsRef<R> for Checkpoint<'a, History<R>> {
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
            let mut cp = record.checkpoint();
            cp.apply(Add('a')).unwrap();
            cp.apply(Add('b')).unwrap();
            cp.apply(Add('c')).unwrap();
            assert_eq!(cp.as_receiver(), "abc");
            {
                let mut cp = cp.checkpoint();
                cp.apply(Add('d')).unwrap();
                cp.apply(Add('e')).unwrap();
                cp.apply(Add('f')).unwrap();
                assert_eq!(cp.as_receiver(), "abcdef");
                {
                    let mut cp = cp.checkpoint();
                    cp.apply(Add('g')).unwrap();
                    cp.apply(Add('h')).unwrap();
                    cp.apply(Add('i')).unwrap();
                    assert_eq!(cp.as_receiver(), "abcdefghi");
                    cp.commit();
                }
                cp.commit();
            }
            cp.commit();
        }
        assert_eq!(record.as_receiver(), "abcdefghi");
    }

    #[test]
    fn cancel() {
        let mut record = Record::default();
        {
            let mut cp = record.checkpoint();
            cp.apply(Add('a')).unwrap();
            cp.apply(Add('b')).unwrap();
            cp.apply(Add('c')).unwrap();
            {
                let mut cp = cp.checkpoint();
                cp.apply(Add('d')).unwrap();
                cp.apply(Add('e')).unwrap();
                cp.apply(Add('f')).unwrap();
                {
                    let mut cp = cp.checkpoint();
                    cp.apply(Add('g')).unwrap();
                    cp.apply(Add('h')).unwrap();
                    cp.apply(Add('i')).unwrap();
                    assert_eq!(cp.as_receiver(), "abcdefghi");
                    cp.cancel().unwrap().unwrap();
                }
                assert_eq!(cp.as_receiver(), "abcdef");
                cp.cancel().unwrap().unwrap();
            }
            assert_eq!(cp.as_receiver(), "abc");
            cp.cancel().unwrap().unwrap();
        }
        assert_eq!(record.as_receiver(), "");
    }
}
