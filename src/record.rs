//! A record of actions.

use crate::{Action, At, Entry, Format, History, Merged, Result, Signal, Slot};
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
    chrono::{DateTime, Utc},
    core::cmp::Ordering,
    core::convert::identity,
};

/// A record of actions.
///
/// The record can roll the targets state backwards and forwards by using
/// the undo and redo methods. In addition, the record can notify the user
/// about changes to the stack or the target through [signal](enum.Signal.html).
/// The user can give the record a function that is called each time the state
/// changes by using the [`builder`](struct.RecordBuilder.html).
///
/// # Examples
/// ```
/// # use undo::{Action, Record};
/// # struct Add(char);
/// # impl Action for Add {
/// #     type Target = String;
/// #     type Output = ();
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
/// # fn main() {
/// let mut target = String::new();
/// let mut record = Record::new();
/// record.apply(&mut target, Add('a')).unwrap();
/// record.apply(&mut target, Add('b')).unwrap();
/// record.apply(&mut target, Add('c')).unwrap();
/// assert_eq!(target, "abc");
/// record.undo(&mut target).unwrap().unwrap();
/// record.undo(&mut target).unwrap().unwrap();
/// record.undo(&mut target).unwrap().unwrap();
/// assert_eq!(target, "");
/// record.redo(&mut target).unwrap().unwrap();
/// record.redo(&mut target).unwrap().unwrap();
/// record.redo(&mut target).unwrap().unwrap();
/// assert_eq!(target, "abc");
/// # }
/// ```
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(bound(serialize = "A: Serialize", deserialize = "A: Deserialize<'de>"))
)]
#[derive(Clone)]
pub struct Record<A, F = Box<dyn FnMut(Signal)>> {
    pub(crate) entries: VecDeque<Entry<A>>,
    current: usize,
    limit: NonZeroUsize,
    pub(crate) saved: Option<usize>,
    pub(crate) slot: Slot<F>,
}

impl<A> Record<A> {
    /// Returns a new record.
    pub fn new() -> Record<A> {
        Builder::new().build()
    }
}

impl<A, F> Record<A, F> {
    /// Reserves capacity for at least `additional` more actions.
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

    /// Returns the number of actions in the record.
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

