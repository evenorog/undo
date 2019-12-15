use crate::chain::Join;
use crate::{Checkpoint, Command, Entry, History, Merge, Queue, Result, Signal, Timeline};
use std::{collections::VecDeque, error::Error, num::NonZeroUsize};
#[cfg(feature = "display")]
use {crate::Display, std::fmt};
#[cfg(feature = "chrono")]
use {
    chrono::{DateTime, TimeZone, Utc},
    std::cmp::Ordering,
};

#[allow(unsafe_code)]
const MAX_LIMIT: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(usize::max_value()) };

/// A record of commands.
///
/// The record can roll the targets state backwards and forwards by using
/// the undo and redo methods. In addition, the record can notify the user
/// about changes to the stack or the target through [signal]. The user
/// can give the record a function that is called each time the state changes
/// by using the [`builder`].
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
/// record.apply(Add('a'))?;
/// record.apply(Add('b'))?;
/// record.apply(Add('c'))?;
/// assert_eq!(record.target(), "abc");
/// record.undo().unwrap()?;
/// record.undo().unwrap()?;
/// record.undo().unwrap()?;
/// assert_eq!(record.target(), "");
/// record.redo().unwrap()?;
/// record.redo().unwrap()?;
/// record.redo().unwrap()?;
/// assert_eq!(record.target(), "abc");
/// # Ok(())
/// # }
/// ```
///
/// [`builder`]: struct.RecordBuilder.html
/// [signal]: enum.Signal.html
pub struct Record<T: 'static> {
    pub(crate) commands: VecDeque<Entry<T>>,
    target: T,
    current: usize,
    limit: NonZeroUsize,
    pub(crate) saved: Option<usize>,
    pub(crate) slot: Option<Box<dyn FnMut(Signal)>>,
}

impl<T> Record<T> {
    /// Returns a new record.
    #[inline]
    pub fn new(target: T) -> Record<T> {
        RecordBuilder::new().build(target)
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

    /// Shrinks the capacity of the record as much as possible.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.commands.shrink_to_fit();
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

    /// Sets how the signal should be handled when the state changes.
    ///
    /// The previous slot is returned if it exists.
    #[inline]
    pub fn connect(
        &mut self,
        slot: impl FnMut(Signal) + 'static,
    ) -> Option<impl FnMut(Signal) + 'static> {
        self.slot.replace(Box::from(slot))
    }

