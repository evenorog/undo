use std::collections::vec_deque::VecDeque;
use std::fmt::{self, Debug, Formatter};
#[cfg(feature = "display")]
use std::fmt::Display;
use std::marker::PhantomData;
use {Command, Error, merge::Merged};

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
    /// Says if the current command has changed.
    ///
    /// This signal will be emitted when the records cursor has changed. This includes
    /// when two commands have been merged, in which case `old == new`.
    Cursor {
        /// The position of the old command.
        old: usize,
        /// The position of the new command.
        new: usize,
    },
}

/// A record of commands.
///
/// The record can roll the receivers state backwards and forwards by using
/// the undo and redo methods. In addition, the record can notify the user
/// about changes to the stack or the receiver through [signals]. The user
/// can give the record a function that is called each time the state changes
/// by using the [`builder`].
///
/// # Examples
/// ```
/// # use std::error::Error;
/// # use undo::*;
/// #[derive(Debug)]
/// struct Add(char);
///
/// impl Command<String> for Add {
///     fn apply(&mut self, s: &mut String) -> Result<(), Box<Error>> {
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
/// fn main() -> Result<(), Box<Error>> {
///     let mut record = Record::default();
///
///     record.apply(Add('a'))?;
///     record.apply(Add('b'))?;
///     record.apply(Add('c'))?;
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
/// ```
///
/// [`builder`]: struct.RecordBuilder.html
/// [signals]: enum.Signal.html
pub struct Record<R> {
    commands: VecDeque<Box<Command<R> + 'static>>,
    receiver: R,
    cursor: usize,
    limit: usize,
    saved: Option<usize>,
    signals: Option<Box<FnMut(Signal) + Send + Sync + 'static>>,
}

impl<R> Record<R> {
    /// Returns a new record.
    #[inline]
    pub fn new<T: Into<R>>(receiver: T) -> Record<R> {
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
    pub fn builder() -> RecordBuilder<R> {
        RecordBuilder {
            receiver: PhantomData,
            capacity: 0,
            limit: 0,
            signals: None,
        }
    }

    /// Reserves capacity for at least `additional` more commands.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.commands.reserve(additional);
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

    /// Returns the limit of the record.
    #[inline]
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Sets the limit of the record and returns the new limit.
    ///
    /// If `limit < len` the first commands will be removed until `len == limit`.
    /// However, if the current active command is going to be removed, the limit is instead
    /// adjusted to `len - active` so that the active command is not popped off.
    #[inline]
    pub fn set_limit(&mut self, limit: usize) -> usize {
        if limit > 0 && limit < self.len() {
            let old = self.cursor;
            let could_undo = self.can_undo();
            let was_saved = self.is_saved();

            let begin = usize::min(self.cursor, self.len() - limit);
            self.commands = self.commands.split_off(begin);
            self.limit = self.len();
            self.cursor -= begin;

            // Check if the saved state has been removed.
            if self.saved.map_or(false, |saved| saved > 0 && saved < begin) {
                self.saved = None;
            }

            let new = self.cursor;
            let can_undo = self.can_undo();
            let is_saved = self.is_saved();
            if let Some(ref mut f) = self.signals {
                // Emit signal if the cursor has changed.
                if old != new { f(Signal::Cursor { old, new }) }
                // Check if the records ability to undo changed.
                if could_undo != can_undo { f(Signal::Undo(can_undo)) }
                // Check if the receiver went from saved to unsaved.
                if was_saved != is_saved { f(Signal::Saved(is_saved)) }
            }
        } else {
            self.limit = limit;
        }
        self.limit
    }

    /// Sets how different signals should be handled when the state changes.
    #[inline]
    pub fn set_signals<F>(&mut self, f: F)
        where
            F: FnMut(Signal) + Send + Sync + 'static,
    {
        self.signals = Some(Box::new(f));
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
            if !was_saved { f(Signal::Saved(true)); }
        }
    }