    /// Returns the position of the current action.
    pub fn current(&self) -> usize {
        self.current
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<A, F> {
        Queue::from(self)
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, F> {
        Checkpoint::from(self)
    }

    /// Returns a structure for configurable formatting of the record.
    pub fn display(&self) -> Display<A, F> {
        Display::from(self)
    }
}

impl<A: Action, F: FnMut(Signal)> Record<A, F> {
    /// Pushes the action on top of the record and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned.
    ///
    /// [`apply`]: trait.Action.html#tymethod.apply
    pub fn apply(&mut self, target: &mut A::Target, action: A) -> Result<A> {
        self.__apply(target, action).map(|(output, _, _)| output)
    }

    #[allow(clippy::type_complexity)]
    pub(crate) fn __apply(
        &mut self,
        target: &mut A::Target,
        mut action: A,
    ) -> core::result::Result<(A::Output, bool, VecDeque<Entry<A>>), A::Error> {
        let output = action.apply(target)?;
        let current = self.current();
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        // Pop off all elements after len from record.
        let tail = self.entries.split_off(current);
        // Check if the saved state was popped off.
        self.saved = self.saved.filter(|&saved| saved <= current);
        // Try to merge actions unless the target is in a saved state.
        let merged = match self.entries.back_mut() {
            Some(last) if !was_saved => last.action.merge(&mut action),
            _ => Merged::No,
        };
        let merged_or_annulled = match merged {
            Merged::Yes => true,
            Merged::Annul => {
                self.entries.pop_back();
                self.current -= 1;
                true
            }
            // If actions are not merged or annulled push it onto the record.
            Merged::No => {
                // If limit is reached, pop off the first action.
                if self.limit() == self.current() {
                    self.entries.pop_front();
                    self.saved = self.saved.and_then(|saved| saved.checked_sub(1));
                } else {
                    self.current += 1;
                }
                self.entries.push_back(Entry::from(action));
                false
            }
        };
        self.slot.emit_if(could_redo, Signal::Redo(false));
        self.slot.emit_if(!could_undo, Signal::Undo(true));
        self.slot.emit_if(was_saved, Signal::Saved(false));
        Ok((output, merged_or_annulled, tail))
    }

    /// Calls the [`undo`] method for the active action and sets
    /// the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned.
    ///
    /// [`undo`]: ../trait.Action.html#tymethod.undo
    pub fn undo(&mut self, target: &mut A::Target) -> Option<Result<A>> {
        self.can_undo().then(|| {
            let was_saved = self.is_saved();
            let old = self.current();
            let output = self.entries[self.current - 1].undo(target)?;
            self.current -= 1;
            let is_saved = self.is_saved();
            self.slot.emit_if(old == self.len(), Signal::Redo(true));
            self.slot.emit_if(old == 1, Signal::Undo(false));
            self.slot
                .emit_if(was_saved != is_saved, Signal::Saved(is_saved));
            Ok(output)
        })
    }

    /// Calls the [`redo`] method for the active action and sets
    /// the next one as the new active one.
    ///
    /// # Errors
    /// If an error occur when applying [`redo`] the error is returned.
    ///
    /// [`redo`]: trait.Action.html#method.redo
    pub fn redo(&mut self, target: &mut A::Target) -> Option<Result<A>> {
        self.can_redo().then(|| {
            let was_saved = self.is_saved();
            let old = self.current();
            let output = self.entries[self.current].redo(target)?;
            self.current += 1;
            let is_saved = self.is_saved();
            self.slot
                .emit_if(old == self.len() - 1, Signal::Redo(false));
            self.slot.emit_if(old == 0, Signal::Undo(true));
            self.slot
                .emit_if(was_saved != is_saved, Signal::Saved(is_saved));
            Ok(output)
        })
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

    /// Removes all actions from the record without undoing them.
    pub fn clear(&mut self) {
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        self.entries.clear();
        self.saved = self.is_saved().then(|| 0);
        self.current = 0;
        self.slot.emit_if(could_undo, Signal::Undo(false));
        self.slot.emit_if(could_redo, Signal::Redo(false));
    }
}

impl<A: Action<Output = ()>, F: FnMut(Signal)> Record<A, F> {
    /// Revert the changes done to the target since the saved state.
    pub fn revert(&mut self, target: &mut A::Target) -> Option<Result<A>> {
        self.saved.and_then(|saved| self.go_to(target, saved))
    }

    /// Repeatedly calls [`undo`] or [`redo`] until the action at `current` is reached.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned.
    ///
    /// [`undo`]: trait.Action.html#tymethod.undo
    /// [`redo`]: trait.Action.html#method.redo
    pub fn go_to(&mut self, target: &mut A::Target, current: usize) -> Option<Result<A>> {
        if current > self.len() {
            return None;
        }
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        // Temporarily remove slot so they are not called each iteration.
        let slot = self.disconnect();
        // Decide if we need to undo or redo to reach current.
        let f = if current > self.current() {
            Record::redo
        } else {
            Record::undo
        };
        while self.current() != current {
            if let Some(Err(err)) = f(self, target) {
                self.slot.f = slot;
                return Some(Err(err));
            }
        }
        // Add slot back.
        self.slot.f = slot;
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

    /// Go back or forward in the record to the action that was made closest to the datetime provided.
    #[cfg(feature = "chrono")]
    pub fn time_travel(&mut self, target: &mut A::Target, to: &DateTime<Utc>) -> Option<Result<A>> {
        let current = match self.entries.as_slices() {
            ([], []) => return None,
            (head, []) => head
                .binary_search_by(|e| e.timestamp.cmp(to))
                .unwrap_or_else(identity),
            ([], tail) => tail
                .binary_search_by(|e| e.timestamp.cmp(to))
                .unwrap_or_else(identity),
            (head, tail) => match head.last().unwrap().timestamp.cmp(to) {
                Ordering::Less => head
                    .binary_search_by(|e| e.timestamp.cmp(to))
                    .unwrap_or_else(identity),
                Ordering::Equal => head.len(),
                Ordering::Greater => {
                    head.len()
                        + tail
                            .binary_search_by(|e| e.timestamp.cmp(to))
                            .unwrap_or_else(identity)
                }
            },
        };
        self.go_to(target, current)
    }
}

impl<A: ToString, F> Record<A, F> {
    /// Returns the string of the action which will be undone
    /// in the next call to [`undo`](struct.Record.html#method.undo).
    pub fn undo_text(&self) -> Option<String> {
        self.current.checked_sub(1).and_then(|i| self.text(i))
    }

    /// Returns the string of the action which will be redone
    /// in the next call to [`redo`](struct.Record.html#method.redo).
    pub fn redo_text(&self) -> Option<String> {
        self.text(self.current)
    }

    fn text(&self, i: usize) -> Option<String> {
        self.entries.get(i).map(|e| e.action.to_string())
    }
}

impl<A> Default for Record<A> {
    fn default() -> Record<A> {
        Record::new()
    }
}

impl<A, F> From<History<A, F>> for Record<A, F> {
    fn from(history: History<A, F>) -> Record<A, F> {
        history.record
    }
}

impl<A: fmt::Debug, F> fmt::Debug for Record<A, F> {
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
/// # use undo::{Action, record::Builder, Record};
/// # struct Add(char);
/// # impl Action for Add {
/// #     type Target = String;
/// #     type Output = ();
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
#[derive(Debug)]
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
            limit: NonZeroUsize::new(usize::MAX).unwrap(),
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
    pub fn build<A>(self) -> Record<A, F> {
        Record {
            entries: VecDeque::with_capacity(self.capacity),
            current: 0,
            limit: self.limit,
            saved: self.saved.then(|| 0),
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
enum QueueAction<A> {
    Apply(A),
    Undo,
    Redo,
}

/// Wraps a record and gives it batch queue functionality.
///
/// # Examples
/// ```
/// # use undo::{Action, Record};
/// # struct Add(char);
/// # impl Action for Add {
/// #     type Target = String;
/// #     type Output = ();
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
/// # fn main() {
/// let mut string = String::new();
/// let mut record = Record::new();
/// let mut queue = record.queue();
/// queue.apply(Add('a'));
/// queue.apply(Add('b'));
/// queue.apply(Add('c'));
/// assert_eq!(string, "");
/// queue.commit(&mut string).unwrap().unwrap();
/// assert_eq!(string, "abc");
/// # }
/// ```
#[derive(Debug)]
pub struct Queue<'a, A, F> {
    record: &'a mut Record<A, F>,
    actions: Vec<QueueAction<A>>,
}

impl<A: Action<Output = ()>, F: FnMut(Signal)> Queue<'_, A, F> {
    /// Queues an `apply` action.
    pub fn apply(&mut self, action: A) {
        self.actions.push(QueueAction::Apply(action));
    }

    /// Queues an `undo` action.
    pub fn undo(&mut self) {
        self.actions.push(QueueAction::Undo);
    }

    /// Queues a `redo` action.
    pub fn redo(&mut self) {
        self.actions.push(QueueAction::Redo);
    }

    /// Applies the queued actions.
    ///
    /// # Errors
    /// If an error occurs, it stops applying the actions and returns the error.
    pub fn commit(self, target: &mut A::Target) -> Option<Result<A>> {
        for action in self.actions {
            let r = match action {
                QueueAction::Apply(action) => Some(self.record.apply(target, action)),
                QueueAction::Undo => self.record.undo(target),
                QueueAction::Redo => self.record.redo(target),
            };
            match r {
                Some(Ok(_)) => (),
                o @ Some(Err(_)) | o @ None => return o,
            }
        }
        Some(Ok(()))
    }

    /// Cancels the queued actions.
    pub fn cancel(self) {}

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<A, F> {
        self.record.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, F> {
        self.record.checkpoint()
    }
}

impl<'a, A, F> From<&'a mut Record<A, F>> for Queue<'a, A, F> {
    fn from(record: &'a mut Record<A, F>) -> Self {
        Queue {
            record,
            actions: Vec::new(),
        }
    }
}

#[derive(Debug)]
enum CheckpointAction<A> {
    Apply(Option<usize>, VecDeque<Entry<A>>),
    Undo,
    Redo,
}

/// Wraps a record and gives it checkpoint functionality.
#[derive(Debug)]
pub struct Checkpoint<'a, A, F> {
    record: &'a mut Record<A, F>,
    actions: Vec<CheckpointAction<A>>,
}

impl<A: Action<Output = ()>, F: FnMut(Signal)> Checkpoint<'_, A, F> {
    /// Calls the `apply` method.
    pub fn apply(&mut self, target: &mut A::Target, action: A) -> Result<A> {
        let saved = self.record.saved;
        let (_, _, tail) = self.record.__apply(target, action)?;
        self.actions.push(CheckpointAction::Apply(saved, tail));
        Ok(())
    }

    /// Calls the `undo` method.
    pub fn undo(&mut self, target: &mut A::Target) -> Option<Result<A>> {
        match self.record.undo(target) {
            o @ Some(Ok(())) => {
                self.actions.push(CheckpointAction::Undo);
                o
            }
            o => o,
        }
    }

    /// Calls the `redo` method.
    pub fn redo(&mut self, target: &mut A::Target) -> Option<Result<A>> {
        match self.record.redo(target) {
            o @ Some(Ok(())) => {
                self.actions.push(CheckpointAction::Redo);
                o
            }
            o => o,
        }
    }

    /// Commits the changes and consumes the checkpoint.
    pub fn commit(self) {}

    /// Cancels the changes and consumes the checkpoint.
    ///
    /// # Errors
    /// If an error occur when canceling the changes, the error is returned
    /// and the remaining actions are not canceled.
    pub fn cancel(self, target: &mut A::Target) -> Option<Result<A>> {
        for action in self.actions.into_iter().rev() {
            match action {
                CheckpointAction::Apply(saved, mut entries) => match self.record.undo(target) {
                    Some(Ok(())) => {
                        self.record.entries.pop_back();
                        self.record.entries.append(&mut entries);
                        self.record.saved = saved;
                    }
                    o => return o,
                },
                CheckpointAction::Undo => match self.record.redo(target) {
                    Some(Ok(())) => (),
                    o => return o,
                },
                CheckpointAction::Redo => match self.record.undo(target) {
                    Some(Ok(())) => (),
                    o => return o,
                },
            };
        }
        Some(Ok(()))
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<A, F> {
        self.record.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, F> {
        self.record.checkpoint()
    }
}

impl<'a, A, F> From<&'a mut Record<A, F>> for Checkpoint<'a, A, F> {
    fn from(record: &'a mut Record<A, F>) -> Self {
        Checkpoint {
            record,
            actions: Vec::new(),
        }
    }
}

/// Configurable display formatting for the record.
pub struct Display<'a, A, F> {
    record: &'a Record<A, F>,
    format: Format,
}

impl<A, F> Display<'_, A, F> {
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

    /// Show the position of the action (on by default).
    pub fn position(&mut self, on: bool) -> &mut Self {
        self.format.position = on;
        self
    }

    /// Show the saved action (on by default).
    pub fn saved(&mut self, on: bool) -> &mut Self {
        self.format.saved = on;
        self
    }
}

impl<A: fmt::Display, F> Display<'_, A, F> {
    fn fmt_list(&self, f: &mut fmt::Formatter, at: At, entry: Option<&Entry<A>>) -> fmt::Result {
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

impl<'a, A, F> From<&'a Record<A, F>> for Display<'a, A, F> {
    fn from(record: &'a Record<A, F>) -> Self {
        Display {
            record,
            format: Format::default(),
        }
    }
}

impl<A: fmt::Display, F> fmt::Display for Display<'_, A, F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, entry) in self.record.entries.iter().enumerate().rev() {
            let at = At::new(0, i + 1);
            self.fmt_list(f, at, Some(entry))?;
        }
        self.fmt_list(f, At::ROOT, None)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use alloc::boxed::Box;
    use alloc::string::String;