    /// Removes and returns the slot.
    #[inline]
    pub fn disconnect(&mut self) -> Option<impl FnMut(Signal) + 'static> {
        self.slot.take()
    }

    /// Returns `true` if the record can undo.
    #[inline]
    pub fn can_undo(&self) -> bool {
        self.current() > 0
    }

    /// Returns `true` if the record can redo.
    #[inline]
    pub fn can_redo(&self) -> bool {
        self.current() < self.len()
    }

    /// Returns `true` if the target is in a saved state, `false` otherwise.
    #[inline]
    pub fn is_saved(&self) -> bool {
        self.saved.map_or(false, |saved| saved == self.current())
    }

    /// Marks the target as currently being in a saved or unsaved state.
    #[inline]
    pub fn set_saved(&mut self, saved: bool) {
        let was_saved = self.is_saved();
        if saved {
            self.saved = Some(self.current());
            if let Some(ref mut slot) = self.slot {
                if !was_saved {
                    slot(Signal::Saved(true));
                }
            }
        } else {
            self.saved = None;
            if let Some(ref mut slot) = self.slot {
                if was_saved {
                    slot(Signal::Saved(false));
                }
            }
        }
    }

    /// Revert the changes done to the target since the saved state.
    #[inline]
    pub fn revert(&mut self) -> Option<Result> {
        self.saved.and_then(|saved| self.go_to(saved))
    }

    /// Returns the position of the current command.
    #[inline]
    pub fn current(&self) -> usize {
        self.current
    }

    /// Removes all commands from the record without undoing them.
    #[inline]
    pub fn clear(&mut self) {
        let old = self.current();
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        self.commands.clear();
        self.saved = if self.is_saved() { Some(0) } else { None };
        self.current = 0;
        if let Some(ref mut slot) = self.slot {
            if old != 0 {
                slot(Signal::Current { old, new: 0 });
            }
            if could_undo {
                slot(Signal::Undo(false));
            }
            if could_redo {
                slot(Signal::Redo(false));
            }
        }
    }

    /// Pushes the command to the top of the record and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    #[inline]
    pub fn apply(&mut self, command: impl Command<T>) -> Result {
        self.__apply(Entry::new(command)).map(|_| ())
    }

    #[inline]
    pub(crate) fn __apply(
        &mut self,
        mut entry: Entry<T>,
    ) -> std::result::Result<(bool, VecDeque<Entry<T>>), Box<dyn Error>> {
        entry.apply(&mut self.target)?;
        let current = self.current();
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        // Pop off all elements after current from record.
        let v = self.commands.split_off(current);
        debug_assert_eq!(current, self.len());
        // Check if the saved state was popped off.
        self.saved = self.saved.filter(|&saved| saved <= current);
        // Try to merge commands unless the target is in a saved state.
        let merges = match (entry.merge(), self.commands.back().map(Command::merge)) {
            (Merge::Yes, Some(_)) => !self.is_saved(),
            (Merge::If(id1), Some(Merge::If(id2))) => id1 == id2 && !self.is_saved(),
            _ => false,
        };
        if merges {
            // Merge the command with the one on the top of the stack.
            let command = Join(self.commands.pop_back().unwrap(), entry);
            self.commands.push_back(Entry::new(command));
        } else {
            // If commands are not merged push it onto the record.
            if self.limit() == self.current() {
                // If limit is reached, pop off the first command.
                self.commands.pop_front();
                self.saved = self.saved.and_then(|saved| saved.checked_sub(1));
            } else {
                self.current += 1;
            }
            self.commands.push_back(entry);
        }
        debug_assert_eq!(self.current(), self.len());
        if let Some(ref mut slot) = self.slot {
            // We emit this signal even if the commands might have been merged.
            slot(Signal::Current {
                old: current,
                new: self.current,
            });
            if could_redo {
                slot(Signal::Redo(false));
            }
            if !could_undo {
                slot(Signal::Undo(true));
            }
            if was_saved {
                slot(Signal::Saved(false));
            }
        }
        Ok((merges, v))
    }

    /// Calls the [`undo`] method for the active command and sets
    /// the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result> {
        if !self.can_undo() {
            return None;
        }
        let was_saved = self.is_saved();
        let old = self.current();
        if let Err(error) = self.commands[self.current - 1].undo(&mut self.target) {
            return Some(Err(error));
        }
        self.current -= 1;
        let len = self.len();
        let is_saved = self.is_saved();
        if let Some(ref mut slot) = self.slot {
            slot(Signal::Current {
                old,
                new: self.current,
            });
            if old == len {
                slot(Signal::Redo(true));
            }
            if old == 1 {
                slot(Signal::Undo(false));
            }
            if was_saved != is_saved {
                slot(Signal::Saved(is_saved));
            }
        }
        Some(Ok(()))
    }

    /// Calls the [`redo`] method for the active command and sets the next one as the
    /// new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the error is returned.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result> {
        if !self.can_redo() {
            return None;
        }
        let was_saved = self.is_saved();
        let old = self.current();
        if let Err(error) = self.commands[self.current].redo(&mut self.target) {
            return Some(Err(error));
        }
        self.current += 1;
        let len = self.len();
        let is_saved = self.is_saved();
        if let Some(ref mut slot) = self.slot {
            slot(Signal::Current {
                old,
                new: self.current,
            });
            if old == len - 1 {
                slot(Signal::Redo(false));
            }
            if old == 0 {
                slot(Signal::Undo(true));
            }
            if was_saved != is_saved {
                slot(Signal::Saved(is_saved));
            }
        }
        Some(Ok(()))
    }

    /// Repeatedly calls [`undo`] or [`redo`] until the command at `current` is reached.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    pub fn go_to(&mut self, current: usize) -> Option<Result> {
        if current > self.len() {
            return None;
        }
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        let old = self.current();
        // Temporarily remove slot so they are not called each iteration.
        let slot = self.slot.take();
        while self.current() != current {
            // Decide if we need to undo or redo to reach current.
            let f = if current > self.current() {
                Record::redo
            } else {
                Record::undo
            };
            if let Err(error) = f(self).unwrap() {
                self.slot = slot;
                return Some(Err(error));
            }
        }
        // Add slot back.
        self.slot = slot;
        let can_undo = self.can_undo();
        let can_redo = self.can_redo();
        let is_saved = self.is_saved();
        if let Some(ref mut slot) = self.slot {
            if old != self.current {
                slot(Signal::Current {
                    old,
                    new: self.current,
                });
            }
            if could_undo != can_undo {
                slot(Signal::Undo(can_undo));
            }
            if could_redo != can_redo {
                slot(Signal::Redo(can_redo));
            }
            if was_saved != is_saved {
                slot(Signal::Saved(is_saved));
            }
        }
        Some(Ok(()))
    }

    /// Go back or forward in the record to the command that was made closest to the datetime provided.
    #[inline]
    #[cfg(feature = "chrono")]
    pub fn time_travel(&mut self, to: &DateTime<impl TimeZone>) -> Option<Result> {
        let to = to.with_timezone(&Utc);
        let current = match self.commands.as_slices() {
            ([], []) => return None,
            (start, []) => match start.binary_search_by(|entry| entry.timestamp.cmp(&to)) {
                Ok(current) | Err(current) => current,
            },
            ([], end) => match end.binary_search_by(|entry| entry.timestamp.cmp(&to)) {
                Ok(current) | Err(current) => current,
            },
            (start, end) => match start.last().unwrap().timestamp.cmp(&to) {
                Ordering::Less => match start.binary_search_by(|entry| entry.timestamp.cmp(&to)) {
                    Ok(current) | Err(current) => current,
                },
                Ordering::Equal => start.len(),
                Ordering::Greater => match end.binary_search_by(|entry| entry.timestamp.cmp(&to)) {
                    Ok(current) | Err(current) => start.len() + current,
                },
            },
        };
        self.go_to(current)
    }

    /// Applies each command in the iterator.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned
    /// and the remaining commands in the iterator are discarded.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    #[inline]
    pub fn extend<C: Command<T>>(&mut self, commands: impl IntoIterator<Item = C>) -> Result {
        for command in commands {
            self.apply(command)?;
        }
        Ok(())
    }

    /// Returns a queue.
    #[inline]
    pub fn queue(&mut self) -> Queue<Record<T>> {
        Queue::from(self)
    }

    /// Returns a checkpoint.
    #[inline]
    pub fn checkpoint(&mut self) -> Checkpoint<Record<T>> {
        Checkpoint::from(self)
    }

    /// Returns the string of the command which will be undone in the next call to [`undo`].
    ///
    /// Requires the `display` feature to be enabled.
    ///
    /// [`undo`]: struct.Record.html#method.undo
    #[inline]
    #[cfg(feature = "display")]
    pub fn to_undo_string(&self) -> Option<String> {
        if self.can_undo() {
            Some(self.commands[self.current - 1].to_string())
        } else {
            None
        }
    }

    /// Returns the string of the command which will be redone in the next call to [`redo`].
    ///
    /// Requires the `display` feature to be enabled.
    ///
    /// [`redo`]: struct.Record.html#method.redo
    #[inline]
    #[cfg(feature = "display")]
    pub fn to_redo_string(&self) -> Option<String> {
        if self.can_redo() {
            Some(self.commands[self.current].to_string())
        } else {
            None
        }
    }

    /// Returns a structure for configurable formatting of the record.
    ///
    /// Requires the `display` feature to be enabled.
    #[inline]
    #[cfg(feature = "display")]
    pub fn display(&self) -> Display<Self> {
        Display::from(self)
    }

    /// Returns a reference to the `target`.
    #[inline]
    pub fn target(&self) -> &T {
        &self.target
    }

    /// Returns a mutable reference to the `target`.
    ///
    /// This method should **only** be used when doing changes that should not be able to be undone.
    #[inline]
    pub fn target_mut(&mut self) -> &mut T {
        &mut self.target
    }

    /// Consumes the record, returning the `target`.
    #[inline]
    pub fn into_target(self) -> T {
        self.target
    }
}

