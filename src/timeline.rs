//! A timeline of actions.

use crate::entry::{Entries, Stack};
use crate::slot::{NoOp, Slot, SW};
use crate::{Action, At, Entry, Format, History, Result};
use alloc::{
    collections::VecDeque,
    string::{String, ToString},
    vec::Vec,
};
use core::ops::{Index, IndexMut};
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

/// A linear timeline of actions.
///
/// The timeline can roll the targets state backwards and forwards by using
/// the undo and redo methods. In addition, the timeline can notify the user
/// about changes to the stack or the target through [signal](enum.Signal.html).
/// The user can give the timeline a function that is called each time the state
/// changes by using the [`builder`](struct.TimelineBuilder.html).
///
/// # Examples
/// ```
/// # use undo::Timeline;
/// # include!("../add.rs");
/// # fn main() {
/// let mut target = String::new();
/// let mut timeline = Timeline::new();
/// timeline.apply(&mut target, Add('a')).unwrap();
/// timeline.apply(&mut target, Add('b')).unwrap();
/// timeline.apply(&mut target, Add('c')).unwrap();
/// assert_eq!(target, "abc");
/// timeline.undo(&mut target).unwrap().unwrap();
/// timeline.undo(&mut target).unwrap().unwrap();
/// timeline.undo(&mut target).unwrap().unwrap();
/// assert_eq!(target, "");
/// timeline.redo(&mut target).unwrap().unwrap();
/// timeline.redo(&mut target).unwrap().unwrap();
/// timeline.redo(&mut target).unwrap().unwrap();
/// assert_eq!(target, "abc");
/// # }
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct Timeline<A, F = NoOp> {
    pub(crate) stack: Stack<LimitDeque<A>, F>,
}

impl<A> Timeline<A> {
    /// Returns a new timeline.
    pub fn new() -> Timeline<A> {
        TimelineBuilder::new().build()
    }
}

impl<A, F> Timeline<A, F> {
    /// Returns a new builder for a timeline.
    pub fn builder() -> TimelineBuilder<F> {
        TimelineBuilder::new()
    }

