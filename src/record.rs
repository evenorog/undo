use std::collections::vec_deque::{IntoIter, VecDeque};
use std::error;
use std::fmt::{self, Debug, Display, Formatter};
use std::marker::PhantomData;
use {Command, Error, Merger};

/// Used to represent the state the record or the receiver can be in.
#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum Signal {
    /// Says if the record can redo.
    Redo(bool),
    /// Says if the record can undo.
    Undo(bool),
    /// Says if the receiver is in a saved state.
    Saved(bool),
    /// Says if the active command has changed.
    ///
    /// The cursors starts at `1`, e.g. they are always `index + 1`.
    Active { old: usize, new: usize },
}

/// A record of commands.
///
/// The record works mostly like a stack, but it stores the commands
/// instead of returning them when undoing. This means it can roll the
/// receivers state backwards and forwards by using the undo and redo methods.
/// In addition, the record can notify the user about changes to the stack or
/// the receiver through [signals]. The user can give the record a function
/// that is called each time the state changes by using the [`builder`].
///
/// # Examples
/// ```
/// use std::error::Error;
/// use std::fmt::{self, Display, Formatter};
/// use undo::{Command, Record};
///
/// #[derive(Debug)]
/// struct Add(char);
///
/// impl Command<String> for Add {
///     fn redo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
///         s.push(self.0);
///         Ok(())
///     }
///
///     fn undo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
///         self.0 = s.pop().ok_or("`s` is empty")?;
///         Ok(())
///     }
/// }
///
/// impl Display for Add {
///     fn fmt(&self, f: &mut Formatter) -> fmt::Result {
///         write!(f, "Add `{}`", self.0)
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
///
/// [`builder`]: struct.RecordBuilder.html
/// [signals]: enum.Signal.html
#[derive(Default)]
pub struct Record<'a, R> {
    commands: VecDeque<Box<Command<R>>>,
    receiver: R,
    cursor: usize,
    limit: usize,
    saved: Option<usize>,
    signals: Option<Box<FnMut(Signal) + Send + Sync + 'a>>,
}

