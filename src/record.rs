use std::collections::vec_deque::{IntoIter, VecDeque};
use std::error;
use std::fmt::{self, Debug, Formatter};
#[cfg(feature = "display")]
use std::fmt::Display;
use std::marker::PhantomData;
use {Command, Error, Merger};

/// The signals sent when the record or the receiver changes.
///
/// When one of these states changes in the record or the receiver, they will send a corresponding
/// signal to the user. For example, if the record can no longer redo any commands, it sends a
/// `Signal::Redo(false)` signal to tell the user. The signals can be handled in the [`signals`]
/// method.
///
/// [`signals`]: struct.RecordBuilder.html#method.signals
#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum Signal {
    /// Says if the record can undo.
    ///
    /// This signal will be emitted when the records ability to undo changes.
    Undo(bool),
    /// Says if the record can redo.
    ///
    /// This signal will be emitted when the records ability to redo changes.
    Redo(bool),
    /// Says if the receiver is in a saved state.
    ///
    /// This signal will be emitted when the record enters or leaves its receivers saved state.
    Saved(bool),
    /// Says if the active command has changed.
    ///
    /// This signal will be emitted when the records active command has changed. This includes
    /// when two commands have been merged, in which case `old == new`.
    /// The `old` and `new` fields are always `index + 1`.
    Active { old: usize, new: usize },
}

