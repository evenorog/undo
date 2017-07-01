use std::collections::vec_deque::{VecDeque, IntoIter};
use std::error::Error;
use std::fmt::{self, Debug, Formatter};
use Command;

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
///         self.0 = s.pop().expect("`String` is unexpectedly empty");
///         Ok(())
///     }
/// }
///
/// fn foo() -> Result<(), Box<Error>> {
///     let mut record = Record::default();
///
///     record.push(Add('a')).map_err(|(_, e)| e)?;
///     record.push(Add('b')).map_err(|(_, e)| e)?;
///     record.push(Add('c')).map_err(|(_, e)| e)?;
///
///     assert_eq!(record.as_receiver(), "abc");
///
///     record.undo()?;
///     record.undo()?;
///     record.undo()?;
///
///     assert_eq!(record.as_receiver(), "");
///
///     record.redo()?;
///     record.redo()?;
///     record.redo()?;
///
///     assert_eq!(record.into_receiver(), "abc");
///
///     Ok(())
/// }
/// # foo().unwrap();
/// ```
#[derive(Default)]
pub struct Record<'a, T: 'static> {
    commands: VecDeque<Box<Command<T>>>,
    receiver: T,
    idx: usize,
    limit: Option<usize>,
    state_change: Option<Box<FnMut(bool) + 'a>>,
}

impl<'a, T> Record<'a, T> {
    /// Returns a new `Record`.
    #[inline]
    pub fn new<U: Into<T>>(receiver: U) -> Record<'a, T> {
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
    /// #         self.0 = s.pop().expect("`String` is unexpectedly empty");
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> Result<(), Box<Error>> {
    /// let mut record = Record::config("")
    ///     .capacity(2)
    ///     .limit(2)
    ///     .finish();
    ///
    /// record.push(Add('a')).map_err(|(_, e)| e)?;
    /// record.push(Add('b')).map_err(|(_, e)| e)?;
    /// record.push(Add('c')).map_err(|(_, e)| e)?; // 'a' is removed from the record since limit is 2.
    ///
    /// assert_eq!(record.as_receiver(), "abc");
    ///
    /// record.undo()?;
    /// record.undo()?;
    /// record.undo()?;
    ///
    /// assert_eq!(record.as_receiver(), "a");
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn config<U: Into<T>>(receiver: U) -> Config<'a, T> {
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

    /// Returns the number of `Command`s in the `Record`.
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
    pub fn as_receiver(&self) -> &T {
        &self.receiver
    }

    /// Consumes the `Record`, returning the `receiver`.
    #[inline]
    pub fn into_receiver(self) -> T {
        self.receiver
    }

    /// Pushes `cmd` to the top of the stack and executes its [`redo`] method.
    /// This pops off all other commands above the active command from the stack.
    ///
    /// If `cmd`s id is equal to the top command on the stack, the two commands are merged.
    ///
    /// # Errors
    /// If an error occur when executing `redo` the error is returned
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
    /// #         self.0 = s.pop().expect("`String` is unexpectedly empty");
    /// #         Ok(())
    /// #     }
    /// # }
    /// # fn foo() -> Result<(), Box<Error>> {
    /// let mut record = Record::default();
    ///
    /// record.push(Add('a')).map_err(|(_, e)| e)?;
    /// record.push(Add('b')).map_err(|(_, e)| e)?;
    /// record.push(Add('c')).map_err(|(_, e)| e)?;
    ///
    /// assert_eq!(record.as_receiver(), "abc");
    ///
    /// record.undo()?;
    /// record.undo()?;
    /// let mut bc = record.push(Add('e')).map_err(|(_, e)| e)?;
    ///
    /// assert_eq!(record.as_receiver(), "ae");
    /// assert!(bc.next().is_some());
    /// assert!(bc.next().is_some());
    /// assert!(bc.next().is_none());
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    ///
    /// [`redo`]: trait.UndoCmd.html#tymethod.redo
    pub fn push<C>(&mut self, mut cmd: C) -> Result<Commands<T>, (Box<Command<T>>, Box<Error>)>
    where
        C: Command<T> + 'static,
    {
        let is_dirty = self.is_dirty();
        let len = self.idx;
        if let Err(e) = cmd.redo(&mut self.receiver) {
            return Err((Box::new(cmd), e));
        }
        // Pop off all elements after len from stack.
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
        // State is always clean after a push, check if it was dirty before.
        if is_dirty {
            if let Some(ref mut f) = self.state_change {
                f(true);
            }
        }
        Ok(Commands(iter))
    }

    /// Calls the [`redo`] method for the active `UndoCmd` and sets the next `UndoCmd` as the new
    /// active one.
    ///
    /// # Errors
    /// If an error occur when executing `redo` the error is returned
    /// and the state of the stack is left unchanged.
    ///
    /// [`redo`]: trait.UndoCmd.html#tymethod.redo
    #[inline]
    pub fn redo(&mut self) -> Result<(), Box<Error>> {
        if self.idx < self.commands.len() {
            let is_dirty = self.is_dirty();
            self.commands[self.idx].redo(&mut self.receiver)?;
            self.idx += 1;
            // Check if stack went from dirty to clean.
            if is_dirty && self.is_clean() {
                if let Some(ref mut f) = self.state_change {
                    f(true);
                }
            }
        }
        Ok(())
    }

    /// Calls the [`undo`] method for the active `UndoCmd` and sets the previous `UndoCmd` as the
    /// new active one.
    ///
    /// # Errors
    /// If an error occur when executing `undo` the error is returned
    /// and the state of the stack is left unchanged.
    ///
    /// [`undo`]: trait.UndoCmd.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Result<(), Box<Error>> {
        if self.idx > 0 {
            let is_clean = self.is_clean();
            self.idx -= 1;
            self.commands[self.idx].undo(&mut self.receiver)?;
            // Check if stack went from clean to dirty.
            if is_clean && self.is_dirty() {
                if let Some(ref mut f) = self.state_change {
                    f(false);
                }
            }
        }
        Ok(())
    }
}

