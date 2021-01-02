//! A record of commands.

use crate::{format::Format, At, Command, Entry, History, Merge, Result, Signal, Slot};
use alloc::{
    boxed::Box,
    collections::VecDeque,
    string::{String, ToString},
    vec::Vec,
};
use core::{
    fmt::{self, Write},
    num::NonZeroUsize,
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "chrono")]
use {
    chrono::{DateTime, TimeZone, Utc},
    core::cmp::Ordering,
};

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
/// # use undo::{Command, Record};
/// # struct Add(char);
/// # impl Command for Add {
/// #     type Target = String;
/// #     type Error = &'static str;
/// #     fn apply(&mut self, s: &mut String) -> undo::Result<Add> {
/// #         s.push(self.0);
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self, s: &mut String) -> undo::Result<Add> {
/// #         self.0 = s.pop().ok_or("s is empty")?;
/// #         Ok(())
/// #     }
/// # }
/// # fn main() -> undo::Result<Add> {
/// let mut target = String::new();
/// let mut record = Record::new();
/// record.apply(&mut target, Add('a'))?;
/// record.apply(&mut target, Add('b'))?;
/// record.apply(&mut target, Add('c'))?;
/// assert_eq!(target, "abc");
/// record.undo(&mut target)?;
/// record.undo(&mut target)?;
/// record.undo(&mut target)?;
/// assert_eq!(target, "");
/// record.redo(&mut target)?;
/// record.redo(&mut target)?;
/// record.redo(&mut target)?;
/// assert_eq!(target, "abc");
/// # Ok(())
/// # }
/// ```
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(bound(serialize = "C: Serialize", deserialize = "C: Deserialize<'de>"))
)]
#[derive(Clone)]
pub struct Record<C, F = Box<dyn FnMut(Signal)>> {
    pub(crate) entries: VecDeque<Entry<C>>,
    current: usize,
    limit: NonZeroUsize,
    pub(crate) saved: Option<usize>,
    pub(crate) slot: Slot<F>,
}

impl<C> Record<C> {
    /// Returns a new record.
    pub fn new() -> Record<C> {
        Builder::new().build()
    }
}

impl<C, F> Record<C, F> {
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
    pub fn connect(&mut self, slot: F) -> Option<F> {
        self.slot.f.replace(slot)
    }

