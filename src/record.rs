use std::collections::vec_deque::{VecDeque, IntoIter};
use std::fmt::{self, Debug, Formatter};
use {Command, Error, Merger};

/// A record of commands.
///
/// The `Record` works mostly like a `Stack`, but it stores the commands
/// instead of returning them when undoing. This means it can roll the
/// receivers state backwards and forwards by using the undo and redo methods.
/// In addition, the `Record` has an internal state that is either clean or dirty.
/// A clean state means that the `Record` does not have any `Command`s to redo,
/// while a dirty state means that it does. The user can give the `Record` a function
/// that is called each time the state changes by using the `config` constructor.
///
/// # Examples
/// ```
/// use std::error::Error;
/// use undo::{Command, Record};
///
/// struct Add(char);
///
/// impl Command<String> for Add {
///     fn redo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
///         s.push(self.0);
///         Ok(())
///     }
///
///     fn undo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
///         self.0 = s.pop().ok_or("`String` is unexpectedly empty")?;
///         Ok(())
///     }
/// }
///
/// fn foo() -> Result<(), Box<Error>> {
///     let mut record = Record::default();
///
///     record.push(Add('a'))?;
///     record.push(Add('b'))?;
///     record.push(Add('c'))?;
///
///     assert_eq!(record.as_receiver(), "abc");
///
///     record.undo().unwrap()?;
///     record.undo().unwrap()?;
///     record.undo().unwrap()?;
///
///     assert_eq!(record.as_receiver(), "");
///
///     record.redo().unwrap()?;
///     record.redo().unwrap()?;
///     record.redo().unwrap()?;
///
///     assert_eq!(record.into_receiver(), "abc");
///
///     Ok(())
/// }
/// # foo().unwrap();
/// ```
#[derive(Default)]
pub struct Record<'a, R> {
    commands: VecDeque<Box<Command<R>>>,
    receiver: R,
    idx: usize,
    limit: Option<usize>,
    state_change: Option<Box<FnMut(bool) + 'a>>,
}

