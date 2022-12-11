//! A record of actions.

use crate::entry::Entries;
use crate::slot::{NoOp, Slot, SW};
use crate::{Action, At, Entry, Format, History, Result, Timeline};
use alloc::{
    collections::VecDeque,
    string::{String, ToString},
    vec::Vec,
};
use core::convert::Infallible;
use core::ops::{Index, IndexMut};
use core::{
    fmt::{self, Write},
    num::NonZeroUsize,
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "time")]
use {core::convert::identity, time::OffsetDateTime};

/// A linear record of actions.
///
/// The record can roll the targets state backwards and forwards by using
/// the undo and redo methods. In addition, the record can notify the user
/// about changes to the stack or the target through [signal](enum.Signal.html).
/// The user can give the record a function that is called each time the state
/// changes by using the [`builder`](struct.RecordBuilder.html).
///
/// # Examples
/// ```
/// # use undo::Record;
/// # include!("../push.rs");
/// # fn main() {
/// let mut target = String::new();
/// let mut record = Record::new();
/// record.apply(&mut target, Push('a')).unwrap();
/// record.apply(&mut target, Push('b')).unwrap();
/// record.apply(&mut target, Push('c')).unwrap();
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct Record<A, S = NoOp> {
    pub(crate) record: Timeline<LimitDeque<A>, S>,
}

impl Record<Infallible> {
    /// Returns a new record builder.
    pub fn builder<S>() -> Builder<S> {
        Builder::new()
    }
}

impl<A> Record<A> {
    /// Returns a new record.
    pub fn new() -> Record<A> {
        Record::builder().build()
    }
}

