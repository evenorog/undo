//! A history of actions.

use crate::record::Builder as RecordBuilder;
use crate::slot::{NoOp, Signal, Slot};
use crate::{Action, At, Entry, Format, Record};
use alloc::{
    collections::{BTreeMap, VecDeque},
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::convert::Infallible;
use core::fmt::{self, Write};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A history tree of actions.
///
/// Unlike [Timeline](struct.Timeline.html) which maintains a linear undo history,
/// History maintains an undo tree containing every edit made to the target.
///
/// # Examples
/// ```
/// # use undo::History;
/// # include!("../push.rs");
/// # fn main() {
/// let mut target = String::new();
/// let mut history = History::new();
/// history.apply(&mut target, Push('a'));
/// history.apply(&mut target, Push('b'));
/// history.apply(&mut target, Push('c'));
/// let abc = history.branch();
/// history.go_to(&mut target, abc, 1);
/// history.apply(&mut target, Push('f'));
/// history.apply(&mut target, Push('g'));
/// assert_eq!(target, "afg");
/// history.go_to(&mut target, abc, 3);
/// assert_eq!(target, "abc");
/// # }
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct History<A, S = NoOp> {
    root: usize,
    next: usize,
    pub(crate) saved: Option<At>,
    pub(crate) record: Record<A, S>,
    pub(crate) branches: BTreeMap<usize, Branch<A>>,
}

impl History<Infallible> {
    /// Returns a new history builder.
    pub fn builder<S>() -> Builder<S> {
        Builder::new()
    }
}

impl<A> History<A> {
    /// Returns a new history.
    pub fn new() -> History<A> {
        History::builder().build()
    }
}

impl<A, S> History<A, S> {
    /// Reserves capacity for at least `additional` more actions.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    pub fn reserve(&mut self, additional: usize) {
        self.record.reserve(additional);
    }

    /// Returns the capacity of the history.
    pub fn capacity(&self) -> usize {
        self.record.capacity()
    }

    /// Shrinks the capacity of the history as much as possible.
    pub fn shrink_to_fit(&mut self) {
        self.record.shrink_to_fit();
    }

    /// Returns the number of actions in the current branch of the history.
    pub fn len(&self) -> usize {
        self.record.len()
    }

    /// Returns `true` if the current branch of the history is empty.
    pub fn is_empty(&self) -> bool {
        self.record.is_empty()
    }

    /// Returns the limit of the history.
    pub fn limit(&self) -> usize {
        self.record.limit()
    }

    /// Sets how the signal should be handled when the state changes.
    pub fn connect(&mut self, slot: S) -> Option<S> {
        self.record.connect(slot)
    }

    /// Removes and returns the slot if it exists.
    pub fn disconnect(&mut self) -> Option<S> {
        self.record.disconnect()
    }

    /// Returns `true` if the target is in a saved state, `false` otherwise.
    pub fn is_saved(&self) -> bool {
        self.record.is_saved()
    }

    /// Returns `true` if the history can undo.
    pub fn can_undo(&self) -> bool {
        self.record.can_undo()
    }

    /// Returns `true` if the history can redo.
    pub fn can_redo(&self) -> bool {
        self.record.can_redo()
    }

    /// Returns the current branch.
    pub fn branch(&self) -> usize {
        self.root
    }

    /// Returns the position of the current action.
    pub fn current(&self) -> usize {
        self.record.current()
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<A, S> {
        Queue::from(self)
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, S> {
        Checkpoint::from(self)
    }

    /// Returns a structure for configurable formatting of the history.
    pub fn display(&self) -> Display<A, S> {
        Display::from(self)
    }

    /// Returns an iterator over the actions in the current branch.
    pub fn actions(&self) -> impl Iterator<Item = &A> {
        self.record.actions()
    }

    fn at(&self) -> At {
        At::new(self.branch(), self.current())
    }
}

impl<A: Action, S: Slot> History<A, S> {
    /// Pushes the action to the top of the history and executes its [`apply`] method.
    ///
    /// [`apply`]: trait.Action.html#tymethod.apply
    pub fn apply(&mut self, target: &mut A::Target, action: A) -> A::Output {
        let at = self.at();
        let saved = self
            .record
            .timeline
            .saved
            .filter(|&saved| saved > at.current);
        let (output, merged, tail) = self.record.timeline.apply(target, action);
        // Check if the limit has been reached.
        if !merged && at.current == self.current() {
            let root = self.branch();
            self.rm_child(root, 0);
            self.branches
                .values_mut()
                .filter(|branch| branch.parent.branch == root)
                .for_each(|branch| branch.parent.current -= 1);
        }
        // Handle new branch.
        if !tail.deque.is_empty() {
            let new = self.next;
            self.next += 1;
            self.branches
                .insert(at.branch, Branch::new(new, at.current, tail.deque));
            self.set_root(new, at.current, saved);
        }
        output
    }

    /// Calls the [`undo`] method for the active action
    /// and sets the previous one as the new active one.
    ///
    /// [`undo`]: trait.Action.html#tymethod.undo
    pub fn undo(&mut self, target: &mut A::Target) -> Option<A::Output> {
        self.record.undo(target)
    }

    /// Calls the [`redo`] method for the active action
    /// and sets the next one as the new active one.
    ///
    /// [`redo`]: trait.Action.html#method.redo
    pub fn redo(&mut self, target: &mut A::Target) -> Option<A::Output> {
        self.record.redo(target)
    }

    /// Marks the target as currently being in a saved or unsaved state.
    pub fn set_saved(&mut self, saved: bool) {
        self.saved = None;
        self.record.set_saved(saved);
    }

    /// Removes all actions from the history without undoing them.
    pub fn clear(&mut self) {
        self.root = 0;
        self.next = 1;
        self.saved = None;
        self.record.clear();
        self.branches.clear();
    }

    pub(crate) fn jump_to(&mut self, root: usize) {
        let mut branch = self.branches.remove(&root).unwrap();
        debug_assert_eq!(branch.parent, self.at());
        let current = self.current();
        let saved = self.record.timeline.saved.filter(|&saved| saved > current);
        let tail = self.record.timeline.entries.deque.split_off(current);
        self.record
            .timeline
            .entries
            .deque
            .append(&mut branch.entries);
        self.branches
            .insert(self.root, Branch::new(root, current, tail));
        self.set_root(root, current, saved);
    }

    fn set_root(&mut self, root: usize, current: usize, saved: Option<usize>) {
        let old = self.branch();
        self.root = root;
        debug_assert_ne!(old, root);
        // Handle the child branches.
        self.branches
            .values_mut()
            .filter(|branch| branch.parent.branch == old && branch.parent.current <= current)
            .for_each(|branch| branch.parent.branch = root);
        match (self.record.timeline.saved, saved, self.saved) {
            (Some(_), None, None) | (None, None, Some(_)) => self.swap_saved(root, old, current),
            (None, Some(_), None) => {
                self.record.timeline.saved = saved;
                self.swap_saved(old, root, current);
            }
            (None, None, None) => (),
            _ => unreachable!(),
        }
    }

    fn swap_saved(&mut self, old: usize, new: usize, current: usize) {
        debug_assert_ne!(old, new);
        if let Some(At { current: saved, .. }) = self
            .saved
            .filter(|at| at.branch == new && at.current <= current)
        {
            self.saved = None;
            self.record.timeline.saved = Some(saved);
            self.record.timeline.slot.emit(Signal::Saved(true));
        } else if let Some(saved) = self.record.timeline.saved {
            self.saved = Some(At::new(old, saved));
            self.record.timeline.saved = None;
            self.record.timeline.slot.emit(Signal::Saved(false));
        }
    }

    fn rm_child(&mut self, branch: usize, current: usize) {
        // We need to check if any of the branches had the removed node as root.
        let mut dead: Vec<_> = self
            .branches
            .iter()
            .filter(|&(_, child)| child.parent == At::new(branch, current))
            .map(|(&id, _)| id)
            .collect();
        while let Some(parent) = dead.pop() {
            // Remove the dead branch.
            self.branches.remove(&parent).unwrap();
            self.saved = self.saved.filter(|saved| saved.branch != parent);
            // Add the children of the dead branch so they are removed too.
            dead.extend(
                self.branches
                    .iter()
                    .filter(|&(_, child)| child.parent.branch == parent)
                    .map(|(&id, _)| id),
            )
        }
    }

    fn mk_path(&mut self, mut to: usize) -> Option<impl Iterator<Item = (usize, Branch<A>)>> {
        debug_assert_ne!(self.branch(), to);
        let mut dest = self.branches.remove(&to)?;
        let mut i = dest.parent.branch;
        let mut path = vec![(to, dest)];
        while i != self.branch() {
            dest = self.branches.remove(&i).unwrap();
            to = i;
            i = dest.parent.branch;
            path.push((to, dest));
        }
        Some(path.into_iter().rev())
    }
}

impl<A: Action<Output = ()>, S: Slot> History<A, S> {
    /// Repeatedly calls [`undo`] or [`redo`] until the action in `branch` at `current` is reached.
    ///
    /// [`undo`]: trait.Action.html#tymethod.undo
    /// [`redo`]: trait.Action.html#method.redo
    pub fn go_to(&mut self, target: &mut A::Target, branch: usize, current: usize) -> Option<()> {
        let root = self.root;
        if root == branch {
            return self.record.go_to(target, current);
        }
        // Walk the path from `root` to `branch`.
        for (new, branch) in self.mk_path(branch)? {
            // Walk to `branch.current` either by undoing or redoing.
            self.record.go_to(target, branch.parent.current).unwrap();
            // Apply the actions in the branch and move older actions into their own branch.
            for entry in branch.entries {
                let current = self.current();
                let saved = self.record.timeline.saved.filter(|&saved| saved > current);
                let (_, _, entries) = self.record.timeline.apply(target, entry.action);
                if !entries.deque.is_empty() {
                    self.branches
                        .insert(self.root, Branch::new(new, current, entries.deque));
                    self.set_root(new, current, saved);
                }
            }
        }
        self.record.go_to(target, current)
    }
}

impl<A: ToString, S> History<A, S> {
    /// Returns the string of the action which will be undone
    /// in the next call to [`undo`](struct.History.html#method.undo).
    pub fn undo_text(&self) -> Option<String> {
        self.record.undo_text()
    }

    /// Returns the string of the action which will be redone
    /// in the next call to [`redo`](struct.History.html#method.redo).
    pub fn redo_text(&self) -> Option<String> {
        self.record.redo_text()
    }
}

impl<A> Default for History<A> {
    fn default() -> History<A> {
        History::new()
    }
}

impl<A, S> From<Record<A, S>> for History<A, S> {
    fn from(record: Record<A, S>) -> Self {
        History {
            root: 0,
            next: 1,
            saved: None,
            record,
            branches: BTreeMap::new(),
        }
    }
}

/// A branch in the history.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub(crate) struct Branch<A> {
    pub(crate) parent: At,
    pub(crate) entries: VecDeque<Entry<A>>,
}

impl<A> Branch<A> {
    fn new(branch: usize, current: usize, entries: VecDeque<Entry<A>>) -> Branch<A> {
        Branch {
            parent: At::new(branch, current),
            entries,
        }
    }
}

/// Builder for a History.
///
/// # Examples
/// ```
/// # use undo::History;
/// # include!("../push.rs");
/// # fn main() {
/// let _ = History::builder()
///     .limit(100)
///     .capacity(100)
///     .connect(|s| { dbg!(s); })
///     .build::<Push>();
/// # }
/// ```
#[derive(Debug)]
pub struct Builder<F = NoOp>(RecordBuilder<F>);

impl<F> Builder<F> {
    /// Returns a builder for a history.
    pub fn new() -> Builder<F> {
        Builder(RecordBuilder::new())
    }

    /// Sets the capacity for the history.
    pub fn capacity(self, capacity: usize) -> Builder<F> {
        Builder(self.0.capacity(capacity))
    }

    /// Sets the `limit` for the history.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    pub fn limit(self, limit: usize) -> Builder<F> {
        Builder(self.0.limit(limit))
    }

    /// Sets if the target is initially in a saved state.
    /// By default the target is in a saved state.
    pub fn saved(self, saved: bool) -> Builder<F> {
        Builder(self.0.saved(saved))
    }

    /// Builds the history.
    pub fn build<A>(self) -> History<A, F> {
        History::from(self.0.build())
    }
}