    enum Edit {
        Add(Add),
        Del(Del),
    }

    impl Action for Edit {
        type Target = String;
        type Output = ();
        type Error = &'static str;

        fn apply(&mut self, s: &mut String) -> Result<Add> {
            match self {
                Edit::Add(add) => add.apply(s),
                Edit::Del(del) => del.apply(s),
            }
        }

        fn undo(&mut self, s: &mut String) -> Result<Add> {
            match self {
                Edit::Add(add) => add.undo(s),
                Edit::Del(del) => del.undo(s),
            }
        }

        fn merge(&mut self, edit: &mut Self) -> Merged
        where
            Self: Sized,
        {
            match (self, edit) {
                (Edit::Add(_), Edit::Del(_)) => Merged::Annul,
                (Edit::Del(Del(Some(a))), Edit::Add(Add(b))) if a == b => Merged::Annul,
                _ => Merged::No,
            }
        }
    }

    struct Add(char);

    impl Action for Add {
        type Target = String;
        type Output = ();
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

    #[derive(Default)]
    struct Del(Option<char>);

    impl Action for Del {
        type Target = String;
        type Output = ();
        type Error = &'static str;

        fn apply(&mut self, s: &mut String) -> Result<Add> {
            self.0 = s.pop();
            Ok(())
        }