/// The command record.
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
/// # use std::error::Error;
/// # use undo::*;
/// #[derive(Debug)]
/// struct Add(char);
///
/// impl Command<String> for Add {
///     fn exec(&mut self, s: &mut String) -> Result<(), Box<Error>> {
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
///     record.exec(Add('a'))?;
///     record.exec(Add('b'))?;
///     record.exec(Add('c'))?;
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
            saved: Some(0),
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

    /// Returns the limit of the record, or `None` if it has no limit.
    #[inline]
    pub fn limit(&self) -> Option<usize> {
        match self.limit {
            0 => None,
            v => Some(v),
        }
    }

    /// Returns `true` if the record can undo.
    #[inline]
    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    /// Returns `true` if the record can redo.
    #[inline]
    pub fn can_redo(&self) -> bool {
        self.cursor < self.len()
    }

    /// Marks the receiver as currently being in a saved state.
    #[inline]
    pub fn set_saved(&mut self) {
        let was_saved = self.is_saved();
        self.saved = Some(self.cursor);
        if let Some(ref mut f) = self.signals {
            // Check if the receiver went from unsaved to saved.
            if !was_saved {
                f(Signal::Saved(true));
            }
        }
    }

    /// Marks the receiver as no longer being in a saved state.
    #[inline]
    pub fn set_unsaved(&mut self) {
        let was_saved = self.is_saved();
        self.saved = None;
        if let Some(ref mut f) = self.signals {
            // Check if the receiver went from saved to unsaved.
            if was_saved {
                f(Signal::Saved(false));
            }
        }
    }

    /// Returns `true` if the receiver is in a saved state, `false` otherwise.
    #[inline]
    pub fn is_saved(&self) -> bool {
        self.saved.map_or(false, |saved| saved == self.cursor)
    }

    /// Removes all commands from the record without undoing them.
    ///
    /// This resets the record back to its initial state and emits the appropriate signals,
    /// while leaving the receiver unmodified.
    #[inline]
    pub fn clear(&mut self) {
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();

        let old = self.cursor;
        self.commands.clear();
        self.cursor = 0;
        self.saved = Some(0);

        if let Some(ref mut f) = self.signals {
            // Emit signal if the cursor has changed.
            if old != 0 {
                f(Signal::Active { old, new: 0 });
            }
            // Record can never undo after being cleared, check if you could undo before.
            if could_undo {
                f(Signal::Undo(false));
            }
            // Record can never redo after being cleared, check if you could redo before.
            if could_redo {
                f(Signal::Redo(false));
            }
            // Check if the receiver went from unsaved to saved.
            if !was_saved {
                f(Signal::Saved(true));
            }
        }
    }

    /// Pushes the command to the top of the record and executes its [`exec`] method.
    /// The command is merged with the previous top command if they have the same [`id`].
    ///
    /// All commands above the active one are removed from the stack and returned as an iterator.
    ///
    /// # Errors
    /// If an error occur when executing [`exec`] the error is returned together with the command,
    /// and the state of the stack is left unchanged.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # use undo::*;
    /// #
    /// # #[derive(Debug)]
    /// # struct Add(char);
    /// #
    /// # impl Command<String> for Add {
    /// #     fn exec(&mut self, s: &mut String) -> Result<(), Box<Error>> {
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
    /// # fn foo() -> Result<(), Box<Error>> {
    /// let mut record = Record::default();
    ///
    /// record.exec(Add('a'))?;
    /// record.exec(Add('b'))?;
    /// record.exec(Add('c'))?;
    ///
    /// assert_eq!(record.as_receiver(), "abc");
    ///
    /// record.undo().unwrap()?;
    /// record.undo().unwrap()?;
    /// let mut bc = record.exec(Add('e'))?;
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
    /// [`exec`]: trait.Command.html#tymethod.exec
    /// [`id`]: trait.Command.html#method.id
    #[inline]
    pub fn exec<C>(&mut self, mut cmd: C) -> Result<Commands<R>, Error<R>>
    where
        C: Command<R> + 'static,
        R: 'static,
    {
        match cmd.exec(&mut self.receiver) {
            Ok(_) => {
                let old = self.cursor;
                let could_undo = self.can_undo();
                let could_redo = self.can_redo();
                let was_saved = self.is_saved();

                // Pop off all elements after cursor from record.
                let iter = self.commands.split_off(self.cursor).into_iter();
                debug_assert_eq!(self.cursor, self.len());

                // Check if the saved state was popped off.
                if self.saved.map_or(false, |saved| saved > self.cursor) {
                    self.saved = None;
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
                        if self.limit != 0 && self.limit == self.cursor {
                            self.commands.pop_front();
                            self.saved = self.saved.and_then(|saved| saved.checked_sub(1));
                        } else {
                            self.cursor += 1;
                        }
                        self.commands.push_back(Box::new(cmd));
                    }
                }

                debug_assert_eq!(self.cursor, self.len());
                if let Some(ref mut f) = self.signals {
                    // We emit this signal even if the commands might have been merged.
                    f(Signal::Active { old, new: self.cursor });
                    // Record can never redo after executing a command, check if you could redo before.
                    if could_redo {
                        f(Signal::Redo(false));
                    }
                    // Record can always undo after executing a command, check if you could not undo before.
                    if !could_undo {
                        f(Signal::Undo(true));
                    }
                    // Check if the receiver went from saved to unsaved.
                    if was_saved {
                        f(Signal::Saved(false));
                    }
                }
                Ok(Commands(iter))
            }
            Err(e) => Err(Error(Box::new(cmd), e)),
        }
    }

    /// Calls the [`undo`] method for the active command and sets the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned and the state is left unchanged.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result<(), Box<error::Error>>> {
        if !self.can_undo() {
            return None;
        }

        let result = self.commands[self.cursor - 1].undo(&mut self.receiver).map(|_| {
            let was_saved = self.is_saved();
            let old = self.cursor;
            self.cursor -= 1;
            let len = self.len();
            let is_saved = self.is_saved();
            if let Some(ref mut f) = self.signals {
                // Cursor has always changed at this point.
                f(Signal::Active { old, new: self.cursor });
                // Check if the records ability to redo changed.
                if old == len {
                    f(Signal::Redo(true));
                }
                // Check if the records ability to undo changed.
                if old == 1 {
                    f(Signal::Undo(false));
                }
                // Check if the receiver went from saved to unsaved, or unsaved to saved.
                if was_saved != is_saved {
                    f(Signal::Saved(is_saved));
                }
            }
        });
        Some(result)
    }

    /// Calls the [`exec`] method for the active command and sets the next one as the
    /// new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`exec`] the
    /// error is returned and the state is left unchanged.
    ///
    /// [`exec`]: trait.Command.html#tymethod.exec
    #[inline]
    pub fn redo(&mut self) -> Option<Result<(), Box<error::Error>>> {
        if !self.can_redo() {
            return None;
        }

        let result = self.commands[self.cursor].exec(&mut self.receiver).map(|_| {
            let was_saved = self.is_saved();
            let old = self.cursor;
            self.cursor += 1;
            let len = self.len();
            let is_saved = self.is_saved();
            if let Some(ref mut f) = self.signals {
                // Cursor has always changed at this point.
                f(Signal::Active { old, new: self.cursor });
                // Check if the records ability to redo changed.
                if old == len - 1 {
                    f(Signal::Redo(false));
                }
                // Check if the records ability to undo changed.
                if old == 0 {
                    f(Signal::Undo(true));
                }
                // Check if the receiver went from saved to unsaved, or unsaved to saved.
                if was_saved != is_saved {
                    f(Signal::Saved(is_saved));
                }
            }
        });
        Some(result)
    }

    /// Returns the string of the command which will be undone in the next call to [`undo`].
    ///
    /// [`undo`]: struct.Record.html#method.undo
    #[inline]
    #[cfg(feature = "display")]
    pub fn to_undo_string(&self) -> Option<String> {
        if self.can_undo() {
            Some(self.commands[self.cursor - 1].to_string())
        } else {
            None
        }
    }

    /// Returns the string of the command which will be redone in the next call to [`redo`].
    ///
    /// [`redo`]: struct.Record.html#method.redo
    #[inline]
    #[cfg(feature = "display")]
    pub fn to_redo_string(&self) -> Option<String> {
        if self.can_redo() {
            Some(self.commands[self.cursor].to_string())
        } else {
            None
        }
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
}

