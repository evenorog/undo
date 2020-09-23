use crate::{join, Checkpoint, Command, Display, Entry, Merge, Queue, Result, Signal, Slot};
use chrono::{DateTime, TimeZone, Utc};
use std::{cmp::Ordering, collections::VecDeque, error::Error, num::NonZeroUsize};

/// A record of commands.
///
/// The record can roll the targets state backwards and forwards by using
/// the undo and redo methods. In addition, the record can notify the user
/// about changes to the stack or the target through [signal](enum.Signal.html).
/// The user can give the record a function that is called each time the state
/// changes by using the [`builder`](struct.RecordBuilder.html).
///
/// # Examples
/// ```
/// # use undo::*;
/// # #[derive(Debug)]
/// # struct Add(char);
/// # impl Command<String> for Add {
/// #     fn apply(&mut self, s: &mut String) -> undo::Result {
/// #         s.push(self.0);
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self, s: &mut String) -> undo::Result {
/// #         self.0 = s.pop().ok_or("s is empty")?;
/// #         Ok(())
/// #     }
/// # }
/// # fn main() -> undo::Result {
/// let mut record = Record::default();
/// record.apply(Add('a'))?;
/// record.apply(Add('b'))?;
/// record.apply(Add('c'))?;
/// assert_eq!(record.target(), "abc");
/// record.undo()?;
/// record.undo()?;
/// record.undo()?;
/// assert_eq!(record.target(), "");
/// record.redo()?;
/// record.redo()?;
/// record.redo()?;
/// assert_eq!(record.target(), "abc");
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Record<T: 'static> {
    pub(crate) entries: VecDeque<Entry<T>>,
    target: T,
    current: usize,
    limit: NonZeroUsize,
    pub(crate) saved: Option<usize>,
    pub(crate) slot: Slot,
}

impl<T> Record<T> {
    /// Returns a new record.
    pub fn new(target: T) -> Record<T> {
        Builder::new().build(target)
    }

