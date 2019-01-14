#[cfg(feature = "display")]
use crate::Display;
use crate::{Checkpoint, Command, History, Merge, Merged, Meta, Queue, Result, Signal};
#[cfg(feature = "chrono")]
use chrono::{DateTime, TimeZone, Utc};
#[cfg(feature = "chrono")]
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::marker::PhantomData;
use std::num::NonZeroUsize;

#[allow(unsafe_code)]
const MAX_LIMIT: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(usize::max_value()) };

/// A record of commands.
///
/// The record can roll the receivers state backwards and forwards by using
/// the undo and redo methods. In addition, the record can notify the user
/// about changes to the stack or the receiver through [signal]. The user
/// can give the record a function that is called each time the state changes
/// by using the [`builder`].
///
/// # Examples
/// ```
/// # use undo::{Command, Record};
/// #[derive(Debug)]
/// struct Add(char);
///
/// impl Command<String> for Add {
///     fn apply(&mut self, s: &mut String) -> undo::Result {
///         s.push(self.0);
///         Ok(())
///     }
///
///     fn undo(&mut self, s: &mut String) -> undo::Result {
///         self.0 = s.pop().ok_or("`s` is empty")?;
///         Ok(())
///     }
/// }
///
/// fn main() -> undo::Result {
///     let mut record = Record::default();
///     record.apply(Add('a'))?;
///     record.apply(Add('b'))?;
///     record.apply(Add('c'))?;
///     assert_eq!(record.as_receiver(), "abc");
///     record.undo().unwrap()?;
///     record.undo().unwrap()?;
///     record.undo().unwrap()?;
///     assert_eq!(record.as_receiver(), "");
///     record.redo().unwrap()?;
///     record.redo().unwrap()?;
///     record.redo().unwrap()?;
///     assert_eq!(record.as_receiver(), "abc");
///     Ok(())
/// }
/// ```
///
/// [`builder`]: struct.RecordBuilder.html
/// [signal]: enum.Signal.html
pub struct Record<R> {
    pub(crate) commands: VecDeque<Meta<R>>,
    receiver: R,
    cursor: usize,
    limit: NonZeroUsize,
    pub(crate) saved: Option<usize>,
    pub(crate) signal: Option<Box<dyn FnMut(Signal) + Send + Sync>>,
}

impl<R> Record<R> {
    /// Returns a new record.
    #[inline]
    pub fn new(receiver: impl Into<R>) -> Record<R> {
        Record {
            commands: VecDeque::new(),
            receiver: receiver.into(),
            cursor: 0,
            limit: MAX_LIMIT,
            saved: Some(0),
            signal: None,
        }
    }