impl<F: Slot> Builder<F> {
    /// Connects the slot.
    pub fn connect(self, f: F) -> Builder<F> {
        Builder(self.0.connect(f))
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
/// queue.commit(&mut string).unwrap();
/// assert_eq!(string, "abc");
/// # }
/// ```
#[derive(Debug)]
pub struct Queue<'a, A, S> {
    history: &'a mut History<A, S>,
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
    pub fn commit(self, target: &mut A::Target) -> Option<()> {
        for action in self.actions {
            match action {
                QueueAction::Apply(action) => self.history.apply(target, action),
                QueueAction::Undo => self.history.undo(target)?,
                QueueAction::Redo => self.history.redo(target)?,
            }
        }
        Some(())
    }

    /// Cancels the queued actions.
    pub fn cancel(self) {}

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<A, S> {
        self.history.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, S> {
        self.history.checkpoint()
    }
}

impl<'a, A, S> From<&'a mut History<A, S>> for Queue<'a, A, S> {
    fn from(history: &'a mut History<A, S>) -> Self {
        Queue {
            history,
            actions: Vec::new(),
        }
    }
}

#[derive(Debug)]
enum CheckpointAction {
    Apply(usize),
    Undo,
    Redo,
}

/// Wraps a history and gives it checkpoint functionality.
#[derive(Debug)]
pub struct Checkpoint<'a, A, S> {
    history: &'a mut History<A, S>,
    actions: Vec<CheckpointAction>,
}