    /// Marks the receiver as no longer being in a saved state.
    #[inline]
    pub fn set_unsaved(&mut self) {
        let was_saved = self.is_saved();
        self.saved = None;
        if let Some(ref mut f) = self.signals {
            // Check if the receiver went from saved to unsaved.
            if was_saved { f(Signal::Saved(false)); }
        }
    }

    /// Returns `true` if the receiver is in a saved state, `false` otherwise.
    #[inline]
    pub fn is_saved(&self) -> bool {
        self.saved.map_or(false, |saved| saved == self.cursor)
    }

    /// Removes all commands from the record without undoing them.
    #[inline]
    pub fn clear(&mut self) {
        let old = self.cursor;
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();

        self.commands.clear();
        self.cursor = 0;
        self.saved = Some(0);

        if let Some(ref mut f) = self.signals {
            // Emit signal if the cursor has changed.
            if old != 0 { f(Signal::Cursor { old, new: 0 }); }
            // Record can never undo after being cleared, check if you could undo before.
            if could_undo { f(Signal::Undo(false)); }
            // Record can never redo after being cleared, check if you could redo before.
            if could_redo { f(Signal::Redo(false)); }
            // Check if the receiver went from unsaved to saved.
            if !was_saved { f(Signal::Saved(true)); }
        }
    }

    /// Pushes the command to the top of the record and executes its [`apply`] method.
    /// The command is merged with the previous top command if they have the same [`id`].
    ///
    /// All commands above the active one are removed from the stack and returned as an iterator.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned together with the command.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    /// [`id`]: trait.Command.html#method.id
    #[inline]
    pub fn apply<C>(&mut self, cmd: C) -> Result<impl Iterator<Item=Box<Command<R> + 'static>>, Error<R>>
        where
            C: Command<R> + 'static,
            R: 'static,
    {
        let mut cmd = Box::new(cmd);
        if let Err(err) = cmd.apply(&mut self.receiver) {
            return Err(Error(cmd, err));
        }

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

        // Try to merge commands unless the receiver is in a saved state.
        match (cmd.id(), self.commands.back().and_then(|last| last.id())) {
            (Some(id1), Some(id2)) if id1 == id2 && !was_saved => {
                // Merge the command with the one on the top of the stack.
                let merged = Merged {
                    cmd1: self.commands.pop_back().unwrap(),
                    cmd2: cmd,
                };
                self.commands.push_back(Box::new(merged));
            }
            _ => {
                // If commands are not merged push it onto the record.
                if self.limit != 0 && self.limit == self.cursor {
                    // If limit is reached, pop off the first command.
                    self.commands.pop_front();
                    self.saved = self.saved.and_then(|saved| saved.checked_sub(1));
                } else {
                    self.cursor += 1;
                }
                self.commands.push_back(cmd);
            }
        }

        debug_assert_eq!(self.cursor, self.len());
        if let Some(ref mut f) = self.signals {
            // We emit this signal even if the commands might have been merged.
            f(Signal::Cursor { old, new: self.cursor });
            // Record can never redo after executing a command, check if you could redo before.
            if could_redo { f(Signal::Redo(false)); }
            // Record can always undo after executing a command, check if you could not undo before.
            if !could_undo { f(Signal::Undo(true)); }
            // Check if the receiver went from saved to unsaved.
            if was_saved { f(Signal::Saved(false)); }
        }
        Ok(iter)
    }

    /// Calls the [`undo`] method for the active command and sets the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned together with the command.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result<(), Error<R>>> {
        if !self.can_undo() {
            return None;
        }

        if let Err(err) = self.commands[self.cursor - 1].undo(&mut self.receiver) {
            let cmd = self.commands.remove(self.cursor - 1).unwrap();
            return Some(Err(Error(cmd, err)));
        }