    /// Returns a builder for a record.
    #[inline]
    pub fn builder() -> RecordBuilder<R> {
        RecordBuilder {
            receiver: PhantomData,
            capacity: 0,
            limit: MAX_LIMIT,
            saved: true,
            signal: None,
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
        self.limit.get()
    }

    /// Sets the limit of the record and returns the new limit.
    ///
    /// If this limit is reached it will start popping of commands at the beginning
    /// of the record when new commands are applied. No limit is set by
    /// default which means it may grow indefinitely.
    ///
    /// If `limit < len` the first commands will be removed until `len == limit`.
    /// However, if the current active command is going to be removed, the limit is instead
    /// adjusted to `len - active` so the active command is not removed.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    #[inline]
    pub fn set_limit(&mut self, limit: usize) -> usize {
        self.limit = NonZeroUsize::new(limit).expect("limit can not be `0`");
        if limit < self.len() {
            let old = self.cursor();
            let could_undo = self.can_undo();
            let was_saved = self.is_saved();
            let begin = old.min(self.len() - limit);
            self.commands = self.commands.split_off(begin);
            self.limit = NonZeroUsize::new(self.len()).unwrap();
            self.cursor -= begin;
            // Check if the saved state has been removed.
            self.saved = self.saved.and_then(|saved| saved.checked_sub(begin));
            let new = self.cursor();
            let can_undo = self.can_undo();
            let is_saved = self.is_saved();
            if let Some(ref mut f) = self.signal {
                if old != new {
                    f(Signal::Cursor { old, new });
                }
                if could_undo != can_undo {
                    f(Signal::Undo(can_undo));
                }
                if was_saved != is_saved {
                    f(Signal::Saved(is_saved));
                }
            }
        }
        self.limit.get()
    }

    /// Sets how the signal should be handled when the state changes.
    #[inline]
    pub fn connect<F>(&mut self, f: impl Into<Option<F>>)
    where
        F: FnMut(Signal) + Send + Sync + 'static,
    {
        self.signal = f.into().map(|f| Box::new(f) as _);
    }

    /// Returns `true` if the record can undo.
    #[inline]
    pub fn can_undo(&self) -> bool {
        self.cursor() > 0
    }

    /// Returns `true` if the record can redo.
    #[inline]
    pub fn can_redo(&self) -> bool {
        self.cursor() < self.len()
    }

    /// Marks the receiver as currently being in a saved or unsaved state.
    #[inline]
    pub fn set_saved(&mut self, saved: bool) {
        let was_saved = self.is_saved();
        if saved {
            self.saved = Some(self.cursor());
            if let Some(ref mut f) = self.signal {
                if !was_saved {
                    f(Signal::Saved(true));
                }
            }
        } else {
            self.saved = None;
            if let Some(ref mut f) = self.signal {
                if was_saved {
                    f(Signal::Saved(false));
                }
            }
        }
    }

    /// Returns `true` if the receiver is in a saved state, `false` otherwise.
    #[inline]
    pub fn is_saved(&self) -> bool {
        self.saved.map_or(false, |saved| saved == self.cursor())
    }

    /// Revert the changes done to the receiver since the saved state.
    #[inline]
    pub fn revert(&mut self) -> Option<Result> {
        self.saved.and_then(|saved| self.go_to(saved))
    }

    /// Returns the position of the current command.
    #[inline]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Removes all commands from the record without undoing them.
    #[inline]
    pub fn clear(&mut self) {
        let old = self.cursor();
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        self.commands.clear();
        self.saved = if self.is_saved() { Some(0) } else { None };
        self.cursor = 0;
        if let Some(ref mut f) = self.signal {
            if old != 0 {
                f(Signal::Cursor { old, new: 0 });
            }
            if could_undo {
                f(Signal::Undo(false));
            }
            if could_redo {
                f(Signal::Redo(false));
            }
        }
    }

    /// Pushes the command to the top of the record and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned and the command is removed.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    #[inline]
    pub fn apply(&mut self, command: impl Command<R> + 'static) -> Result
    where
        R: 'static,
    {
        self.__apply(Meta::new(command)).map(|(_, _)| ())
    }

    #[inline]
    pub(crate) fn __apply(
        &mut self,
        mut meta: Meta<R>,
    ) -> std::result::Result<(bool, VecDeque<Meta<R>>), Box<dyn Error + Send + Sync>>
    where
        R: 'static,
    {
        if let Err(error) = meta.apply(&mut self.receiver) {
            return Err(error);
        }
        let cursor = self.cursor();
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        // Pop off all elements after cursor from record.
        let v = self.commands.split_off(cursor);
        debug_assert_eq!(cursor, self.len());
        // Check if the saved state was popped off.
        self.saved = self.saved.filter(|&saved| saved <= cursor);
        // Try to merge commands unless the receiver is in a saved state.
        let merges = match (meta.merge(), self.commands.back().map(|last| last.merge())) {
            (Merge::Always, Some(_)) => !self.is_saved(),
            (Merge::If(id1), Some(Merge::If(id2))) => id1 == id2 && !self.is_saved(),
            _ => false,
        };
        if merges {
            // Merge the command with the one on the top of the stack.
            let merged = Merged::new(self.commands.pop_back().unwrap(), meta);
            self.commands.push_back(Meta::new(merged));
        } else {
            // If commands are not merged push it onto the record.
            if self.limit.get() == self.cursor {
                // If limit is reached, pop off the first command.
                self.commands.pop_front();
                self.saved = self.saved.and_then(|saved| saved.checked_sub(1));
            } else {
                self.cursor += 1;
            }
            self.commands.push_back(meta);
        }
        debug_assert_eq!(self.cursor(), self.len());
        if let Some(ref mut f) = self.signal {
            // We emit this signal even if the commands might have been merged.
            f(Signal::Cursor {
                old: cursor,
                new: self.cursor,
            });
            if could_redo {
                f(Signal::Redo(false));
            }
            if !could_undo {
                f(Signal::Undo(true));
            }
            if was_saved {
                f(Signal::Saved(false));
            }
        }
        Ok((merges, v))
    }

    /// Calls the [`undo`] method for the active command and sets the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned and the command is removed.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result> {
        if !self.can_undo() {
            return None;
        } else if let Err(error) = self.commands[self.cursor - 1].undo(&mut self.receiver) {
            self.commands.remove(self.cursor - 1).unwrap();
            return Some(Err(error));
        }
        let was_saved = self.is_saved();
        let old = self.cursor();
        self.cursor -= 1;
        let len = self.len();
        let is_saved = self.is_saved();
        if let Some(ref mut f) = self.signal {
            f(Signal::Cursor {
                old,
                new: self.cursor,
            });
            if old == len {
                f(Signal::Redo(true));
            }
            if old == 1 {
                f(Signal::Undo(false));
            }
            if was_saved != is_saved {
                f(Signal::Saved(is_saved));
            }
        }
        Some(Ok(()))
    }

    /// Calls the [`redo`] method for the active command and sets the next one as the
    /// new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the error is returned and the command is removed.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result> {
        if !self.can_redo() {
            return None;
        } else if let Err(error) = self.commands[self.cursor].redo(&mut self.receiver) {
            self.commands.remove(self.cursor).unwrap();
            return Some(Err(error));
        }
        let was_saved = self.is_saved();
        let old = self.cursor();
        self.cursor += 1;
        let len = self.len();
        let is_saved = self.is_saved();
        if let Some(ref mut f) = self.signal {
            f(Signal::Cursor {
                old,
                new: self.cursor,
            });
            if old == len - 1 {
                f(Signal::Redo(false));
            }
            if old == 0 {
                f(Signal::Undo(true));
            }
            if was_saved != is_saved {
                f(Signal::Saved(is_saved));
            }
        }
        Some(Ok(()))
    }