impl<'a, R> Record<'a, R> {
    /// Returns a new record.
    #[inline]
    pub fn new<T: Into<R>>(receiver: T) -> Record<'a, R> {
        Record {
            commands: VecDeque::new(),
            receiver: receiver.into(),
            cursor: 0,
            limit: 0,
            saved: None,
            signals: None,
        }
    }

    /// Returns a builder for a record.
    #[inline]
    pub fn builder() -> RecordBuilder<'a, R> {
        RecordBuilder {
            receiver: PhantomData,
            capacity: 0,
            limit: 0,
            signals: None,
        }
    }

    /// Returns the capacity of the record.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.commands.capacity()
    }

    /// Returns the limit of the record, or `None` if it has no limit.
    #[inline]
    pub fn limit(&self) -> Option<usize> {
        match self.limit {
            0 => None,
            v => Some(v),
        }
    }

    /// Returns the number of commands in the record.
    #[inline]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns `true` if the record is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Returns a reference to the `receiver`.
    #[inline]
    pub fn as_receiver(&self) -> &R {
        &self.receiver
    }

    /// Consumes the record, returning the `receiver`.
    #[inline]
    pub fn into_receiver(self) -> R {
        self.receiver
    }

    /// Marks the receiver as currently being in a saved state.
    #[inline]
    pub fn set_saved(&mut self) {
        self.saved = match self.saved {
            Some(saved) if saved != self.cursor => {
                if let Some(ref mut f) = self.signals {
                    f(Signal::Saved(true));
                }
                Some(self.cursor)
            },
            Some(saved) => Some(saved),
            None => None,
        };
    }

    /// Marks the receiver as no longer being in a saved state.
    #[inline]
    pub fn set_unsaved(&mut self) {
        let was_saved = self.is_saved();
        self.saved = None;
        if was_saved {
            if let Some(ref mut f) = self.signals {
                f(Signal::Saved(false));
            }
        }
    }

    /// Returns `true` if the receiver is in a saved state, `false` otherwise.
    #[inline]
    pub fn is_saved(&self) -> bool {
        self.saved.map_or(false, |saved| saved == self.cursor)
    }

    /// Pushes the command to the top of the record and executes its [`redo`] method.
    /// The command is merged with the previous top command if they have the same [`id`].
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
    /// # use std::fmt::{self, Display, Formatter};
    /// # use undo::{Command, Record};
    /// #
    /// # #[derive(Debug)]
    /// # struct Add(char);
    /// #
    /// # impl Command<String> for Add {
    /// #     fn redo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
    /// #         s.push(self.0);
    /// #         Ok(())
    /// #     }
    /// #
    /// #     fn undo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
    /// #         self.0 = s.pop().ok_or("`String` is unexpectedly empty")?;
    /// #         Ok(())
    /// #     }
    /// # }
    /// #
    /// # impl Display for Add {
    /// #     fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    /// #         write!(f, "Add `{}`", self.0)
    /// #     }
    /// # }
    /// #
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
    /// [`id`]: trait.Command.html#method.id
    #[inline]
    pub fn push<C>(&mut self, mut cmd: C) -> Result<Commands<R>, Error<R>>
    where
        C: Command<R> + 'static,
        R: 'static,
    {
        match cmd.redo(&mut self.receiver) {
            Ok(_) => {
                let cursor = self.cursor;
                let was_dirty = cursor != self.len();
                let was_saved = self.is_saved();

                // Pop off all elements after cursor from record.
                let iter = self.commands.split_off(cursor).into_iter();
                debug_assert_eq!(cursor, self.len());

                // Check if the saved state was popped off.
                if let Some(saved) = self.saved {
                    if saved > cursor {
                        self.saved = None;
                    }
                }

                match (cmd.id(), self.commands.back().and_then(|last| last.id())) {
                    (Some(id1), Some(id2)) if id1 == id2 && !was_saved => {
                        // Merge the command with the one on the top of the stack.
                        let cmd = Merger {
                            cmd1: self.commands.pop_back().unwrap(),
                            cmd2: Box::new(cmd),
                        };
                        self.commands.push_back(Box::new(cmd));
                    }
                    _ => {
                        if self.limit != 0 && self.limit == cursor {
                            let _ = self.commands.pop_front().unwrap();
                            self.saved = match self.saved {
                                Some(0) => None,
                                Some(saved) => Some(saved - 1),
                                None => None,
                            };
                        } else {
                            self.cursor += 1;
                        }
                        self.commands.push_back(Box::new(cmd));
                    }
                }

                debug_assert_eq!(self.cursor, self.len());
                if let Some(ref mut f) = self.signals {
                    // We emit this signal even if the commands might have been merged.
                    f(Signal::Active { old: cursor, new: self.cursor });
                    // Record is always clean after a push, check if it was dirty before.
                    if was_dirty {
                        f(Signal::Redo(false));
                    }
                    // Check if the stack was empty before pushing the command.
                    if cursor == 0 {
                        f(Signal::Undo(true));
                    }
                    // Check if receiver went from saved to unsaved.
                    if was_saved {
                        f(Signal::Saved(false));
                    }
                }
                Ok(Commands(iter))
            }
            Err(e) => Err(Error(Box::new(cmd), e)),
        }
    }

    /// Calls the [`redo`] method for the active command and sets the next one as the
    /// new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the
    /// error is returned and the state is left unchanged.
    ///
    /// [`redo`]: trait.Command.html#tymethod.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result<(), Box<error::Error>>> {
        if self.cursor >= self.len() {
            return None;
        }

        match self.commands[self.cursor].redo(&mut self.receiver) {
            Ok(_) => {
                let was_dirty = self.cursor != self.len();
                let was_saved = self.is_saved();
                let old = self.cursor;
                self.cursor += 1;
                let is_clean = self.cursor == self.len();
                let is_saved = self.is_saved();
                if let Some(ref mut f) = self.signals {
                    // Cursor has always changed at this point.
                    f(Signal::Active { old, new: self.cursor });
                    // Check if record went from dirty to clean.
                    if was_dirty && is_clean {
                        f(Signal::Redo(false));
                    }
                    // Check if the stack was empty before pushing the command.
                    if self.cursor == 1 {
                        f(Signal::Undo(true));
                    }
                    // Check if receiver went from saved to unsaved, or unsaved to saved.
                    if was_saved {
                        f(Signal::Saved(false));
                    } else if is_saved {
                        f(Signal::Saved(true));
                    }
                }
                Some(Ok(()))
            }
            Err(e) => Some(Err(e)),
        }
    }

    /// Calls the [`undo`] method for the active command and sets the previous one as the
    /// new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the
    /// error is returned and the state is left unchanged.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result<(), Box<error::Error>>> {
        if self.cursor == 0 {
            return None;
        }

        match self.commands[self.cursor - 1].undo(&mut self.receiver) {
            Ok(_) => {
                let was_clean = self.cursor == self.len();
                let was_saved = self.is_saved();
                let old = self.cursor;
                self.cursor -= 1;
                let is_dirty = self.cursor != self.len();
                let is_saved = self.is_saved();
                if let Some(ref mut f) = self.signals {
                    // Cursor has always changed at this point.
                    f(Signal::Active { old, new: self.cursor });
                    // Check if record went from clean to dirty.
                    if was_clean && is_dirty {
                        f(Signal::Redo(true));
                    }
                    // Check if the stack was not empty before pushing the command.
                    if self.cursor == 0 {
                        f(Signal::Undo(false));
                    }
                    // Check if receiver went from saved to unsaved, or unsaved to saved.
                    if was_saved {
                        f(Signal::Saved(false));
                    } else if is_saved {
                        f(Signal::Saved(true));
                    }
                }
                Some(Ok(()))
            }
            Err(e) => Some(Err(e)),
        }
    }
}

impl<'a, R> AsRef<R> for Record<'a, R> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.as_receiver()
    }
}

impl<'a, R> From<R> for Record<'a, R> {
    #[inline]
    fn from(receiver: R) -> Self {
        Record::new(receiver)
    }
}