impl<A, S> Record<A, S> {
    /// Reserves capacity for at least `additional` more actions.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    pub fn reserve(&mut self, additional: usize) {
        self.record.entries.deque.reserve(additional);
    }

    /// Returns the capacity of the record.
    pub fn capacity(&self) -> usize {
        self.record.entries.deque.capacity()
    }

    /// Shrinks the capacity of the record as much as possible.
    pub fn shrink_to_fit(&mut self) {
        self.record.entries.deque.shrink_to_fit();
    }

    /// Returns the number of actions in the record.
    pub fn len(&self) -> usize {
        self.record.entries.deque.len()
    }

    /// Returns `true` if the record is empty.
    pub fn is_empty(&self) -> bool {
        self.record.entries.deque.is_empty()
    }

    /// Returns the limit of the record.
    pub fn limit(&self) -> usize {
        self.record.entries.limit.get()
    }

    /// Sets how the signal should be handled when the state changes.
    pub fn connect(&mut self, slot: S) -> Option<S> {
        self.record.slot.connect(Some(slot))
    }

    /// Removes and returns the slot if it exists.
    pub fn disconnect(&mut self) -> Option<S> {
        self.record.slot.disconnect()
    }

    /// Returns `true` if the record can undo.
    pub fn can_undo(&self) -> bool {
        self.record.can_undo()
    }

    /// Returns `true` if the record can redo.
    pub fn can_redo(&self) -> bool {
        self.record.can_redo()
    }

    /// Returns `true` if the target is in a saved state, `false` otherwise.
    pub fn is_saved(&self) -> bool {
        self.record.is_saved()
    }

    /// Returns the position of the current action.
    pub fn current(&self) -> usize {
        self.record.current
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<A, S> {
        Queue::from(self)
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, S> {
        Checkpoint::from(self)
    }

    /// Returns a structure for configurable formatting of the record.
    pub fn display(&self) -> Display<A, S> {
        Display::from(self)
    }

    /// Returns an iterator over the actions.
    pub fn actions(&self) -> impl Iterator<Item = &A> {
        self.record.entries.deque.iter().map(|e| &e.action)
    }
}

impl<A: Action, S: Slot> Record<A, S> {
    /// Pushes the action on top of the record and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned.
    ///
    /// [`apply`]: trait.Action.html#tymethod.apply
    pub fn apply(&mut self, target: &mut A::Target, action: A) -> Result<A> {
        self.record
            .apply(target, action)
            .map(|(output, _, _)| output)
    }

    /// Calls the [`undo`] method for the active action and sets
    /// the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned.
    ///
    /// [`undo`]: ../trait.Action.html#tymethod.undo
    pub fn undo(&mut self, target: &mut A::Target) -> Option<Result<A>> {
        self.record.undo(target)
    }

    /// Calls the [`redo`] method for the active action and sets
    /// the next one as the new active one.
    ///
    /// # Errors
    /// If an error occur when applying [`redo`] the error is returned.
    ///
    /// [`redo`]: trait.Action.html#method.redo
    pub fn redo(&mut self, target: &mut A::Target) -> Option<Result<A>> {
        self.record.redo(target)
    }

    /// Marks the target as currently being in a saved or unsaved state.
    pub fn set_saved(&mut self, saved: bool) {
        self.record.set_saved(saved)
    }

    /// Removes all actions from the record without undoing them.
    pub fn clear(&mut self) {
        self.record.clear()
    }
}

impl<A: Action<Output = ()>, S: Slot> Record<A, S> {
    /// Revert the changes done to the target since the saved state.
    pub fn revert(&mut self, target: &mut A::Target) -> Option<Result<A>> {
        self.record.revert(target)
    }

    /// Repeatedly calls [`undo`] or [`redo`] until the action at `current` is reached.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned.
    ///
    /// [`undo`]: trait.Action.html#tymethod.undo
    /// [`redo`]: trait.Action.html#method.redo
    pub fn go_to(&mut self, target: &mut A::Target, current: usize) -> Option<Result<A>> {
        self.record.go_to(target, current)
    }

    /// Go back or forward in the record to the action that was made closest to the datetime provided.
    #[cfg(feature = "time")]
    pub fn time_travel(
        &mut self,
        target: &mut A::Target,
        to: &OffsetDateTime,
    ) -> Option<Result<A>> {
        let current = self
            .record
            .entries
            .deque
            .binary_search_by(|e| e.timestamp.cmp(to))
            .unwrap_or_else(identity);
        self.go_to(target, current)
    }
}

impl<A: ToString, S> Record<A, S> {
    /// Returns the string of the action which will be undone
    /// in the next call to [`undo`](struct.Timeline.html#method.undo).
    pub fn undo_text(&self) -> Option<String> {
        self.record
            .current
            .checked_sub(1)
            .and_then(|i| self.text(i))
    }

    /// Returns the string of the action which will be redone
    /// in the next call to [`redo`](struct.Timeline.html#method.redo).
    pub fn redo_text(&self) -> Option<String> {
        self.text(self.record.current)
    }

    fn text(&self, i: usize) -> Option<String> {
        self.record
            .entries
            .deque
            .get(i)
            .map(|e| e.action.to_string())
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

/// Builder for a record.
///
/// # Examples
/// ```
/// # include!("../push.rs");
/// # fn main() {
/// # use undo::Record;
///
/// let _ = Record::builder()
///     .limit(100)
///     .capacity(100)
///     .connect(|s| { dbg!(s); })
///     .build::<Push>();
/// # }
/// ```
#[derive(Debug)]
pub struct Builder<S = NoOp> {
    capacity: usize,
    limit: NonZeroUsize,
    saved: bool,
    slot: SW<S>,
}

impl<S> Builder<S> {
    /// Returns a builder for a record.
    pub fn new() -> Builder<S> {
        Builder {
            capacity: 0,
            limit: NonZeroUsize::new(usize::MAX).unwrap(),
            saved: true,
            slot: SW::default(),
        }
    }

    /// Sets the capacity for the record.
    pub fn capacity(mut self, capacity: usize) -> Builder<S> {
        self.capacity = capacity;
        self
    }

    /// Sets the `limit` of the record.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    pub fn limit(mut self, limit: usize) -> Builder<S> {
        self.limit = NonZeroUsize::new(limit).expect("limit can not be `0`");
        self
    }

    /// Sets if the target is initially in a saved state.
    /// By default the target is in a saved state.
    pub fn saved(mut self, saved: bool) -> Builder<S> {
        self.saved = saved;
        self
    }

    /// Builds the record.
    pub fn build<A>(self) -> Record<A, S> {
        Record {
            record: Timeline {
                entries: LimitDeque::new(self.capacity, self.limit),
                current: 0,
                saved: self.saved.then_some(0),
                slot: self.slot,
            },
        }
    }
}

impl<S: Slot> Builder<S> {
    /// Connects the slot.
    pub fn connect(mut self, f: S) -> Builder<S> {
        self.slot = SW::new(f);
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
/// # use undo::{Record};
/// # include!("../push.rs");
/// # fn main() {
/// let mut string = String::new();
/// let mut record = Record::new();
/// let mut queue = record.queue();
/// queue.apply(Push('a'));
/// queue.apply(Push('b'));
/// queue.apply(Push('c'));
/// assert_eq!(string, "");
/// queue.commit(&mut string).unwrap().unwrap();
/// assert_eq!(string, "abc");
/// # }
/// ```
#[derive(Debug)]
pub struct Queue<'a, A, S> {
    record: &'a mut Record<A, S>,
    actions: Vec<QueueAction<A>>,
}

impl<A: Action<Output = ()>, S: Slot> Queue<'_, A, S> {
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
    pub fn queue(&mut self) -> Queue<A, S> {
        self.record.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, S> {
        self.record.checkpoint()
    }
}

impl<'a, A, S> From<&'a mut Record<A, S>> for Queue<'a, A, S> {
    fn from(record: &'a mut Record<A, S>) -> Self {
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
pub struct Checkpoint<'a, A, S> {
    record: &'a mut Record<A, S>,
    actions: Vec<CheckpointAction<A>>,
}

impl<A: Action<Output = ()>, S: Slot> Checkpoint<'_, A, S> {
    /// Calls the `apply` method.
    pub fn apply(&mut self, target: &mut A::Target, action: A) -> Result<A> {
        let saved = self.record.record.saved;
        let (_, _, tail) = self.record.record.apply(target, action)?;
        self.actions
            .push(CheckpointAction::Apply(saved, tail.deque));
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
                        self.record.record.entries.deque.pop_back();
                        self.record.record.entries.deque.append(&mut entries);
                        self.record.record.saved = saved;
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
    pub fn queue(&mut self) -> Queue<A, S> {
        self.record.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, S> {
        self.record.checkpoint()
    }
}

impl<'a, A, S> From<&'a mut Record<A, S>> for Checkpoint<'a, A, S> {
    fn from(record: &'a mut Record<A, S>) -> Self {
        Checkpoint {
            record,
            actions: Vec::new(),
        }
    }
}

/// Configurable display formatting for the record.
pub struct Display<'a, A, S> {
    record: &'a Record<A, S>,
    format: Format,
}

impl<A, S> Display<'_, A, S> {
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

impl<A: fmt::Display, S> Display<'_, A, S> {
    fn fmt_list(&self, f: &mut fmt::Formatter, at: At, entry: Option<&Entry<A>>) -> fmt::Result {
        self.format.position(f, at, false)?;

        #[cfg(feature = "time")]
        if let Some(entry) = entry {
            if self.format.detailed {
                self.format.timestamp(f, &entry.timestamp)?;
            }
        }

        self.format.labels(
            f,
            at,
            At::new(0, self.record.current()),
            self.record.record.saved.map(|saved| At::new(0, saved)),
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

impl<'a, A, S> From<&'a Record<A, S>> for Display<'a, A, S> {
    fn from(record: &'a Record<A, S>) -> Self {
        Display {
            record,
            format: Format::default(),
        }
    }
}

impl<A: fmt::Display, S> fmt::Display for Display<'_, A, S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, entry) in self.record.record.entries.deque.iter().enumerate().rev() {
            let at = At::new(0, i + 1);
            self.fmt_list(f, at, Some(entry))?;
        }
        self.fmt_list(f, At::ROOT, None)
    }
}