impl<T> Timeline for Record<T> {
    type Target = T;

    #[inline]
    fn apply(&mut self, command: impl Command<T>) -> Result {
        self.apply(command)
    }

    #[inline]
    fn undo(&mut self) -> Option<Result> {
        self.undo()
    }

    #[inline]
    fn redo(&mut self) -> Option<Result> {
        self.redo()
    }
}

impl<T: Default> Default for Record<T> {
    #[inline]
    fn default() -> Record<T> {
        Record::new(T::default())
    }
}

impl<T> From<T> for Record<T> {
    #[inline]
    fn from(target: T) -> Record<T> {
        Record::new(target)
    }
}

impl<T> From<History<T>> for Record<T> {
    #[inline]
    fn from(history: History<T>) -> Record<T> {
        history.record
    }
}

#[cfg(feature = "display")]
impl<T: fmt::Debug> fmt::Debug for Record<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Record")
            .field("commands", &self.commands)
            .field("target", &self.target)
            .field("current", &self.current)
            .field("limit", &self.limit)
            .field("saved", &self.saved)
            .finish()
    }
}

#[cfg(feature = "display")]
impl<T> fmt::Display for Record<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.display() as &dyn fmt::Display).fmt(f)
    }
}

/// Builder for a record.
///
/// # Examples
/// ```
/// # use undo::{Record, RecordBuilder};
/// # fn foo() -> Record<String> {
/// RecordBuilder::new()
///     .capacity(100)
///     .limit(100)
///     .saved(false)
///     .default()
/// # }
/// ```
#[derive(Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct RecordBuilder {
    capacity: usize,
    limit: NonZeroUsize,
    saved: bool,
}