        let was_saved = self.is_saved();
        let old = self.cursor;
        self.cursor -= 1;
        let len = self.len();
        let is_saved = self.is_saved();
        if let Some(ref mut f) = self.signals {
            // Cursor has always changed at this point.
            f(Signal::Cursor { old, new: self.cursor });
            // Check if the records ability to redo changed.
            if old == len { f(Signal::Redo(true)); }
            // Check if the records ability to undo changed.
            if old == 1 { f(Signal::Undo(false)); }
            // Check if the receiver went from saved to unsaved, or unsaved to saved.
            if was_saved != is_saved { f(Signal::Saved(is_saved)); }
        }
        Some(Ok(()))
    }

    /// Calls the [`redo`] method for the active command and sets the next one as the
    /// new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the error is returned together with the command.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result<(), Error<R>>> {
        if !self.can_redo() {
            return None;
        }

        if let Err(err) = self.commands[self.cursor].redo(&mut self.receiver) {
            let cmd = self.commands.remove(self.cursor).unwrap();
            return Some(Err(Error(cmd, err)));
        }

        let was_saved = self.is_saved();
        let old = self.cursor;
        self.cursor += 1;
        let len = self.len();
        let is_saved = self.is_saved();
        if let Some(ref mut f) = self.signals {
            // Cursor has always changed at this point.
            f(Signal::Cursor { old, new: self.cursor });
            // Check if the records ability to redo changed.
            if old == len - 1 { f(Signal::Redo(false)); }
            // Check if the records ability to undo changed.
            if old == 0 { f(Signal::Undo(true)); }
            // Check if the receiver went from saved to unsaved, or unsaved to saved.
            if was_saved != is_saved { f(Signal::Saved(is_saved)); }
        }
        Some(Ok(()))
    }

    /// Returns the position of the current command.
    #[inline]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Repeatedly calls [`undo`] or [`redo`] until the command at `cursor` is reached.
    /// The signals are emitted once after reaching the `cursor`.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned together with the command.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    pub fn set_cursor(&mut self, cursor: usize) -> Option<Result<(), Error<R>>> {
        if cursor > self.len() {
            return None;
        }

        let was_saved = self.is_saved();
        let old = self.cursor;
        let len = self.len();
        // Temporarily remove signals so they are not called each iteration.
        let signals = self.signals.take();
        // Decide if we need to undo or redo to reach cursor.
        let redo = cursor > self.cursor;
        let f = if redo { Record::redo } else { Record::undo };
        while self.cursor != cursor {
            if let Err(err) = f(self).unwrap() {
                self.signals = signals;
                return Some(Err(err));
            }
        }
        // Add signals back.
        self.signals = signals;
        let is_saved = self.is_saved();
        if let Some(ref mut f) = self.signals {
            // Emit signal if the cursor has changed.
            if old != self.cursor { f(Signal::Cursor { old, new: self.cursor }); }
            // Check if the receiver went from saved to unsaved, or unsaved to saved.
            if was_saved != is_saved { f(Signal::Saved(is_saved)); }
            if redo {
                // Check if the records ability to redo changed.
                if old == len - 1 { f(Signal::Redo(false)); }
                // Check if the records ability to undo changed.
                if old == 0 { f(Signal::Undo(true)); }
            } else {
                // Check if the records ability to redo changed.
                if old == len { f(Signal::Redo(true)); }
                // Check if the records ability to undo changed.
                if old == 1 { f(Signal::Undo(false)); }
            }
        }
        Some(Ok(()))
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

    /// Returns a mutable reference to the `receiver`.
    ///
    /// This method should **only** be used when doing changes that should not be able to be undone.
    #[inline]
    pub fn as_mut_receiver(&mut self) -> &mut R {
        &mut self.receiver
    }

    /// Consumes the record, returning the `receiver`.
    #[inline]
    pub fn into_receiver(self) -> R {
        self.receiver
    }
}

impl<R: Default> Default for Record<R> {
    #[inline]
    fn default() -> Record<R> {
        Record::new(R::default())
    }
}

impl<R> AsRef<R> for Record<R> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.as_receiver()
    }
}

impl<R> AsMut<R> for Record<R> {
    #[inline]
    fn as_mut(&mut self) -> &mut R {
        self.as_mut_receiver()
    }
}

impl<R> From<R> for Record<R> {
    #[inline]
    fn from(receiver: R) -> Self {
        Record::new(receiver)
    }
}