    /// Reserves capacity for at least `additional` more commands.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    pub fn reserve(&mut self, additional: usize) {
        self.entries.reserve(additional);
    }

    /// Returns the capacity of the record.
    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }

    /// Shrinks the capacity of the record as much as possible.
    pub fn shrink_to_fit(&mut self) {
        self.entries.shrink_to_fit();
    }

    /// Returns the number of commands in the record.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the record is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the limit of the record.
    pub fn limit(&self) -> usize {
        self.limit.get()
    }

    /// Sets how the signal should be handled when the state changes.
    ///
    /// The previous slot is returned if it exists.
    pub fn connect(
        &mut self,
        slot: impl FnMut(Signal) + 'static,
    ) -> Option<impl FnMut(Signal) + 'static> {
        self.slot.f.replace(Box::from(slot))
    }

    /// Removes and returns the slot if it exists.
    pub fn disconnect(&mut self) -> Option<impl FnMut(Signal) + 'static> {
        self.slot.f.take()
    }

    /// Returns `true` if the record can undo.
    pub fn can_undo(&self) -> bool {
        self.current() > 0
    }

    /// Returns `true` if the record can redo.
    pub fn can_redo(&self) -> bool {
        self.current() < self.len()
    }

    /// Returns `true` if the target is in a saved state, `false` otherwise.
    pub fn is_saved(&self) -> bool {
        self.saved.map_or(false, |saved| saved == self.current())
    }

    /// Marks the target as currently being in a saved or unsaved state.
    pub fn set_saved(&mut self, saved: bool) {
        let was_saved = self.is_saved();
        if saved {
            self.saved = Some(self.current());
            self.slot.emit_if(!was_saved, Signal::Saved(true));
        } else {
            self.saved = None;
            self.slot.emit_if(was_saved, Signal::Saved(false));
        }
    }

    /// Revert the changes done to the target since the saved state.
    pub fn revert(&mut self) -> Option<Result> {
        self.saved.and_then(|saved| self.go_to(saved))
    }

    /// Returns the position of the current command.
    pub fn current(&self) -> usize {
        self.current
    }

    /// Removes all commands from the record without undoing them.
    pub fn clear(&mut self) {
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        self.entries.clear();
        self.saved = if self.is_saved() { Some(0) } else { None };
        self.current = 0;
        self.slot.emit_if(could_undo, Signal::Undo(false));
        self.slot.emit_if(could_redo, Signal::Redo(false));
    }

    /// Pushes the command to the top of the record and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    pub fn apply(&mut self, command: impl Command<T>) -> Result {
        self.__apply(command).map(|_| ())
    }

    #[allow(clippy::type_complexity)]
    pub(crate) fn __apply(
        &mut self,
        mut command: impl Command<T>,
    ) -> std::result::Result<(bool, VecDeque<Entry<T>>), Box<dyn Error>> {
        command.apply(&mut self.target)?;
        let current = self.current();
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        // Pop off all elements after current from record.
        let tail = self.entries.split_off(current);
        // Check if the saved state was popped off.
        self.saved = self.saved.filter(|&saved| saved <= current);
        // Try to merge commands unless the target is in a saved state.
        let merges = match (command.merge(), self.entries.back().map(Command::merge)) {
            (Merge::Yes, Some(_)) => !self.is_saved(),
            (Merge::If(id1), Some(Merge::If(id2))) => id1 == id2 && !self.is_saved(),
            _ => false,
        };
        if merges {
            // Merge the command with the one on the top of the stack.
            let merge = command.merge();
            let command = join(self.entries.pop_back().unwrap(), command).with_merge(merge);
            self.entries.push_back(Entry::new(Box::new(command)));
        } else {
            // If commands are not merged push it onto the record.
            if self.limit() == self.current() {
                // If limit is reached, pop off the first command.
                self.entries.pop_front();
                self.saved = self.saved.and_then(|saved| saved.checked_sub(1));
            } else {
                self.current += 1;
            }
            self.entries.push_back(Entry::new(Box::new(command)));
        }
        self.slot.emit_if(could_redo, Signal::Redo(false));
        self.slot.emit_if(!could_undo, Signal::Undo(true));
        self.slot.emit_if(was_saved, Signal::Saved(false));
        Ok((merges, tail))
    }

    /// Calls the [`undo`] method for the active command and sets
    /// the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    pub fn undo(&mut self) -> Result {
        if !self.can_undo() {
            return Ok(());
        }
        let was_saved = self.is_saved();
        let old = self.current();
        self.entries[self.current - 1].undo(&mut self.target)?;
        self.current -= 1;
        let len = self.len();
        let is_saved = self.is_saved();
        self.slot.emit_if(old == len, Signal::Redo(true));
        self.slot.emit_if(old == 1, Signal::Undo(false));
        self.slot
            .emit_if(was_saved != is_saved, Signal::Saved(is_saved));
        Ok(())
    }

    /// Calls the [`redo`] method for the active command and sets the next one as the
    /// new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the error is returned.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    pub fn redo(&mut self) -> Result {
        if !self.can_redo() {
            return Ok(());
        }
        let was_saved = self.is_saved();
        let old = self.current();
        self.entries[self.current].redo(&mut self.target)?;
        self.current += 1;
        let len = self.len();
        let is_saved = self.is_saved();
        self.slot.emit_if(old == len - 1, Signal::Redo(false));
        self.slot.emit_if(old == 0, Signal::Undo(true));
        self.slot
            .emit_if(was_saved != is_saved, Signal::Saved(is_saved));
        Ok(())
    }

    /// Repeatedly calls [`undo`] or [`redo`] until the command at `current` is reached.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    /// [`redo`]: trait.Command.html#method.redo
    pub fn go_to(&mut self, current: usize) -> Option<Result> {
        if current > self.len() {
            return None;
        }
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        // Temporarily remove slot so they are not called each iteration.
        let f = self.slot.f.take();
        // Decide if we need to undo or redo to reach current.
        let apply = if current > self.current() {
            Record::redo
        } else {
            Record::undo
        };
        while self.current() != current {
            if let Err(error) = apply(self) {
                self.slot.f = f;
                return Some(Err(error));
            }
        }
        self.slot.f = f;
        let can_undo = self.can_undo();
        let can_redo = self.can_redo();
        let is_saved = self.is_saved();
        self.slot
            .emit_if(could_undo != can_undo, Signal::Undo(can_undo));
        self.slot
            .emit_if(could_redo != can_redo, Signal::Redo(can_redo));
        self.slot
            .emit_if(was_saved != is_saved, Signal::Saved(is_saved));
        Some(Ok(()))
    }

    /// Go back or forward in the record to the command that was made closest to the datetime provided.
    pub fn time_travel(&mut self, to: &DateTime<impl TimeZone>) -> Option<Result> {
        let to = to.with_timezone(&Utc);
        let current = match self.entries.as_slices() {
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

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<T> {
        Queue::from(self)
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<T> {
        Checkpoint::from(self)
    }

    /// Returns the string of the command which will be undone in the next call to [`undo`].
    ///
    /// [`undo`]: struct.Record.html#method.undo
    pub fn undo_text(&self) -> Option<String> {
        if self.can_undo() {
            self.entries.get(self.current - 1).map(Command::text)
        } else {
            None
        }
    }

    /// Returns the string of the command which will be redone in the next call to [`redo`].
    ///
    /// [`redo`]: struct.Record.html#method.redo
    pub fn redo_text(&self) -> Option<String> {
        self.entries.get(self.current).map(Command::text)
    }

    /// Returns a structure for configurable formatting of the record.
    ///
    /// Requires the `display` feature to be enabled.
    pub fn display(&self) -> Display<T> {
        Display::from(self)
    }

    /// Returns a reference to the `target`.
    pub fn target(&self) -> &T {
        &self.target
    }

    /// Returns a mutable reference to the `target`.
    ///
    /// This method should **only** be used when doing changes that should not be able to be undone.
    pub fn target_mut(&mut self) -> &mut T {
        &mut self.target
    }

    /// Consumes the record, returning the `target`.
    pub fn into_target(self) -> T {
        self.target
    }
}

impl<T: Default> Default for Record<T> {
    fn default() -> Record<T> {
        Record::new(T::default())
    }
}

impl<T> From<T> for Record<T> {
    fn from(target: T) -> Record<T> {
        Record::new(target)
    }
}

/// A builder for a record.
///
/// # Examples
/// ```
/// # use undo::{Record, Builder};
/// # fn foo() -> Record<String> {
/// Builder::new()
///     .capacity(100)
///     .limit(100)
///     .saved(false)
///     .default()
/// # }
/// ```
#[derive(Debug)]
pub struct Builder {
    capacity: usize,
    limit: NonZeroUsize,
    saved: bool,
}

impl Builder {
    /// Returns a builder for a record.
    pub fn new() -> Builder {
        Builder {
            capacity: 0,
            limit: NonZeroUsize::new(usize::max_value()).unwrap(),
            saved: true,
        }
    }

    /// Sets the capacity for the record.
    pub fn capacity(&mut self, capacity: usize) -> &mut Builder {
        self.capacity = capacity;
        self
    }

    /// Sets the `limit` for the record.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    pub fn limit(&mut self, limit: usize) -> &mut Builder {
        self.limit = NonZeroUsize::new(limit).expect("limit can not be `0`");
        self
    }

    /// Sets if the target is initially in a saved state.
    /// By default the target is in a saved state.
    pub fn saved(&mut self, saved: bool) -> &mut Builder {
        self.saved = saved;
        self
    }

    /// Builds the record.
    pub fn build<T>(&self, target: T) -> Record<T> {
        Record {
            entries: VecDeque::with_capacity(self.capacity),
            target,
            current: 0,
            limit: self.limit,
            saved: if self.saved { Some(0) } else { None },
            slot: Slot::default(),
        }
    }

    /// Builds the record with the slot.
    pub fn build_with<T>(&self, target: T, slot: impl FnMut(Signal) + 'static) -> Record<T> {
        Record {
            slot: Slot {
                f: Some(Box::new(slot)),
            },
            ..self.build(target)
        }
    }

    /// Creates the record with a default `target`.
    pub fn default<T: Default>(&self) -> Record<T> {
        self.build(T::default())
    }

    /// Creates the record with a default `target` and with the slot.
    pub fn default_with<T: Default>(&self, slot: impl FnMut(Signal) + 'static) -> Record<T> {
        self.build_with(T::default(), slot)
    }
}

impl Default for Builder {
    fn default() -> Self {
        Builder::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[derive(Debug)]
    struct Add(char);

    impl Command<String> for Add {
        fn apply(&mut self, s: &mut String) -> Result {
            s.push(self.0);
            Ok(())
        }

        fn undo(&mut self, s: &mut String) -> Result {
            self.0 = s.pop().ok_or("s is empty")?;
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
}