    /// Removes and returns the slot if it exists.
    pub fn disconnect(&mut self) -> Option<F> {
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

    /// Returns the position of the current command.
    pub fn current(&self) -> usize {
        self.current
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<C, F> {
        Queue::from(self)
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<C, F> {
        Checkpoint::from(self)
    }

    /// Returns a structure for configurable formatting of the record.
    pub fn display(&self) -> Display<C, F> {
        Display::from(self)
    }
}

impl<C: Command, F: FnMut(Signal)> Record<C, F> {
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
    pub fn revert(&mut self, target: &mut C::Target) -> Option<Result<C>> {
        self.saved.and_then(|saved| self.go_to(target, saved))
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

    /// Pushes the command on top of the record and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    pub fn apply(&mut self, target: &mut C::Target, command: C) -> Result<C> {
        self.__apply(target, command).map(|_| ())
    }

    pub(crate) fn __apply(
        &mut self,
        target: &mut C::Target,
        mut command: C,
    ) -> core::result::Result<(bool, VecDeque<Entry<C>>), C::Error> {
        command.apply(target)?;
        let current = self.current();
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        // Pop off all elements after len from record.
        let tail = self.entries.split_off(current);
        // Check if the saved state was popped off.
        self.saved = self.saved.filter(|&saved| saved <= current);
        // Try to merge commands unless the target is in a saved state.
        let merged = match self.entries.back_mut() {
            Some(ref mut last) if !was_saved => last.command.merge(command),
            _ => Merge::No(command),
        };
        let merged_or_annulled = match merged {
            Merge::Yes => true,
            Merge::Annul => {
                self.entries.pop_back();
                true
            }
            // If commands are not merged or annulled push it onto the record.
            Merge::No(command) => {
                // If limit is reached, pop off the first command.
                if self.limit() == self.current() {
                    self.entries.pop_front();
                    self.saved = self.saved.and_then(|saved| saved.checked_sub(1));
                } else {
                    self.current += 1;
                }
                self.entries.push_back(Entry::from(command));
                false
            }
        };
        self.slot.emit_if(could_redo, Signal::Redo(false));
        self.slot.emit_if(!could_undo, Signal::Undo(true));
        self.slot.emit_if(was_saved, Signal::Saved(false));
        Ok((merged_or_annulled, tail))
    }

    /// Calls the [`undo`] method for the active command and sets
    /// the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned.
    ///
    /// [`undo`]: ../trait.Command.html#tymethod.undo
    pub fn undo(&mut self, target: &mut C::Target) -> Result<C> {
        if !self.can_undo() {
            return Ok(());
        }
        let was_saved = self.is_saved();
        let old = self.current();
        self.entries[self.current - 1].undo(target)?;
        self.current -= 1;
        let len = self.len();
        let is_saved = self.is_saved();
        self.slot.emit_if(old == len, Signal::Redo(true));
        self.slot.emit_if(old == 1, Signal::Undo(false));
        self.slot
            .emit_if(was_saved != is_saved, Signal::Saved(is_saved));
        Ok(())
    }

    /// Calls the [`redo`] method for the active command and sets
    /// the next one as the new active one.
    ///
    /// # Errors
    /// If an error occur when applying [`redo`] the error is returned.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    pub fn redo(&mut self, target: &mut C::Target) -> Result<C> {
        if !self.can_redo() {
            return Ok(());
        }
        let was_saved = self.is_saved();
        let old = self.current();
        self.entries[self.current].redo(target)?;
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
    pub fn go_to(&mut self, target: &mut C::Target, current: usize) -> Option<Result<C>> {
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
            if let Err(err) = apply(self, target) {
                self.slot.f = f;
                return Some(Err(err));
            }
        }
        // Add slot back.
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
    #[cfg(feature = "chrono")]
    pub fn time_travel(
        &mut self,
        target: &mut C::Target,
        to: &DateTime<impl TimeZone>,
    ) -> Option<Result<C>> {
        let to = to.with_timezone(&Utc);
        let current = match self.entries.as_slices() {
            ([], []) => return None,
            (head, []) => match head.binary_search_by(|e| e.timestamp.cmp(&to)) {
                Ok(current) | Err(current) => current,
            },
            ([], tail) => match tail.binary_search_by(|e| e.timestamp.cmp(&to)) {
                Ok(current) | Err(current) => current,
            },
            (head, tail) => match head.last().unwrap().timestamp.cmp(&to) {
                Ordering::Less => match head.binary_search_by(|e| e.timestamp.cmp(&to)) {
                    Ok(current) | Err(current) => current,
                },
                Ordering::Equal => head.len(),
                Ordering::Greater => match tail.binary_search_by(|e| e.timestamp.cmp(&to)) {
                    Ok(current) | Err(current) => head.len() + current,
                },
            },
        };
        self.go_to(target, current)
    }
}

impl<C: ToString, F> Record<C, F> {
    /// Returns the string of the command which will be undone in the next call to [`undo`].
    ///
    /// [`undo`]: struct.Record.html#method.undo
    pub fn undo_text(&self) -> Option<String> {
        self.current.checked_sub(1).and_then(|i| self.text(i))
    }

    /// Returns the string of the command which will be redone in the next call to [`redo`].
    ///
    /// [`redo`]: struct.Record.html#method.redo
    pub fn redo_text(&self) -> Option<String> {
        self.text(self.current)
    }

    fn text(&self, i: usize) -> Option<String> {
        self.entries.get(i).map(|e| e.command.to_string())
    }
}

impl<C: Command> Default for Record<C>
where
    C::Target: Default,
{
    fn default() -> Record<C> {
        Record::new()
    }
}

impl<C, F> From<History<C, F>> for Record<C, F> {
    fn from(history: History<C, F>) -> Record<C, F> {
        history.record
    }
}

impl<C, F> fmt::Debug for Record<C, F>
where
    C: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Record")
            .field("entries", &self.entries)
            .field("current", &self.current)
            .field("limit", &self.limit)
            .field("saved", &self.saved)
            .field("slot", &self.slot)
            .finish()
    }
}

/// Builder for a Record.
///
/// # Examples
/// ```
/// # use undo::{Command, record::Builder, Record};
/// # struct Add(char);
/// # impl Command for Add {
/// #     type Target = String;
/// #     type Error = &'static str;
/// #     fn apply(&mut self, s: &mut String) -> undo::Result<Add> {
/// #         s.push(self.0);
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self, s: &mut String) -> undo::Result<Add> {
/// #         self.0 = s.pop().ok_or("s is empty")?;
/// #         Ok(())
/// #     }
/// # }
/// let _ = Builder::new()
///     .limit(100)
///     .capacity(100)
///     .connect(|s| { dbg!(s); })
///     .build::<Add>();
/// ```
pub struct Builder<F = Box<dyn FnMut(Signal)>> {
    capacity: usize,
    limit: NonZeroUsize,
    saved: bool,
    slot: Slot<F>,
}

impl<F> Builder<F> {
    /// Returns a builder for a record.
    pub fn new() -> Builder<F> {
        Builder {
            capacity: 0,
            limit: NonZeroUsize::new(usize::max_value()).unwrap(),
            saved: true,
            slot: Slot::default(),
        }
    }

    /// Sets the capacity for the record.
    pub fn capacity(mut self, capacity: usize) -> Builder<F> {
        self.capacity = capacity;
        self
    }

    /// Sets the `limit` of the record.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    pub fn limit(mut self, limit: usize) -> Builder<F> {
        self.limit = NonZeroUsize::new(limit).expect("limit can not be `0`");
        self
    }

    /// Sets if the target is initially in a saved state.
    /// By default the target is in a saved state.
    pub fn saved(mut self, saved: bool) -> Builder<F> {
        self.saved = saved;
        self
    }

    /// Builds the record.
    pub fn build<C>(self) -> Record<C, F> {
        Record {
            entries: VecDeque::with_capacity(self.capacity),
            current: 0,
            limit: self.limit,
            saved: if self.saved { Some(0) } else { None },
            slot: self.slot,
        }
    }
}