/// A deque that holds a limit of how many items it can hold.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub(crate) struct LimitDeque<T> {
    pub deque: VecDeque<Entry<T>>,
    pub limit: NonZeroUsize,
}

impl<T> LimitDeque<T> {
    pub fn new(capacity: usize, limit: NonZeroUsize) -> LimitDeque<T> {
        LimitDeque {
            deque: VecDeque::with_capacity(capacity),
            limit,
        }
    }
}

impl<T> Entries for LimitDeque<T> {
    type Item = T;

    fn limit(&self) -> usize {
        self.limit.get()
    }

    fn len(&self) -> usize {
        self.deque.len()
    }

    fn back_mut(&mut self) -> Option<&mut Entry<T>> {
        self.deque.back_mut()
    }

    fn push_back(&mut self, t: Entry<T>) {
        self.deque.push_back(t)
    }

    fn pop_front(&mut self) -> Option<Entry<T>> {
        self.deque.pop_front()
    }

    fn pop_back(&mut self) -> Option<Entry<T>> {
        self.deque.pop_back()
    }

    fn split_off(&mut self, at: usize) -> Self {
        LimitDeque {
            deque: self.deque.split_off(at),
            limit: self.limit,
        }
    }

    fn clear(&mut self) {
        self.deque.clear();
    }
}

impl<T> Index<usize> for LimitDeque<T> {
    type Output = Entry<T>;

    fn index(&self, index: usize) -> &Self::Output {
        self.deque.index(index)
    }
}