impl RecordBuilder {
    /// Returns a builder for a record.
    #[inline]
    pub fn new() -> RecordBuilder {
        RecordBuilder {
            capacity: 0,
            limit: MAX_LIMIT,
            saved: true,
        }
    }

    /// Sets the capacity for the record.
    #[inline]
    pub fn capacity(&mut self, capacity: usize) -> &mut RecordBuilder {
        self.capacity = capacity;
        self
    }

    /// Sets the `limit` for the record.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    #[inline]
    pub fn limit(&mut self, limit: usize) -> &mut RecordBuilder {
        self.limit = NonZeroUsize::new(limit).expect("limit can not be `0`");
        self
    }

    /// Sets if the target is initially in a saved state.
    /// By default the target is in a saved state.
    #[inline]
    pub fn saved(&mut self, saved: bool) -> &mut RecordBuilder {
        self.saved = saved;
        self
    }

    /// Builds the record.
    #[inline]
    pub fn build<T>(&self, target: T) -> Record<T> {
        Record {
            commands: VecDeque::with_capacity(self.capacity),
            target,
            current: 0,
            limit: self.limit,
            saved: if self.saved { Some(0) } else { None },
            slot: None,
        }
    }

    /// Builds the record with the slot.
    #[inline]
    pub fn build_with<T>(&self, target: T, slot: impl FnMut(Signal) + 'static) -> Record<T> {
        Record {
            slot: Some(Box::new(slot)),
            ..self.build(target)
        }
    }

    /// Creates the record with a default `target`.
    #[inline]
    pub fn default<T: Default>(&self) -> Record<T> {
        self.build(T::default())
    }

    /// Creates the record with a default `target` and with the slot.
    #[inline]
    pub fn default_with<T: Default>(&self, slot: impl FnMut(Signal) + 'static) -> Record<T> {
        self.build_with(T::default(), slot)
    }
}

impl Default for RecordBuilder {
    #[inline]
    fn default() -> Self {
        RecordBuilder::new()
    }
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
    fn go_to() {
        let mut record = Record::default();
        record.apply(Add('a')).unwrap();
        record.apply(Add('b')).unwrap();
        record.apply(Add('c')).unwrap();
        record.apply(Add('d')).unwrap();
        record.apply(Add('e')).unwrap();

        record.go_to(0).unwrap().unwrap();
        assert_eq!(record.current(), 0);
        assert_eq!(record.target(), "");
        record.go_to(5).unwrap().unwrap();
        assert_eq!(record.current(), 5);
        assert_eq!(record.target(), "abcde");
        record.go_to(1).unwrap().unwrap();
        assert_eq!(record.current(), 1);
        assert_eq!(record.target(), "a");
        record.go_to(4).unwrap().unwrap();
        assert_eq!(record.current(), 4);
        assert_eq!(record.target(), "abcd");
        record.go_to(2).unwrap().unwrap();
        assert_eq!(record.current(), 2);
        assert_eq!(record.target(), "ab");
        record.go_to(3).unwrap().unwrap();
        assert_eq!(record.current(), 3);
        assert_eq!(record.target(), "abc");
        assert!(record.go_to(6).is_none());
        assert_eq!(record.current(), 3);
    }

    #[test]
    #[cfg(feature = "chrono")]
    fn time_travel() {
        let mut record = Record::default();
        record.apply(Add('a')).unwrap();
        let a = chrono::Utc::now();
        record.apply(Add('b')).unwrap();
        record.apply(Add('c')).unwrap();
        record.time_travel(&a).unwrap().unwrap();
        assert_eq!(record.target(), "a");
        record.time_travel(&chrono::Utc::now()).unwrap().unwrap();
        assert_eq!(record.target(), "abc");
    }
}