impl<'a, T: Debug> Debug for Record<'a, T> {
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
pub struct Commands<T>(IntoIter<Box<Command<T>>>);

impl<T> Iterator for Commands<T> {
    type Item = Box<Command<T>>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

#[derive(Debug)]
struct Merger<T> {
    cmd1: Box<Command<T>>,
    cmd2: Box<Command<T>>,
}

impl<T> Command<T> for Merger<T> {
    #[inline]
    fn redo(&mut self, receiver: &mut T) -> Result<(), Box<Error>> {
        self.cmd1.redo(receiver)?;
        self.cmd2.redo(receiver)
    }

    #[inline]
    fn undo(&mut self, receiver: &mut T) -> Result<(), Box<Error>> {
        self.cmd2.undo(receiver)?;
        self.cmd1.undo(receiver)
    }

    #[inline]
    fn id(&self) -> Option<u64> {
        self.cmd1.id()
    }
}

/// Configurator for `Record`.
pub struct Config<'a, T> {
    receiver: T,
    capacity: usize,
    limit: Option<usize>,
    state_change: Option<Box<FnMut(bool) + 'a>>,
}

impl<'a, T> Config<'a, T> {
    /// Sets the specified [capacity] for the stack.
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> Config<'a, T> {
        self.capacity = capacity;
        self
    }

    /// Sets a limit on how many `UndoCmd`s can be stored in the stack.
    /// If this limit is reached it will start popping of commands at the bottom of the stack when
    /// pushing new commands on to the stack. No limit is set by default which means it may grow
    /// indefinitely.
    #[inline]
    pub fn limit(mut self, limit: usize) -> Config<'a, T> {
        self.limit = if limit == 0 { None } else { Some(limit) };
        self
    }

    /// Sets what should happen when the state changes.
    /// By default the `Record` does nothing when the state changes.
    #[inline]
    pub fn state_change<F>(mut self, f: F) -> Config<'a, T>
    where
        F: FnMut(bool) + 'a,
    {
        self.state_change = Some(Box::new(f));
        self
    }

    /// Returns the `Record`.
    #[inline]
    pub fn finish(self) -> Record<'a, T> {
        Record {
            commands: VecDeque::with_capacity(self.capacity),
            receiver: self.receiver,
            idx: 0,
            limit: self.limit,
            state_change: self.state_change,
        }
    }
}

impl<'a, T: Debug> Debug for Config<'a, T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Config")
            .field("receiver", &self.receiver)
            .field("capacity", &self.capacity)
            .field("limit", &self.limit)
            .finish()
    }
}