impl<A: Action<Output = ()>, S: Slot> Checkpoint<'_, A, S> {
    /// Calls the `apply` method.
    pub fn apply(&mut self, target: &mut A::Target, action: A) {
        let branch = self.history.branch();
        self.history.apply(target, action);
        self.actions.push(CheckpointAction::Apply(branch));
    }

    /// Calls the `undo` method.
    pub fn undo(&mut self, target: &mut A::Target) -> Option<()> {
        self.history.undo(target)?;
        self.actions.push(CheckpointAction::Undo);
        Some(())
    }

    /// Calls the `redo` method.
    pub fn redo(&mut self, target: &mut A::Target) -> Option<()> {
        self.history.redo(target)?;
        self.actions.push(CheckpointAction::Redo);
        Some(())
    }

    /// Commits the changes and consumes the checkpoint.
    pub fn commit(self) {}

    /// Cancels the changes and consumes the checkpoint.
    pub fn cancel(self, target: &mut A::Target) -> Option<()> {
        for action in self.actions.into_iter().rev() {
            match action {
                CheckpointAction::Apply(branch) => {
                    let root = self.history.branch();
                    self.history.jump_to(branch);
                    if root == branch {
                        self.history.record.timeline.entries.deque.pop_back();
                    } else {
                        self.history.branches.remove(&root).unwrap();
                    }
                }
                CheckpointAction::Undo => self.history.redo(target)?,
                CheckpointAction::Redo => self.history.undo(target)?,
            };
        }
        Some(())
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<A, S> {
        self.history.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, S> {
        self.history.checkpoint()
    }
}

impl<'a, A, S> From<&'a mut History<A, S>> for Checkpoint<'a, A, S> {
    fn from(history: &'a mut History<A, S>) -> Self {
        Checkpoint {
            history,
            actions: Vec::new(),
        }
    }
}