    /// Repeatedly calls [`undo`] or [`redo`] until the command at `cursor` is reached.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned and the command is removed.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    pub fn go_to(&mut self, cursor: usize) -> Option<Result> {
        if cursor > self.len() {
            return None;
        }
        let was_saved = self.is_saved();
        let old = self.cursor();
        let len = self.len();
        // Temporarily remove signal so they are not called each iteration.
        let signal = self.signal.take();
        // Decide if we need to undo or redo to reach cursor.
        let redo = cursor > self.cursor();
        let f = if redo { Record::redo } else { Record::undo };
        while self.cursor() != cursor {
            if let Err(error) = f(self).unwrap() {
                self.signal = signal;
                return Some(Err(error));
            }
        }
        // Add signal back.
        self.signal = signal;
        let is_saved = self.is_saved();
        if let Some(ref mut f) = self.signal {
            if old != self.cursor {
                f(Signal::Cursor {
                    old,
                    new: self.cursor,
                });
            }
            if was_saved != is_saved {
                f(Signal::Saved(is_saved));
            }
            if redo {
                if old == len - 1 {
                    f(Signal::Redo(false));
                }
                if old == 0 {
                    f(Signal::Undo(true));
                }
            } else {
                if old == len {
                    f(Signal::Redo(true));
                }
                if old == 1 {
                    f(Signal::Undo(false));
                }
            }
        }
        Some(Ok(()))
    }

    /// Go back or forward in time.
    #[inline]
    #[cfg(feature = "chrono")]
    pub fn time_travel<Tz: TimeZone>(&mut self, to: &DateTime<Tz>) -> Option<Result> {
        let to = Utc.from_utc_datetime(&to.naive_utc());
        let cursor = match self.commands.as_slices() {
            ([], []) => return None,
            (start, []) => match start.binary_search_by(|meta| meta.timestamp.cmp(&to)) {
                Ok(cursor) | Err(cursor) => cursor,
            },
            ([], end) => match end.binary_search_by(|meta| meta.timestamp.cmp(&to)) {
                Ok(cursor) | Err(cursor) => cursor,
            },
            (start, end) => match start.last().unwrap().timestamp.cmp(&to) {
                Ordering::Less => match start.binary_search_by(|meta| meta.timestamp.cmp(&to)) {
                    Ok(cursor) | Err(cursor) => cursor,
                },
                Ordering::Equal => start.len(),
                Ordering::Greater => match end.binary_search_by(|meta| meta.timestamp.cmp(&to)) {
                    Ok(cursor) | Err(cursor) => start.len() + cursor,
                },
            },
        };
        self.go_to(cursor)
    }

    /// Applies each command in the iterator.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned and the command is removed.
    /// The remaining commands in the iterator are discarded.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    #[inline]
    pub fn extend<C: Command<R> + 'static>(
        &mut self,
        commands: impl IntoIterator<Item = C>,
    ) -> Result
    where
        R: 'static,
    {
        for command in commands {
            self.apply(command)?;
        }
        Ok(())
    }

    /// Returns a checkpoint.
    #[inline]
    pub fn checkpoint(&mut self) -> Checkpoint<Record<R>, R> {
        Checkpoint::from(self)
    }

    /// Returns a queue.
    #[inline]
    pub fn queue(&mut self) -> Queue<Record<R>, R> {
        Queue::from(self)
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

    /// Returns a structure for configurable formatting of the record.
    #[inline]
    #[cfg(feature = "display")]
    pub fn display(&self) -> Display<Self> {
        Display::from(self)
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
    fn from(receiver: R) -> Record<R> {
        Record::new(receiver)
    }
}

impl<R> From<History<R>> for Record<R> {
    #[inline]
    fn from(history: History<R>) -> Record<R> {
        history.record
    }
}

impl<R: fmt::Debug> fmt::Debug for Record<R> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
impl<R> fmt::Display for Record<R> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.display() as &dyn fmt::Display).fmt(f)
    }
}