impl<'a, R: Default> Default for Record<'a, R> {
    #[inline]
    fn default() -> Record<'a, R> {
        Record::new(R::default())
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

#[cfg(feature = "display")]
impl<'a, R> Display for Record<'a, R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        for (i, cmd) in self.commands.iter().enumerate().rev() {
            if i + 1 == self.cursor {
                writeln!(f, "* {}", cmd)?;
            } else {
                writeln!(f, "  {}", cmd)?;
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

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
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
    /// Sets the specified [capacity] for the record.
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> RecordBuilder<'a, R> {
        self.capacity = capacity;
        self
    }

    /// Sets the `limit` for the record.
    ///
    /// If this limit is reached it will start popping of commands at the beginning
    /// of the record when pushing new commands on to the stack. No limit is set by
    /// default which means it may grow indefinitely.
    ///
    /// # Examples
    /// ```
    /// # use std::error::Error;
    /// # use undo::*;
    /// #
    /// # #[derive(Debug)]
    /// # struct Add(char);
    /// #
    /// # impl Command<String> for Add {
    /// #     fn exec(&mut self, s: &mut String) -> Result<(), Box<Error>> {
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
    /// # fn foo() -> Result<(), Box<Error>> {
    /// let mut record = Record::builder()
    ///     .capacity(2)
    ///     .limit(2)
    ///     .default();
    ///
    /// record.exec(Add('a'))?;
    /// record.exec(Add('b'))?;
    /// record.exec(Add('c'))?; // 'a' is removed from the record since limit is 2.
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
    /// # use undo::*;
    /// #
    /// # #[derive(Debug)]
    /// # struct Add(char);
    /// #
    /// # impl Command<String> for Add {
    /// #     fn exec(&mut self, s: &mut String) -> Result<(), Box<Error>> {
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
    /// # fn foo() -> Result<(), Box<Error>> {
    /// # let mut record =
    /// Record::builder()
    ///     .signals(|signal| {
    ///         match signal {
    ///             Signal::Undo(true) => println!("The record can undo."),
    ///             Signal::Undo(false) => println!("The record can not undo."),
    ///             Signal::Redo(true) => println!("The record can redo."),
    ///             Signal::Redo(false) => println!("The record can not redo."),
    ///             Signal::Saved(true) => println!("The receiver is in a saved state."),
    ///             Signal::Saved(false) => println!("The receiver is not in a saved state."),
    ///             Signal::Active { old, new } => {
    ///                 println!("The active command has changed from {} to {}.", old, new);
    ///             }
    ///         }
    ///     })
    ///     .default();
    /// # record.exec(Add('a'))?;
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
            saved: Some(0),
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