impl<'a, R: Debug> Debug for Record<'a, R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Record")
            .field("commands", &self.commands)
            .field("receiver", &self.receiver)
            .field("cursor", &self.cursor)
            .field("limit", &self.limit)
            .field("saved", &self.saved)
            .finish()
    }
}

impl<'a, R> Display for Record<'a, R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        for (idx, cmd) in self.commands.iter().rev().enumerate() {
            if idx + 1 == self.cursor {
                writeln!(f, "-> {}.", cmd)?;
            } else {
                writeln!(f, "   {}.", cmd)?;
            }
        }
        Ok(())
    }
}

/// Iterator over commands.
#[derive(Debug)]
pub struct Commands<R>(IntoIter<Box<Command<R>>>);

impl<R> Iterator for Commands<R> {
    type Item = Box<Command<R>>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// Builder for a record.
pub struct RecordBuilder<'a, R> {
    receiver: PhantomData<R>,
    capacity: usize,
    limit: usize,
    signals: Option<Box<FnMut(Signal) + Send + Sync + 'a>>,
}

impl<'a, R> RecordBuilder<'a, R> {
    /// Sets the specified [capacity] for the stack.
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> RecordBuilder<'a, R> {
        self.capacity = capacity;
        self
    }

    /// Sets a limit on how many commands can be stored in the stack.
    /// If this limit is reached it will start popping of commands at the bottom of the stack when
    /// pushing new commands on to the stack. No limit is set by default which means it may grow
    /// indefinitely.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # use std::fmt::{self, Display, Formatter};
    /// # use undo::{Command, Record};
    /// #
    /// # #[derive(Debug)]
    /// # struct Add(char);
    /// #
    /// # impl Command<String> for Add {
    /// #     fn redo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
    /// #         s.push(self.0);
    /// #         Ok(())
    /// #     }
    /// #
    /// #     fn undo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
    /// #         self.0 = s.pop().ok_or("`String` is unexpectedly empty")?;
    /// #         Ok(())
    /// #     }
    /// # }
    /// #
    /// # impl Display for Add {
    /// #     fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    /// #         write!(f, "Add `{}`", self.0)
    /// #     }
    /// # }
    /// #
    /// # fn foo() -> Result<(), Box<Error>> {
    /// let mut record = Record::builder()
    ///     .capacity(2)
    ///     .limit(2)
    ///     .default();
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
    pub fn limit(mut self, limit: usize) -> RecordBuilder<'a, R> {
        self.limit = limit;
        self
    }

    /// Decides how different signals should be handled when the state changes.
    /// By default the record does nothing.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # use std::fmt::{self, Display, Formatter};
    /// # use undo::{Command, Record, Signal};
    /// #
    /// # #[derive(Debug)]
    /// # struct Add(char);
    /// #
    /// # impl Command<String> for Add {
    /// #     fn redo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
    /// #         s.push(self.0);
    /// #         Ok(())
    /// #     }
    /// #
    /// #     fn undo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
    /// #         self.0 = s.pop().ok_or("`String` is unexpectedly empty")?;
    /// #         Ok(())
    /// #     }
    /// # }
    /// #
    /// # impl Display for Add {
    /// #     fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    /// #         write!(f, "Add `{}`", self.0)
    /// #     }
    /// # }
    /// #
    /// # fn foo() -> Result<(), Box<Error>> {
    /// # let mut record =
    /// Record::builder()
    ///     .signals(|signal| {
    ///         match signal {
    ///             Signal::Redo(true) => println!("The record can redo."),
    ///             Signal::Redo(false) => println!("The record can not redo."),
    ///             Signal::Undo(true) => println!("The record can undo."),
    ///             Signal::Undo(false) => println!("The record can not undo."),
    ///             Signal::Saved(true) => println!("The receiver is in a saved state."),
    ///             Signal::Saved(false) => println!("The receiver is not in a saved state."),
    ///             Signal::Active { old, new } => {
    ///                 println!("The active command has changed from {} to {}.", old, new);
    ///             }
    ///         }
    ///     })
    ///     .default();
    /// # record.push(Add('a'))?;
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn signals<F>(mut self, f: F) -> RecordBuilder<'a, R>
    where
        F: FnMut(Signal) + Send + Sync + 'a,
    {
        self.signals = Some(Box::new(f));
        self
    }

    /// Creates the record.
    #[inline]
    pub fn build<T: Into<R>>(self, receiver: T) -> Record<'a, R> {
        Record {
            commands: VecDeque::with_capacity(self.capacity),
            receiver: receiver.into(),
            cursor: 0,
            limit: self.limit,
            saved: None,
            signals: self.signals,
        }
    }
}

impl<'a, R: Default> RecordBuilder<'a, R> {
    /// Creates the record with a default `receiver`.
    #[inline]
    pub fn default(self) -> Record<'a, R> {
        self.build(R::default())
    }
}

impl<'a, R: Debug> Debug for RecordBuilder<'a, R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("RecordBuilder")
            .field("receiver", &self.receiver)
            .field("capacity", &self.capacity)
            .field("limit", &self.limit)
            .finish()
    }
}