/// Configurable display formatting for the history.
pub struct Display<'a, A, S> {
    history: &'a History<A, S>,
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
    fn fmt_list(
        &self,
        f: &mut fmt::Formatter,
        at: At,
        entry: Option<&Entry<A>>,
        level: usize,
    ) -> fmt::Result {
        self.format.mark(f, level)?;
        self.format.position(f, at, true)?;

        #[cfg(feature = "time")]
        if let Some(entry) = entry {
            if self.format.detailed {
                self.format.timestamp(f, &entry.timestamp)?;
            }
        }

        self.format.labels(
            f,
            at,
            At::new(self.history.branch(), self.history.current()),
            self.history
                .record
                .timeline
                .saved
                .map(|saved| At::new(self.history.branch(), saved))
                .or(self.history.saved),
        )?;
        if let Some(entry) = entry {
            if self.format.detailed {
                writeln!(f)?;
                self.format.message(f, entry, Some(level))?;
            } else {
                f.write_char(' ')?;
                self.format.message(f, entry, Some(level))?;
                writeln!(f)?;
            }
        }
        Ok(())
    }

    fn fmt_graph(
        &self,
        f: &mut fmt::Formatter,
        at: At,
        entry: Option<&Entry<A>>,
        level: usize,
    ) -> fmt::Result {
        for (&i, branch) in self
            .history
            .branches
            .iter()
            .filter(|(_, branch)| branch.parent == at)
        {
            for (j, entry) in branch.entries.iter().enumerate().rev() {
                let at = At::new(i, j + branch.parent.current + 1);
                self.fmt_graph(f, at, Some(entry), level + 1)?;
            }
            for j in 0..level {
                self.format.edge(f, j)?;
                f.write_char(' ')?;
            }
            self.format.split(f, level)?;
            writeln!(f)?;
        }
        for i in 0..level {
            self.format.edge(f, i)?;
            f.write_char(' ')?;
        }
        self.fmt_list(f, at, entry, level)
    }
}