impl<F: FnMut(Signal)> Builder<F> {
    /// Connects the slot.
    pub fn connect(mut self, f: F) -> Builder<F> {
        self.slot = Slot::from(f);
        self
    }
}

impl Default for Builder {
    fn default() -> Self {
        Builder::new()
    }
}

#[derive(Debug)]
enum QueueCommand<C> {
    Apply(C),
    Undo,
    Redo,
}

/// Wraps a record and gives it batch queue functionality.
///
/// # Examples
/// ```
/// # use undo::{Command, Record};
/// # struct Add(char);
/// # impl Command for Add {
/// #     type Target = String;
/// #     type Error = &'static str;
/// #     fn apply(&mut self, s: &mut String) -> undo::Result<Add> {
/// #         s.push(self.0);
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self, s: &mut String) -> undo::Result<Add> {
/// #         self.0 = s.pop().ok_or("s is empty")?;
/// #         Ok(())
/// #     }
/// # }
/// # fn main() -> undo::Result<Add> {
/// let mut string = String::new();
/// let mut record = Record::new();
/// let mut queue = record.queue();
/// queue.apply(Add('a'));
/// queue.apply(Add('b'));
/// queue.apply(Add('c'));
/// assert_eq!(string, "");
/// queue.commit(&mut string)?;
/// assert_eq!(string, "abc");
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Queue<'a, C, F> {
    record: &'a mut Record<C, F>,
    commands: Vec<QueueCommand<C>>,
}