    /// Reserves capacity for at least `additional` more actions.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    pub fn reserve(&mut self, additional: usize) {
        self.stack.entries.deque.reserve(additional);
    }

    /// Returns the capacity of the timeline.
    pub fn capacity(&self) -> usize {
        self.stack.entries.deque.capacity()
    }

    /// Shrinks the capacity of the timeline as much as possible.
    pub fn shrink_to_fit(&mut self) {
        self.stack.entries.deque.shrink_to_fit();
    }

    /// Returns the number of actions in the timeline.
    pub fn len(&self) -> usize {
        self.stack.entries.deque.len()
    }

    /// Returns `true` if the timeline is empty.
    pub fn is_empty(&self) -> bool {
        self.stack.entries.deque.is_empty()
    }

    /// Returns the limit of the timeline.
    pub fn limit(&self) -> usize {
        self.stack.entries.limit.get()
    }

    /// Sets how the signal should be handled when the state changes.
    pub fn connect(&mut self, slot: F) -> Option<F> {
        self.stack.slot.connect(Some(slot))
    }

    /// Removes and returns the slot if it exists.
    pub fn disconnect(&mut self) -> Option<F> {
        self.stack.slot.disconnect()
    }

    /// Returns `true` if the timeline can undo.
    pub fn can_undo(&self) -> bool {
        self.stack.can_undo()
    }

    /// Returns `true` if the timeline can redo.
    pub fn can_redo(&self) -> bool {
        self.stack.can_redo()
    }

    /// Returns `true` if the target is in a saved state, `false` otherwise.
    pub fn is_saved(&self) -> bool {
        self.stack.is_saved()
    }

    /// Returns the position of the current action.
    pub fn current(&self) -> usize {
        self.stack.current
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<A, F> {
        Queue::from(self)
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, F> {
        Checkpoint::from(self)
    }

    /// Returns a structure for configurable formatting of the timeline.
    pub fn display(&self) -> Display<A, F> {
        Display::from(self)
    }
}

impl<A: Action, F: Slot> Timeline<A, F> {
    /// Pushes the action on top of the timeline and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned.
    ///
    /// [`apply`]: trait.Action.html#tymethod.apply
    pub fn apply(&mut self, target: &mut A::Target, action: A) -> Result<A> {
        self.stack
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
        self.stack.undo(target)
    }

    /// Calls the [`redo`] method for the active action and sets
    /// the next one as the new active one.
    ///
    /// # Errors
    /// If an error occur when applying [`redo`] the error is returned.
    ///
    /// [`redo`]: trait.Action.html#method.redo
    pub fn redo(&mut self, target: &mut A::Target) -> Option<Result<A>> {
        self.stack.redo(target)
    }

    /// Marks the target as currently being in a saved or unsaved state.
    pub fn set_saved(&mut self, saved: bool) {
        self.stack.set_saved(saved)
    }

    /// Removes all actions from the timeline without undoing them.
    pub fn clear(&mut self) {
        self.stack.clear()
    }
}

impl<A: Action<Output = ()>, F: Slot> Timeline<A, F> {
    /// Revert the changes done to the target since the saved state.
    pub fn revert(&mut self, target: &mut A::Target) -> Option<Result<A>> {
        self.stack.revert(target)
    }

    /// Repeatedly calls [`undo`] or [`redo`] until the action at `current` is reached.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned.
    ///
    /// [`undo`]: trait.Action.html#tymethod.undo
    /// [`redo`]: trait.Action.html#method.redo
    pub fn go_to(&mut self, target: &mut A::Target, current: usize) -> Option<Result<A>> {
        self.stack.go_to(target, current)
    }

    /// Go back or forward in the timeline to the action that was made closest to the datetime provided.
    #[cfg(feature = "chrono")]
    pub fn time_travel(&mut self, target: &mut A::Target, to: &DateTime<Utc>) -> Option<Result<A>> {
        let current = match self.stack.entries.deque.as_slices() {
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

impl<A: ToString, F> Timeline<A, F> {
    /// Returns the string of the action which will be undone
    /// in the next call to [`undo`](struct.Timeline.html#method.undo).
    pub fn undo_text(&self) -> Option<String> {
        self.stack.current.checked_sub(1).and_then(|i| self.text(i))
    }

    /// Returns the string of the action which will be redone
    /// in the next call to [`redo`](struct.Timeline.html#method.redo).
    pub fn redo_text(&self) -> Option<String> {
        self.text(self.stack.current)
    }

    fn text(&self, i: usize) -> Option<String> {
        self.stack
            .entries
            .deque
            .get(i)
            .map(|e| e.action.to_string())
    }
}

impl<A> Default for Timeline<A> {
    fn default() -> Timeline<A> {
        Timeline::new()
    }
}

impl<A, F> From<History<A, F>> for Timeline<A, F> {
    fn from(history: History<A, F>) -> Timeline<A, F> {
        history.timeline
    }
}

/// Builder for a timeline.
///
/// # Examples
/// ```
/// # include!("../add.rs");
/// # fn main() {
/// # use undo::{timeline::TimelineBuilder, Timeline};
///
/// let _ = TimelineBuilder::new()
///     .limit(100)
///     .capacity(100)
///     .connect(|s| { dbg!(s); })
///     .build::<Add>();
/// # }
/// ```
#[derive(Debug)]
pub struct TimelineBuilder<F = NoOp> {
    capacity: usize,
    limit: NonZeroUsize,
    saved: bool,
    slot: SW<F>,
}

impl<F> TimelineBuilder<F> {
    /// Returns a builder for a timeline.
    pub fn new() -> TimelineBuilder<F> {
        TimelineBuilder {
            capacity: 0,
            limit: NonZeroUsize::new(usize::MAX).unwrap(),
            saved: true,
            slot: SW::default(),
        }
    }

    /// Sets the capacity for the timeline.
    pub fn capacity(mut self, capacity: usize) -> TimelineBuilder<F> {
        self.capacity = capacity;
        self
    }

    /// Sets the `limit` of the timeline.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    pub fn limit(mut self, limit: usize) -> TimelineBuilder<F> {
        self.limit = NonZeroUsize::new(limit).expect("limit can not be `0`");
        self
    }

    /// Sets if the target is initially in a saved state.
    /// By default the target is in a saved state.
    pub fn saved(mut self, saved: bool) -> TimelineBuilder<F> {
        self.saved = saved;
        self
    }

    /// Builds the timeline.
    pub fn build<A>(self) -> Timeline<A, F> {
        Timeline {
            stack: Stack {
                entries: LimitDeque {
                    deque: VecDeque::with_capacity(self.capacity),
                    limit: self.limit,
                },
                current: 0,
                saved: self.saved.then_some(0),
                slot: self.slot,
            },
        }
    }
}

impl<F: Slot> TimelineBuilder<F> {
    /// Connects the slot.
    pub fn connect(mut self, f: F) -> TimelineBuilder<F> {
        self.slot = SW::new(f);
        self
    }
}

impl Default for TimelineBuilder {
    fn default() -> Self {
        TimelineBuilder::new()
    }
}

#[derive(Debug)]
enum QueueAction<A> {
    Apply(A),
    Undo,
    Redo,
}

/// Wraps a timeline and gives it batch queue functionality.
///
/// # Examples
/// ```
/// # use undo::{Timeline};
/// # include!("../add.rs");
/// # fn main() {
/// let mut string = String::new();
/// let mut timeline = Timeline::new();
/// let mut queue = timeline.queue();
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
    timeline: &'a mut Timeline<A, F>,
    actions: Vec<QueueAction<A>>,
}

impl<A: Action<Output = ()>, F: Slot> Queue<'_, A, F> {
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
                QueueAction::Apply(action) => Some(self.timeline.apply(target, action)),
                QueueAction::Undo => self.timeline.undo(target),
                QueueAction::Redo => self.timeline.redo(target),
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
        self.timeline.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, F> {
        self.timeline.checkpoint()
    }
}

impl<'a, A, F> From<&'a mut Timeline<A, F>> for Queue<'a, A, F> {
    fn from(timeline: &'a mut Timeline<A, F>) -> Self {
        Queue {
            timeline,
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

/// Wraps a timeline and gives it checkpoint functionality.
#[derive(Debug)]
pub struct Checkpoint<'a, A, F> {
    timeline: &'a mut Timeline<A, F>,
    actions: Vec<CheckpointAction<A>>,
}

impl<A: Action<Output = ()>, F: Slot> Checkpoint<'_, A, F> {
    /// Calls the `apply` method.
    pub fn apply(&mut self, target: &mut A::Target, action: A) -> Result<A> {
        let saved = self.timeline.stack.saved;
        let (_, _, tail) = self.timeline.stack.apply(target, action)?;
        self.actions
            .push(CheckpointAction::Apply(saved, tail.deque));
        Ok(())
    }

    /// Calls the `undo` method.
    pub fn undo(&mut self, target: &mut A::Target) -> Option<Result<A>> {
        match self.timeline.undo(target) {
            o @ Some(Ok(())) => {
                self.actions.push(CheckpointAction::Undo);
                o
            }
            o => o,
        }
    }

    /// Calls the `redo` method.
    pub fn redo(&mut self, target: &mut A::Target) -> Option<Result<A>> {
        match self.timeline.redo(target) {
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
                CheckpointAction::Apply(saved, mut entries) => match self.timeline.undo(target) {
                    Some(Ok(())) => {
                        self.timeline.stack.entries.deque.pop_back();
                        self.timeline.stack.entries.deque.append(&mut entries);
                        self.timeline.stack.saved = saved;
                    }
                    o => return o,
                },
                CheckpointAction::Undo => match self.timeline.redo(target) {
                    Some(Ok(())) => (),
                    o => return o,
                },
                CheckpointAction::Redo => match self.timeline.undo(target) {
                    Some(Ok(())) => (),
                    o => return o,
                },
            };
        }
        Some(Ok(()))
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<A, F> {
        self.timeline.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, F> {
        self.timeline.checkpoint()
    }
}

impl<'a, A, F> From<&'a mut Timeline<A, F>> for Checkpoint<'a, A, F> {
    fn from(timeline: &'a mut Timeline<A, F>) -> Self {
        Checkpoint {
            timeline,
            actions: Vec::new(),
        }
    }
}

/// Configurable display formatting for the timeline.
pub struct Display<'a, A, F> {
    timeline: &'a Timeline<A, F>,
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
            At::new(0, self.timeline.current()),
            self.timeline.stack.saved.map(|saved| At::new(0, saved)),
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

impl<'a, A, F> From<&'a Timeline<A, F>> for Display<'a, A, F> {
    fn from(timeline: &'a Timeline<A, F>) -> Self {
        Display {
            timeline,
            format: Format::default(),
        }
    }
}