impl<'a, R> Record<'a, R> {
    /// Returns a new `Record`.
    #[inline]
    pub fn new<T: Into<R>>(receiver: T) -> Record<'a, R> {
        Record {
            commands: VecDeque::new(),
            receiver: receiver.into(),
            idx: 0,
            limit: None,
            state_change: None,
        }
    }

    /// Returns a configurator for a `Record`.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # use undo::{Command, Record};
    /// # struct Add(char);
    /// # impl Command<String> for Add {
    /// #     fn redo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
    /// #         s.push(self.0);
    /// #         Ok(())
    /// #     }
    /// #     fn undo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
    /// #         self.0 = s.pop().ok_or("`String` is unexpectedly empty")?;
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> Result<(), Box<Error>> {
    /// let mut record = Record::config("")
    ///     .capacity(2)
    ///     .limit(2)
    ///     .create();
    ///
    /// record.push(Add('a'))?;
    /// record.push(Add('b'))?;
    /// record.push(Add('c'))?; // 'a' is removed from the record since limit is 2.
    ///
    /// assert_eq!(record.as_receiver(), "abc");
    ///
    /// record.undo().unwrap()?;
    /// record.undo().unwrap()?;
    /// assert!(record.undo().is_none());
    ///
    /// assert_eq!(record.into_receiver(), "a");
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn config<T: Into<R>>(receiver: T) -> Config<'a, R> {
        Config {
            receiver: receiver.into(),
            capacity: 0,
            limit: None,
            state_change: None,
        }
    }

    /// Returns the limit of the `Record`, or `None` if it has no limit.
    #[inline]
    pub fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// Returns the number of commands the stack can hold without reallocating.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.commands.capacity()
    }

    /// Returns the number of commands in the `Record`.
    #[inline]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns `true` if the `Record` is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Returns `true` if the state of the stack is clean, `false` otherwise.
    #[inline]
    pub fn is_clean(&self) -> bool {
        self.idx == self.commands.len()
    }

    /// Returns `true` if the state of the stack is dirty, `false` otherwise.
    #[inline]
    pub fn is_dirty(&self) -> bool {
        !self.is_clean()
    }

    /// Returns a reference to the `receiver`.
    #[inline]
    pub fn as_receiver(&self) -> &R {
        &self.receiver
    }

    /// Consumes the `Record`, returning the `receiver`.
    #[inline]
    pub fn into_receiver(self) -> R {
        self.receiver
    }

    /// Pushes `cmd` to the top of the `Record` and executes its [`redo`] method.
    /// The command is merged with the previous top command if [`merge`] does not return `None`.
    ///
    /// All commands above the active one are removed from the stack and returned as an iterator.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the error is returned together with the command,
    /// and the state of the stack is left unchanged.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # use undo::{Command, Record};
    /// # struct Add(char);
    /// # impl Command<String> for Add {
    /// #     fn redo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
    /// #         s.push(self.0);
    /// #         Ok(())
    /// #     }
    /// #     fn undo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
    /// #         self.0 = s.pop().ok_or("`String` is unexpectedly empty")?;
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> Result<(), Box<Error>> {
    /// let mut record = Record::default();
    ///
    /// record.push(Add('a'))?;
    /// record.push(Add('b'))?;
    /// record.push(Add('c'))?;
    ///
    /// assert_eq!(record.as_receiver(), "abc");
    ///
    /// record.undo().unwrap()?;
    /// record.undo().unwrap()?;
    /// let mut bc = record.push(Add('e'))?;
    ///
    /// assert_eq!(record.into_receiver(), "ae");
    /// assert!(bc.next().is_some());
    /// assert!(bc.next().is_some());
    /// assert!(bc.next().is_none());
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    ///
    /// [`redo`]: trait.Command.html#tymethod.redo
    pub fn push<C>(&mut self, mut cmd: C) -> Result<Commands<R>, Error<R>>
    where
        C: Command<R> + 'static,
        R: 'static,
    {
        let is_dirty = self.is_dirty();
        let len = self.idx;
        if let Err(e) = cmd.redo(&mut self.receiver) {
            return Err(Error(Box::new(cmd), e));
        }
        // Pop off all elements after len from record.
        let iter = self.commands.split_off(len).into_iter();
        debug_assert_eq!(len, self.len());

        match (cmd.id(), self.commands.back().and_then(|last| last.id())) {
            (Some(id1), Some(id2)) if id1 == id2 => {
                // Merge the command with the one on the top of the stack.
                let cmd = Merger {
                    cmd1: self.commands.pop_back().unwrap(),
                    cmd2: Box::new(cmd),
                };
                self.commands.push_back(Box::new(cmd));
            }
            _ => {
                match self.limit {
                    Some(limit) if len == limit => {
                        self.commands.pop_front();
                    }
                    _ => self.idx += 1,
                }
                self.commands.push_back(Box::new(cmd));
            }
        }

        debug_assert_eq!(self.idx, self.len());
        // Record is always clean after a push, check if it was dirty before.
        if is_dirty {
            if let Some(ref mut f) = self.state_change {
                f(true);
            }
        }
        Ok(Commands(iter))
    }

    /// Calls the [`redo`] method for the active `Command` and sets the next one as the new
    /// active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the command that caused the error is removed from
    /// the record and returned together with the error.
    ///
    /// [`redo`]: trait.Command.html#tymethod.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result<(), Error<R>>> {
        if self.idx < self.commands.len() {
            let is_dirty = self.is_dirty();
            match self.commands[self.idx].redo(&mut self.receiver) {
                Ok(_) => {
                    self.idx += 1;
                    // Check if record went from dirty to clean.
                    if is_dirty && self.is_clean() {
                        if let Some(ref mut f) = self.state_change {
                            f(true);
                        }
                    }
                    Some(Ok(()))
                }
                Err(e) => {
                    let cmd = self.commands.remove(self.idx).unwrap();
                    Some(Err(Error(cmd, e)))
                }
            }
        } else {
            None
        }
    }

    /// Calls the [`undo`] method for the active `Command` and sets the previous one as the
    /// new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the command that caused the error is removed from
    /// the record and returned together with the error.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result<(), Error<R>>> {
        if self.idx > 0 {
            let is_clean = self.is_clean();
            self.idx -= 1;
            match self.commands[self.idx].undo(&mut self.receiver) {
                Ok(_) => {
                    // Check if record went from clean to dirty.
                    if is_clean && self.is_dirty() {
                        if let Some(ref mut f) = self.state_change {
                            f(false);
                        }
                    }
                    Some(Ok(()))
                }
                Err(e) => {
                    let cmd = self.commands.remove(self.idx).unwrap();
                    Some(Err(Error(cmd, e)))
                }
            }
        } else {
            None
        }
    }
}