impl<C: Command, F: FnMut(Signal)> Queue<'_, C, F> {
    /// Queues an `apply` action.
    pub fn apply(&mut self, command: C) {
        self.commands.push(QueueCommand::Apply(command));
    }

    /// Queues an `undo` action.
    pub fn undo(&mut self) {
        self.commands.push(QueueCommand::Undo);
    }

    /// Queues a `redo` action.
    pub fn redo(&mut self) {
        self.commands.push(QueueCommand::Redo);
    }

    /// Applies the queued commands.
    ///
    /// # Errors
    /// If an error occurs, it stops applying the commands and returns the error.
    pub fn commit(self, target: &mut C::Target) -> Result<C> {
        for command in self.commands {
            match command {
                QueueCommand::Apply(command) => self.record.apply(target, command)?,
                QueueCommand::Undo => self.record.undo(target)?,
                QueueCommand::Redo => self.record.redo(target)?,
            }
        }
        Ok(())
    }

    /// Cancels the queued actions.
    pub fn cancel(self) {}

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<C, F> {
        self.record.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<C, F> {
        self.record.checkpoint()
    }
}

impl<'a, C, F> From<&'a mut Record<C, F>> for Queue<'a, C, F> {
    fn from(record: &'a mut Record<C, F>) -> Self {
        Queue {
            record,
            commands: Vec::new(),
        }
    }
}

#[derive(Debug)]
enum CheckpointCommand<C> {
    Apply(Option<usize>, VecDeque<Entry<C>>),
    Undo,
    Redo,
}

/// Wraps a record and gives it checkpoint functionality.
#[derive(Debug)]
pub struct Checkpoint<'a, C, F> {
    record: &'a mut Record<C, F>,
    commands: Vec<CheckpointCommand<C>>,
}

impl<C: Command, F: FnMut(Signal)> Checkpoint<'_, C, F> {
    /// Calls the `apply` method.
    pub fn apply(&mut self, target: &mut C::Target, command: C) -> Result<C> {
        let saved = self.record.saved;
        let (_, tail) = self.record.__apply(target, command)?;
        self.commands.push(CheckpointCommand::Apply(saved, tail));
        Ok(())
    }

    /// Calls the `undo` method.
    pub fn undo(&mut self, target: &mut C::Target) -> Result<C> {
        if self.record.can_undo() {
            self.record.undo(target)?;
            self.commands.push(CheckpointCommand::Undo);
        }
        Ok(())
    }

    /// Calls the `redo` method.
    pub fn redo(&mut self, target: &mut C::Target) -> Result<C> {
        if self.record.can_redo() {
            self.record.redo(target)?;
            self.commands.push(CheckpointCommand::Redo);
        }
        Ok(())
    }

    /// Commits the changes and consumes the checkpoint.
    pub fn commit(self) {}

    /// Cancels the changes and consumes the checkpoint.
    ///
    /// # Errors
    /// If an error occur when canceling the changes, the error is returned
    /// and the remaining commands are not canceled.
    pub fn cancel(self, target: &mut C::Target) -> Result<C> {
        for command in self.commands.into_iter().rev() {
            match command {
                CheckpointCommand::Apply(saved, mut entries) => {
                    self.record.undo(target)?;
                    self.record.entries.pop_back();
                    self.record.entries.append(&mut entries);
                    self.record.saved = saved;
                }
                CheckpointCommand::Undo => self.record.redo(target)?,
                CheckpointCommand::Redo => self.record.undo(target)?,
            }
        }
        Ok(())
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<C, F> {
        self.record.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<C, F> {
        self.record.checkpoint()
    }
}

impl<'a, C, F> From<&'a mut Record<C, F>> for Checkpoint<'a, C, F> {
    fn from(record: &'a mut Record<C, F>) -> Self {
        Checkpoint {
            record,
            commands: Vec::new(),
        }
    }
}

/// Configurable display formatting for record.
#[derive(Clone, Debug)]
pub struct Display<'a, C, F> {
    record: &'a Record<C, F>,
    format: crate::format::Format,
}