impl<T> IndexMut<usize> for LimitDeque<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.deque.index_mut(index)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use alloc::string::String;
    use alloc::vec::Vec;

    enum Edit {
        Push(Push),
        Pop(Pop),
    }

    impl Action for Edit {
        type Target = String;
        type Output = ();
        type Error = &'static str;

        fn apply(&mut self, s: &mut String) -> Result<Push> {
            match self {
                Edit::Push(add) => add.apply(s),
                Edit::Pop(del) => del.apply(s),
            }
        }

        fn undo(&mut self, s: &mut String) -> Result<Push> {
            match self {
                Edit::Push(add) => add.undo(s),
                Edit::Pop(del) => del.undo(s),
            }
        }

        fn merge(&mut self, edit: Self) -> Merged<Self>
        where
            Self: Sized,
        {
            match (self, edit) {
                (Edit::Push(_), Edit::Pop(_)) => Merged::Annul,
                (Edit::Pop(Pop(Some(a))), Edit::Push(Push(b))) if a == &b => Merged::Annul,
                (_, edit) => Merged::No(edit),
            }
        }
    }

    #[derive(Debug, PartialEq)]
    struct Push(char);

    impl Action for Push {
        type Target = String;
        type Output = ();
        type Error = &'static str;

        fn apply(&mut self, s: &mut String) -> Result<Push> {
            s.push(self.0);
            Ok(())
        }

        fn undo(&mut self, s: &mut String) -> Result<Push> {
            self.0 = s.pop().ok_or("s is empty")?;
            Ok(())
        }
    }

    #[derive(Default)]
    struct Pop(Option<char>);

    impl Action for Pop {
        type Target = String;
        type Output = ();
        type Error = &'static str;

        fn apply(&mut self, s: &mut String) -> Result<Push> {
            self.0 = s.pop();
            Ok(())
        }

        fn undo(&mut self, s: &mut String) -> Result<Push> {
            let ch = self.0.ok_or("s is empty")?;
            s.push(ch);
            Ok(())
        }
    }

    #[test]
    fn go_to() {
        let mut target = String::new();
        let mut record = Record::new();
        record.apply(&mut target, Push('a')).unwrap();
        record.apply(&mut target, Push('b')).unwrap();
        record.apply(&mut target, Push('c')).unwrap();
        record.apply(&mut target, Push('d')).unwrap();
        record.apply(&mut target, Push('e')).unwrap();

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
        q3.apply(Push('a'));
        q3.apply(Push('b'));
        q3.apply(Push('c'));
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
        cp1.apply(&mut target, Push('a')).unwrap();
        cp1.apply(&mut target, Push('b')).unwrap();
        cp1.apply(&mut target, Push('c')).unwrap();
        assert_eq!(target, "abc");
        let mut cp2 = cp1.checkpoint();
        cp2.apply(&mut target, Push('d')).unwrap();
        cp2.apply(&mut target, Push('e')).unwrap();
        cp2.apply(&mut target, Push('f')).unwrap();
        assert_eq!(target, "abcdef");
        let mut cp3 = cp2.checkpoint();
        cp3.apply(&mut target, Push('g')).unwrap();
        cp3.apply(&mut target, Push('h')).unwrap();
        cp3.apply(&mut target, Push('i')).unwrap();
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
        cp1.apply(&mut target, Push('a')).unwrap();
        cp1.apply(&mut target, Push('b')).unwrap();
        cp1.apply(&mut target, Push('c')).unwrap();
        let mut cp2 = cp1.checkpoint();
        cp2.apply(&mut target, Push('d')).unwrap();
        cp2.apply(&mut target, Push('e')).unwrap();
        cp2.apply(&mut target, Push('f')).unwrap();
        let mut cp3 = cp2.checkpoint();
        cp3.apply(&mut target, Push('g')).unwrap();
        cp3.apply(&mut target, Push('h')).unwrap();
        cp3.apply(&mut target, Push('i')).unwrap();
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
        record.apply(&mut target, Push('a')).unwrap();
        record.apply(&mut target, Push('b')).unwrap();
        record.apply(&mut target, Push('c')).unwrap();
        record.set_saved(true);
        record.undo(&mut target).unwrap().unwrap();
        record.undo(&mut target).unwrap().unwrap();
        record.undo(&mut target).unwrap().unwrap();
        let mut cp = record.checkpoint();
        cp.apply(&mut target, Push('d')).unwrap();
        cp.apply(&mut target, Push('e')).unwrap();
        cp.apply(&mut target, Push('f')).unwrap();
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
    fn annul() {
        let mut target = String::new();
        let mut record = Record::new();
        record.apply(&mut target, Edit::Push(Push('a'))).unwrap();
        record
            .apply(&mut target, Edit::Pop(Pop::default()))
            .unwrap();
        record.apply(&mut target, Edit::Push(Push('b'))).unwrap();
        assert_eq!(record.len(), 1);
    }

    #[test]
    fn actions() {
        let mut target = String::new();
        let mut record = Record::new();
        record.apply(&mut target, Push('a')).unwrap();
        record.apply(&mut target, Push('b')).unwrap();
        let collected = record.actions().collect::<Vec<_>>();
        assert_eq!(&collected[..], &[&Push('a'), &Push('b')][..]);
    }
}