impl<A: fmt::Display, F> fmt::Display for Display<'_, A, F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, entry) in self.timeline.stack.entries.deque.iter().enumerate().rev() {
            let at = At::new(0, i + 1);
            self.fmt_list(f, at, Some(entry))?;
        }
        self.fmt_list(f, At::ROOT, None)
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub(crate) struct LimitDeque<T> {
    pub deque: VecDeque<Entry<T>>,
    pub limit: NonZeroUsize,
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

        fn merge(&mut self, edit: Self) -> Merged<Self>
        where
            Self: Sized,
        {
            match (self, edit) {
                (Edit::Add(_), Edit::Del(_)) => Merged::Annul,
                (Edit::Del(Del(Some(a))), Edit::Add(Add(b))) if a == &b => Merged::Annul,
                (_, edit) => Merged::No(edit),
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
        let mut timeline = Timeline::new();
        timeline.apply(&mut target, Add('a')).unwrap();
        timeline.apply(&mut target, Add('b')).unwrap();
        timeline.apply(&mut target, Add('c')).unwrap();
        timeline.apply(&mut target, Add('d')).unwrap();
        timeline.apply(&mut target, Add('e')).unwrap();

        timeline.go_to(&mut target, 0).unwrap().unwrap();
        assert_eq!(timeline.current(), 0);
        assert_eq!(target, "");
        timeline.go_to(&mut target, 5).unwrap().unwrap();
        assert_eq!(timeline.current(), 5);
        assert_eq!(target, "abcde");
        timeline.go_to(&mut target, 1).unwrap().unwrap();
        assert_eq!(timeline.current(), 1);
        assert_eq!(target, "a");
        timeline.go_to(&mut target, 4).unwrap().unwrap();
        assert_eq!(timeline.current(), 4);
        assert_eq!(target, "abcd");
        timeline.go_to(&mut target, 2).unwrap().unwrap();
        assert_eq!(timeline.current(), 2);
        assert_eq!(target, "ab");
        timeline.go_to(&mut target, 3).unwrap().unwrap();
        assert_eq!(timeline.current(), 3);
        assert_eq!(target, "abc");
        assert!(timeline.go_to(&mut target, 6).is_none());
        assert_eq!(timeline.current(), 3);
    }

    #[test]
    fn queue_commit() {
        let mut target = String::new();
        let mut timeline = Timeline::new();
        let mut q1 = timeline.queue();
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
        let mut timeline = Timeline::new();
        let mut cp1 = timeline.checkpoint();
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
        let mut timeline = Timeline::new();
        let mut cp1 = timeline.checkpoint();
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
        let mut timeline = Timeline::new();
        timeline.apply(&mut target, Add('a')).unwrap();
        timeline.apply(&mut target, Add('b')).unwrap();
        timeline.apply(&mut target, Add('c')).unwrap();
        timeline.set_saved(true);
        timeline.undo(&mut target).unwrap().unwrap();
        timeline.undo(&mut target).unwrap().unwrap();
        timeline.undo(&mut target).unwrap().unwrap();
        let mut cp = timeline.checkpoint();
        cp.apply(&mut target, Add('d')).unwrap();
        cp.apply(&mut target, Add('e')).unwrap();
        cp.apply(&mut target, Add('f')).unwrap();
        assert_eq!(target, "def");
        cp.cancel(&mut target).unwrap().unwrap();
        assert_eq!(target, "");
        timeline.redo(&mut target).unwrap().unwrap();
        timeline.redo(&mut target).unwrap().unwrap();
        timeline.redo(&mut target).unwrap().unwrap();
        assert!(timeline.is_saved());
        assert_eq!(target, "abc");
    }

    #[test]
    fn annul() {
        let mut target = String::new();
        let mut timeline = Timeline::new();
        timeline.apply(&mut target, Edit::Add(Add('a'))).unwrap();
        timeline
            .apply(&mut target, Edit::Del(Del::default()))
            .unwrap();
        timeline.apply(&mut target, Edit::Add(Add('b'))).unwrap();
        assert_eq!(timeline.len(), 1);
    }
}