impl<C, F> Display<'_, C, F> {
    /// Show colored output (on by default).
    ///
    /// Requires the `colored` feature to be enabled.
    #[cfg(feature = "colored")]
    pub fn colored(&mut self, on: bool) -> &mut Self {
        self.format.colored = on;
        self
    }

    /// Show the current position in the output (on by default).
    pub fn current(&mut self, on: bool) -> &mut Self {
        self.format.current = on;
        self
    }

    /// Show detailed output (on by default).
    pub fn detailed(&mut self, on: bool) -> &mut Self {
        self.format.detailed = on;
        self
    }

    /// Show the position of the command (on by default).
    pub fn position(&mut self, on: bool) -> &mut Self {
        self.format.position = on;
        self
    }

    /// Show the saved command (on by default).
    pub fn saved(&mut self, on: bool) -> &mut Self {
        self.format.saved = on;
        self
    }
}

impl<C: fmt::Display, F> Display<'_, C, F> {
    fn fmt_list(&self, f: &mut fmt::Formatter, at: At, entry: Option<&Entry<C>>) -> fmt::Result {
        self.format.position(f, at, false)?;

        #[cfg(feature = "chrono")]
        if let Some(entry) = entry {
            if self.format.detailed {
                self.format.timestamp(f, &entry.timestamp)?;
            }
        }

        self.format.labels(
            f,
            at,
            At::new(0, self.record.current()),
            self.record.saved.map(|saved| At::new(0, saved)),
        )?;
        if let Some(entry) = entry {
            if self.format.detailed {
                writeln!(f)?;
                self.format.message(f, entry, None)?;
            } else {
                f.write_char(' ')?;
                self.format.message(f, entry, None)?;
                writeln!(f)?;
            }
        }
        Ok(())
    }
}

impl<'a, C, F> From<&'a Record<C, F>> for Display<'a, C, F> {
    fn from(record: &'a Record<C, F>) -> Self {
        Display {
            record,
            format: Format::default(),
        }
    }
}