impl<R: Debug> Debug for Record<R> {
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
impl<R> Display for Record<R> {
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

/// Builder for a record.
pub struct RecordBuilder<R> {
    receiver: PhantomData<R>,
    capacity: usize,
    limit: usize,
    signals: Option<Box<FnMut(Signal) + Send + Sync + 'static>>,
}

impl<R> RecordBuilder<R> {
    /// Sets the specified [capacity] for the record.
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> RecordBuilder<R> {
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
    /// #     fn apply(&mut self, s: &mut String) -> Result<(), Box<Error>> {
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
    /// record.apply(Add('a'))?;
    /// record.apply(Add('b'))?;
    /// record.apply(Add('c'))?; // 'a' is removed from the record since limit is 2.
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
    pub fn limit(mut self, limit: usize) -> RecordBuilder<R> {
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
    /// #     fn apply(&mut self, s: &mut String) -> Result<(), Box<Error>> {
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
    /// # fn main() -> Result<(), Box<Error>> {
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
    ///             Signal::Cursor { old, new } => {
    ///                 println!("The current command has changed from {} to {}.", old, new);
    ///             }
    ///         }
    ///     })
    ///     .default();
    /// # record.apply(Add('a'))?;
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn signals<F>(mut self, f: F) -> RecordBuilder<R>
        where
            F: FnMut(Signal) + Send + Sync + 'static,
    {
        self.signals = Some(Box::new(f));
        self
    }

    /// Creates the record.
    #[inline]
    pub fn build<T: Into<R>>(self, receiver: T) -> Record<R> {
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

impl<R: Default> RecordBuilder<R> {
    /// Creates the record with a default `receiver`.
    #[inline]
    pub fn default(self) -> Record<R> {
        self.build(R::default())
    }
}

impl<R: Debug> Debug for RecordBuilder<R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("RecordBuilder")
            .field("receiver", &self.receiver)
            .field("capacity", &self.capacity)
            .field("limit", &self.limit)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use super::*;

    #[derive(Debug)]
    struct Add(char);

    impl Command<String> for Add {
        fn apply(&mut self, receiver: &mut String) -> Result<(), Box<Error>> {
            receiver.push(self.0);
            Ok(())
        }

        fn undo(&mut self, receiver: &mut String) -> Result<(), Box<Error>> {
            self.0 = receiver.pop().ok_or("`receiver` is empty")?;
            Ok(())
        }
    }

    #[test]
    fn set_limit() {
        let mut record = Record::default();
        record.apply(Add('a')).unwrap();
        record.apply(Add('b')).unwrap();
        record.apply(Add('c')).unwrap();
        record.apply(Add('d')).unwrap();
        record.apply(Add('e')).unwrap();

        record.set_limit(3);
        assert_eq!(record.cursor(), 3);
        assert_eq!(record.limit(), 3);
        assert_eq!(record.len(), 3);
        assert!(record.can_undo());
        assert!(!record.can_redo());

        record.clear();
        assert_eq!(record.set_limit(5), 5);
        record.apply(Add('a')).unwrap();
        record.apply(Add('b')).unwrap();
        record.apply(Add('c')).unwrap();
        record.apply(Add('d')).unwrap();
        record.apply(Add('e')).unwrap();

        record.undo().unwrap().unwrap();
        record.undo().unwrap().unwrap();
        record.undo().unwrap().unwrap();

        record.set_limit(2);
        assert_eq!(record.cursor(), 0);
        assert_eq!(record.limit(), 3);
        assert_eq!(record.len(), 3);
        assert!(!record.can_undo());
        assert!(record.can_redo());

        record.redo().unwrap().unwrap();
        record.redo().unwrap().unwrap();
        record.redo().unwrap().unwrap();

        record.clear();
        assert_eq!(record.set_limit(5), 5);
        record.apply(Add('a')).unwrap();
        record.apply(Add('b')).unwrap();
        record.apply(Add('c')).unwrap();
        record.apply(Add('d')).unwrap();
        record.apply(Add('e')).unwrap();

        record.undo().unwrap().unwrap();
        record.undo().unwrap().unwrap();
        record.undo().unwrap().unwrap();
        record.undo().unwrap().unwrap();
        record.undo().unwrap().unwrap();

        record.set_limit(2);
        assert_eq!(record.cursor(), 0);
        assert_eq!(record.limit(), 5);
        assert_eq!(record.len(), 5);
        assert!(!record.can_undo());
        assert!(record.can_redo());

        record.redo().unwrap().unwrap();
        record.redo().unwrap().unwrap();
        record.redo().unwrap().unwrap();
        record.redo().unwrap().unwrap();
        record.redo().unwrap().unwrap();
    }

    #[test]
    fn set_cursor() {
        let mut record = Record::default();
        record.apply(Add('a')).unwrap();
        record.apply(Add('b')).unwrap();
        record.apply(Add('c')).unwrap();
        record.apply(Add('d')).unwrap();
        record.apply(Add('e')).unwrap();

        record.set_cursor(0).unwrap().unwrap();
        assert_eq!(record.cursor(), 0);
        assert_eq!(record.as_receiver(), "");
        record.set_cursor(1).unwrap().unwrap();
        assert_eq!(record.cursor(), 1);
        assert_eq!(record.as_receiver(), "a");
        record.set_cursor(2).unwrap().unwrap();
        assert_eq!(record.cursor(), 2);
        assert_eq!(record.as_receiver(), "ab");
        record.set_cursor(3).unwrap().unwrap();
        assert_eq!(record.cursor(), 3);
        assert_eq!(record.as_receiver(), "abc");
        record.set_cursor(4).unwrap().unwrap();
        assert_eq!(record.cursor(), 4);
        assert_eq!(record.as_receiver(), "abcd");
        record.set_cursor(5).unwrap().unwrap();
        assert_eq!(record.cursor(), 5);
        assert_eq!(record.as_receiver(), "abcde");
        assert!(record.set_cursor(6).is_none());
        assert_eq!(record.cursor(), 5);
    }

    #[test]
    fn signals() {
        use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};

        let mut record = Record::default();
        let undo = Arc::new(AtomicBool::new(false));
        let redo = Arc::new(AtomicBool::new(false));
        let saved = Arc::new(AtomicBool::new(false));
        let cursor = Arc::new(AtomicUsize::new(0));
        {
            let undo = undo.clone();
            let redo = redo.clone();
            let saved = saved.clone();
            let cursor = cursor.clone();
            record.set_signals(move |signal| {
                match signal {
                    Signal::Undo(x) => undo.store(x, Ordering::Relaxed),
                    Signal::Redo(x) => redo.store(x, Ordering::Relaxed),
                    Signal::Saved(x) => saved.store(x, Ordering::Relaxed),
                    Signal::Cursor { new, .. } => cursor.store(new, Ordering::Relaxed),
                }
            });
        }

        record.apply(Add('a')).unwrap();
        assert_eq!(undo.load(Ordering::Relaxed), true);
        assert_eq!(redo.load(Ordering::Relaxed), false);
        assert_eq!(saved.load(Ordering::Relaxed), false);
        assert_eq!(cursor.load(Ordering::Relaxed), 1);

        record.undo().unwrap().unwrap();
        assert_eq!(undo.load(Ordering::Relaxed), false);
        assert_eq!(redo.load(Ordering::Relaxed), true);
        assert_eq!(saved.load(Ordering::Relaxed), true);
        assert_eq!(cursor.load(Ordering::Relaxed), 0);

        record.redo().unwrap().unwrap();
        assert_eq!(undo.load(Ordering::Relaxed), true);
        assert_eq!(redo.load(Ordering::Relaxed), false);
        assert_eq!(saved.load(Ordering::Relaxed), false);
        assert_eq!(cursor.load(Ordering::Relaxed), 1);

        record.clear();
        assert_eq!(undo.load(Ordering::Relaxed), false);
        assert_eq!(redo.load(Ordering::Relaxed), false);
        assert_eq!(saved.load(Ordering::Relaxed), true);
        assert_eq!(cursor.load(Ordering::Relaxed), 0);
    }
}