        fn undo(&mut self, s: &mut String) -> Result<Add> {
            let ch = self.0.ok_or("s is empty")?;
            s.push(ch);
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
        q3.commit(&mut target).unwrap().unwrap();
        assert_eq!(target, "abc");
        q2.commit(&mut target).unwrap().unwrap();
        assert_eq!(target, "");
        q1.commit(&mut target).unwrap().unwrap();
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
        cp3.cancel(&mut target).unwrap().unwrap();
        assert_eq!(target, "abcdef");
        cp2.cancel(&mut target).unwrap().unwrap();
        assert_eq!(target, "abc");
        cp1.cancel(&mut target).unwrap().unwrap();
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
        record.undo(&mut target).unwrap().unwrap();
        record.undo(&mut target).unwrap().unwrap();
        record.undo(&mut target).unwrap().unwrap();
        let mut cp = record.checkpoint();
        cp.apply(&mut target, Add('d')).unwrap();
        cp.apply(&mut target, Add('e')).unwrap();
        cp.apply(&mut target, Add('f')).unwrap();
        assert_eq!(target, "def");
        cp.cancel(&mut target).unwrap().unwrap();
        assert_eq!(target, "");
        record.redo(&mut target).unwrap().unwrap();
        record.redo(&mut target).unwrap().unwrap();
        record.redo(&mut target).unwrap().unwrap();
        assert!(record.is_saved());
        assert_eq!(target, "abc");
    }

    #[test]
    fn dyn_trait() {
        let mut target = String::new();
        let action: Box<dyn Action<Output = (), Error = &'static str, Target = String>> =
            Box::new(Add('a'));
        let mut record: Record<
            Box<dyn Action<Output = (), Error = &'static str, Target = String>>,
        > = Record::default();
        record.apply(&mut target, action).unwrap();
    }

    #[test]
    fn annul() {
        let mut target = String::new();
        let mut record = Record::new();
        record.apply(&mut target, Edit::Add(Add('a'))).unwrap();
        record
            .apply(&mut target, Edit::Del(Del::default()))
            .unwrap();
        record.apply(&mut target, Edit::Add(Add('b'))).unwrap();
    }
}