impl<C: fmt::Display, F> fmt::Display for Display<'_, C, F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, entry) in self.record.entries.iter().enumerate().rev() {
            let at = At::new(0, i + 1);
            self.fmt_list(f, at, Some(entry))?;
        }
        self.fmt_list(f, At::new(0, 0), None)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use alloc::string::String;

    struct Add(char);

    impl Command for Add {
        type Target = String;
        type Error = &'static str;

        fn apply(&mut self, s: &mut String) -> Result<Add> {
            s.push(self.0);
            Ok(())
        }

        fn undo(&mut self, s: &mut String) -> Result<Add> {
            self.0 = s.pop().ok_or("s is empty")?;
            Ok(())
        }
    }

    #[test]
    fn go_to() {
        let mut target = String::new();
        let mut record = Record::new();
        record.apply(&mut target, Add('a')).unwrap();
        record.apply(&mut target, Add('b')).unwrap();
        record.apply(&mut target, Add('c')).unwrap();
        record.apply(&mut target, Add('d')).unwrap();
        record.apply(&mut target, Add('e')).unwrap();

        record.go_to(&mut target, 0).unwrap().unwrap();
        assert_eq!(record.current(), 0);
        assert_eq!(target, "");
        record.go_to(&mut target, 5).unwrap().unwrap();
        assert_eq!(record.current(), 5);
        assert_eq!(target, "abcde");
        record.go_to(&mut target, 1).unwrap().unwrap();
        assert_eq!(record.current(), 1);
        assert_eq!(target, "a");
        record.go_to(&mut target, 4).unwrap().unwrap();
        assert_eq!(record.current(), 4);
        assert_eq!(target, "abcd");
        record.go_to(&mut target, 2).unwrap().unwrap();
        assert_eq!(record.current(), 2);
        assert_eq!(target, "ab");
        record.go_to(&mut target, 3).unwrap().unwrap();
        assert_eq!(record.current(), 3);
        assert_eq!(target, "abc");
        assert!(record.go_to(&mut target, 6).is_none());
        assert_eq!(record.current(), 3);
    }

    #[test]
    fn queue_commit() {
        let mut target = String::new();
        let mut record = Record::new();
        let mut q1 = record.queue();
        q1.redo();
        q1.redo();
        q1.redo();
        let mut q2 = q1.queue();
        q2.undo();
        q2.undo();
        q2.undo();
        let mut q3 = q2.queue();
        q3.apply(Add('a'));
        q3.apply(Add('b'));
        q3.apply(Add('c'));
        assert_eq!(target, "");
        q3.commit(&mut target).unwrap();
        assert_eq!(target, "abc");
        q2.commit(&mut target).unwrap();
        assert_eq!(target, "");
        q1.commit(&mut target).unwrap();
        assert_eq!(target, "abc");
    }

    #[test]
    fn checkpoint_commit() {
        let mut target = String::new();
        let mut record = Record::new();
        let mut cp1 = record.checkpoint();
        cp1.apply(&mut target, Add('a')).unwrap();
        cp1.apply(&mut target, Add('b')).unwrap();
        cp1.apply(&mut target, Add('c')).unwrap();
        assert_eq!(target, "abc");
        let mut cp2 = cp1.checkpoint();
        cp2.apply(&mut target, Add('d')).unwrap();
        cp2.apply(&mut target, Add('e')).unwrap();
        cp2.apply(&mut target, Add('f')).unwrap();
        assert_eq!(target, "abcdef");
        let mut cp3 = cp2.checkpoint();
        cp3.apply(&mut target, Add('g')).unwrap();
        cp3.apply(&mut target, Add('h')).unwrap();
        cp3.apply(&mut target, Add('i')).unwrap();
        assert_eq!(target, "abcdefghi");
        cp3.commit();
        cp2.commit();
        cp1.commit();
        assert_eq!(target, "abcdefghi");
    }

    #[test]
    fn checkpoint_cancel() {
        let mut target = String::new();
        let mut record = Record::new();
        let mut cp1 = record.checkpoint();
        cp1.apply(&mut target, Add('a')).unwrap();
        cp1.apply(&mut target, Add('b')).unwrap();
        cp1.apply(&mut target, Add('c')).unwrap();
        let mut cp2 = cp1.checkpoint();
        cp2.apply(&mut target, Add('d')).unwrap();
        cp2.apply(&mut target, Add('e')).unwrap();
        cp2.apply(&mut target, Add('f')).unwrap();
        let mut cp3 = cp2.checkpoint();
        cp3.apply(&mut target, Add('g')).unwrap();
        cp3.apply(&mut target, Add('h')).unwrap();
        cp3.apply(&mut target, Add('i')).unwrap();
        assert_eq!(target, "abcdefghi");
        cp3.cancel(&mut target).unwrap();
        assert_eq!(target, "abcdef");
        cp2.cancel(&mut target).unwrap();
        assert_eq!(target, "abc");
        cp1.cancel(&mut target).unwrap();
        assert_eq!(target, "");
    }

    #[test]
    fn checkpoint_saved() {
        let mut target = String::new();
        let mut record = Record::new();
        record.apply(&mut target, Add('a')).unwrap();
        record.apply(&mut target, Add('b')).unwrap();
        record.apply(&mut target, Add('c')).unwrap();
        record.set_saved(true);
        record.undo(&mut target).unwrap();
        record.undo(&mut target).unwrap();
        record.undo(&mut target).unwrap();
        let mut cp = record.checkpoint();
        cp.apply(&mut target, Add('d')).unwrap();
        cp.apply(&mut target, Add('e')).unwrap();
        cp.apply(&mut target, Add('f')).unwrap();
        assert_eq!(target, "def");
        cp.cancel(&mut target).unwrap();
        assert_eq!(target, "");
        record.redo(&mut target).unwrap();
        record.redo(&mut target).unwrap();
        record.redo(&mut target).unwrap();
        assert!(record.is_saved());
        assert_eq!(target, "abc");
    }
}