impl<'a, R> AsRef<R> for Record<'a, R> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.as_receiver()
    }
}

impl<'a, R: Debug> Debug for Record<'a, R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Record")
            .field("commands", &self.commands)
            .field("receiver", &self.receiver)
            .field("idx", &self.idx)
            .field("limit", &self.limit)
            .finish()
    }
}

/// Iterator over `Command`s.
#[derive(Debug)]
pub struct Commands<R>(IntoIter<Box<Command<R>>>);

impl<R> Iterator for Commands<R> {
    type Item = Box<Command<R>>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// Configurator for `Record`.
pub struct Config<'a, R> {
    receiver: R,
    capacity: usize,
    limit: Option<usize>,
    state_change: Option<Box<FnMut(bool) + 'a>>,
}

impl<'a, R> Config<'a, R> {
    /// Sets the specified [capacity] for the stack.
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> Config<'a, R> {
        self.capacity = capacity;
        self
    }

    /// Sets a limit on how many `Command`s can be stored in the stack.
    /// If this limit is reached it will start popping of commands at the bottom of the stack when
    /// pushing new commands on to the stack. No limit is set by default which means it may grow
    /// indefinitely.
    #[inline]
    pub fn limit(mut self, limit: usize) -> Config<'a, R> {
        self.limit = if limit == 0 { None } else { Some(limit) };
        self
    }

    /// Sets what should happen when the state changes.
    /// By default the `Record` does nothing when the state changes.
    ///
    /// # Examples
    /// ```
    /// # use std::cell::Cell;
    /// # use std::error::Error;
    /// # use undo::{Command, Record};
    /// # struct Add(char);
    /// # impl Command<String> for Add {
    /// #     fn redo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
    /// #         s.push(self.0);
    /// #         Ok(())
    /// #     }
    /// #     fn undo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
    /// #         self.0 = s.pop().ok_or("`String` is unexpectedly empty")?;
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> Result<(), Box<Error>> {
    /// let x = Cell::new(0);
    /// let mut record = Record::config("")
    ///     .state_change(|is_clean| {
    ///         if is_clean {
    ///             x.set(1);
    ///         } else {
    ///             x.set(2);
    ///         }
    ///     })
    ///     .create();
    ///
    /// assert_eq!(x.get(), 0);
    /// record.push(Add('a'))?;
    /// assert_eq!(x.get(), 0);
    /// record.undo().unwrap()?;
    /// assert_eq!(x.get(), 2);
    /// record.redo().unwrap()?;
    /// assert_eq!(x.get(), 1);
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn state_change<F>(mut self, f: F) -> Config<'a, R>
    where
        F: FnMut(bool) + 'a,
    {
        self.state_change = Some(Box::new(f));
        self
    }

    /// Creates the `Record`.
    #[inline]
    pub fn create(self) -> Record<'a, R> {
        Record {
            commands: VecDeque::with_capacity(self.capacity),
            receiver: self.receiver,
            idx: 0,
            limit: self.limit,
            state_change: self.state_change,
        }
    }
}

impl<'a, R: Debug> Debug for Config<'a, R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Config")
            .field("receiver", &self.receiver)
            .field("capacity", &self.capacity)
            .field("limit", &self.limit)
            .finish()
    }
}