impl<'a, A, S> From<&'a History<A, S>> for Display<'a, A, S> {
    fn from(history: &'a History<A, S>) -> Self {
        Display {
            history,
            format: Format::default(),
        }
    }
}

impl<A: fmt::Display, S> fmt::Display for Display<'_, A, S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let branch = self.history.branch();
        for (i, entry) in self
            .history
            .record
            .timeline
            .entries
            .deque
            .iter()
            .enumerate()
            .rev()
        {
            let at = At::new(branch, i + 1);
            self.fmt_graph(f, at, Some(entry), 0)?;
        }
        self.fmt_graph(f, At::new(branch, 0), None, 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use alloc::string::String;

    struct Push(char);

    impl Action for Push {
        type Target = String;
        type Output = ();

        fn apply(&mut self, s: &mut String) {
            s.push(self.0);
        }

        fn undo(&mut self, s: &mut String) {
            self.0 = s.pop().unwrap();
        }
    }

    #[test]
    fn go_to() {
        //          m
        //          |
        //    j  k  l
        //     \ | /
        //       i
        //       |
        // e  g  h
        // |  | /
        // d  f  p - q *
        // | /  /
        // c  n - o
        // | /
        // b
        // |
        // a
        let mut target = String::new();
        let mut history = History::new();
        history.apply(&mut target, Push('a'));
        history.apply(&mut target, Push('b'));
        history.apply(&mut target, Push('c'));
        history.apply(&mut target, Push('d'));
        history.apply(&mut target, Push('e'));
        assert_eq!(target, "abcde");
        history.undo(&mut target).unwrap();
        history.undo(&mut target).unwrap();
        assert_eq!(target, "abc");
        let abcde = history.branch();
        history.apply(&mut target, Push('f'));
        history.apply(&mut target, Push('g'));
        assert_eq!(target, "abcfg");
        history.undo(&mut target).unwrap();
        let abcfg = history.branch();
        history.apply(&mut target, Push('h'));
        history.apply(&mut target, Push('i'));
        history.apply(&mut target, Push('j'));
        assert_eq!(target, "abcfhij");
        history.undo(&mut target).unwrap();
        let abcfhij = history.branch();
        history.apply(&mut target, Push('k'));
        assert_eq!(target, "abcfhik");
        history.undo(&mut target).unwrap();
        let abcfhik = history.branch();
        history.apply(&mut target, Push('l'));
        assert_eq!(target, "abcfhil");
        history.apply(&mut target, Push('m'));
        assert_eq!(target, "abcfhilm");
        let abcfhilm = history.branch();
        history.go_to(&mut target, abcde, 2).unwrap();
        history.apply(&mut target, Push('n'));
        history.apply(&mut target, Push('o'));
        assert_eq!(target, "abno");
        history.undo(&mut target).unwrap();
        let abno = history.branch();
        history.apply(&mut target, Push('p'));
        history.apply(&mut target, Push('q'));
        assert_eq!(target, "abnpq");

        let abnpq = history.branch();
        history.go_to(&mut target, abcde, 5).unwrap();
        assert_eq!(target, "abcde");
        history.go_to(&mut target, abcfg, 5).unwrap();
        assert_eq!(target, "abcfg");
        history.go_to(&mut target, abcfhij, 7).unwrap();
        assert_eq!(target, "abcfhij");
        history.go_to(&mut target, abcfhik, 7).unwrap();
        assert_eq!(target, "abcfhik");
        history.go_to(&mut target, abcfhilm, 8).unwrap();
        assert_eq!(target, "abcfhilm");
        history.go_to(&mut target, abno, 4).unwrap();
        assert_eq!(target, "abno");
        history.go_to(&mut target, abnpq, 5).unwrap();
        assert_eq!(target, "abnpq");
    }
}