/// Builder for a record.
///
/// # Examples
/// ```
/// # use undo::Record;
/// # fn foo() -> Record<String> {
/// let record = Record::builder()
///     .capacity(100)
///     .limit(100)
///     .saved(false)
///     .default();
/// # record
/// # }
/// ```
pub struct RecordBuilder<R> {
    receiver: PhantomData<R>,
    capacity: usize,
    limit: NonZeroUsize,
    saved: bool,
    signal: Option<Box<dyn FnMut(Signal) + Send + Sync>>,
}

impl<R> RecordBuilder<R> {
    /// Sets the capacity for the record.
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> RecordBuilder<R> {
        self.capacity = capacity;
        self
    }

    /// Sets the `limit` for the record.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    #[inline]
    pub fn limit(mut self, limit: usize) -> RecordBuilder<R> {
        self.limit = NonZeroUsize::new(limit).expect("limit can not be `0`");
        self
    }

    /// Sets if the receiver is initially in a saved state.
    /// By default the receiver is in a saved state.
    #[inline]
    pub fn saved(mut self, saved: bool) -> RecordBuilder<R> {
        self.saved = saved;
        self
    }

    /// Decides how the signal should be handled when the state changes.
    /// By default the record does not handle any signals.
    #[inline]
    pub fn connect<F>(mut self, f: impl Into<Option<F>>) -> RecordBuilder<R>
    where
        F: FnMut(Signal) + Send + Sync + 'static,
    {
        self.signal = f.into().map(|f| Box::new(f) as _);
        self
    }

    /// Builds the record.
    #[inline]
    pub fn build(self, receiver: impl Into<R>) -> Record<R> {
        Record {
            commands: VecDeque::with_capacity(self.capacity),
            receiver: receiver.into(),
            cursor: 0,
            limit: self.limit,
            saved: if self.saved { Some(0) } else { None },
            signal: self.signal,
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

impl<R: fmt::Debug> fmt::Debug for RecordBuilder<R> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("RecordBuilder")
            .field("receiver", &self.receiver)
            .field("capacity", &self.capacity)
            .field("limit", &self.limit)
            .field("saved", &self.saved)
            .finish()
    }
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
    fn go_to() {
        let mut record = Record::default();
        record.apply(Add('a')).unwrap();
        record.apply(Add('b')).unwrap();
        record.apply(Add('c')).unwrap();
        record.apply(Add('d')).unwrap();
        record.apply(Add('e')).unwrap();

        record.go_to(0).unwrap().unwrap();
        assert_eq!(record.cursor(), 0);
        assert_eq!(record.as_receiver(), "");
        record.go_to(5).unwrap().unwrap();
        assert_eq!(record.cursor(), 5);
        assert_eq!(record.as_receiver(), "abcde");
        record.go_to(1).unwrap().unwrap();
        assert_eq!(record.cursor(), 1);
        assert_eq!(record.as_receiver(), "a");
        record.go_to(4).unwrap().unwrap();
        assert_eq!(record.cursor(), 4);
        assert_eq!(record.as_receiver(), "abcd");
        record.go_to(2).unwrap().unwrap();
        assert_eq!(record.cursor(), 2);
        assert_eq!(record.as_receiver(), "ab");
        record.go_to(3).unwrap().unwrap();
        assert_eq!(record.cursor(), 3);
        assert_eq!(record.as_receiver(), "abc");
        assert!(record.go_to(6).is_none());
        assert_eq!(record.cursor(), 3);
    }
}
