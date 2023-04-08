//! A history tree of edits.

mod builder;
mod checkpoint;
mod display;
mod queue;

pub use builder::Builder;
pub use checkpoint::Checkpoint;
pub use display::Display;
pub use queue::Queue;

use crate::socket::{Nop, Signal, Slot};
use crate::{At, Edit, Entry, Record};
use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A history tree of edits.
///
/// Unlike [`Record`] which maintains a linear undo history,
/// [`History`] maintains an undo tree containing every edit made to the target.
///
/// # Examples
/// ```
/// # include!("doctest.rs");
/// # fn main() {
/// # use undo::History;
/// let mut target = String::new();
/// let mut history = History::new();
///
/// history.edit(&mut target, Push('a'));
/// history.edit(&mut target, Push('b'));
/// history.edit(&mut target, Push('c'));
/// let abc = history.branch();
///
/// history.go_to(&mut target, abc, 1);
/// history.edit(&mut target, Push('f'));
/// history.edit(&mut target, Push('g'));
/// assert_eq!(target, "afg");
///
/// history.go_to(&mut target, abc, 3);
/// assert_eq!(target, "abc");
/// # }
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct History<E, S = Nop> {
    root: usize,
    next: usize,
    saved: Option<At>,
    pub(crate) record: Record<E, S>,
    branches: BTreeMap<usize, Branch<E>>,
}

impl<E> History<E> {
    /// Returns a new history.
    pub fn new() -> History<E> {
        History::builder().build()
    }
}

impl<E, S> History<E, S> {
    /// Returns a new history builder.
    pub fn builder() -> Builder<E, S> {
        Builder::new()
    }

    /// Reserves capacity for at least `additional` more edits.
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

    /// Returns the number of edits in the current branch of the history.
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

    /// Returns the position of the current edit.
    pub fn current(&self) -> usize {
        self.record.current()
    }

    /// Returns a structure for configurable formatting of the history.
    pub fn display(&self) -> Display<E, S> {
        Display::from(self)
    }

    /// Returns an iterator over the edits in the current branch.
    pub fn edits(&self) -> impl Iterator<Item = &E> {
        self.record.edits()
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<E, S> {
        Queue::from(self)
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<E, S> {
        Checkpoint::from(self)
    }

    pub(crate) fn at(&self) -> At {
        At::new(self.root, self.current())
    }
}

impl<E: Edit, S: Slot> History<E, S> {
    /// Pushes the [`Edit`] to the top of the history and executes its [`Edit::edit`] method.
    pub fn edit(&mut self, target: &mut E::Target, edit: E) -> E::Output {
        let at = self.at();
        let saved = self.record.saved.filter(|&saved| saved > at.current);
        let (output, merged, tail) = self.record.edit_inner(target, edit);
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
        if !tail.is_empty() {
            let new = self.next;
            self.next += 1;
            self.branches
                .insert(at.branch, Branch::new(new, at.current, tail));
            self.set_root(new, at.current, saved);
        }
        output
    }

    /// Calls the [`Edit::undo`] method for the active edit
    /// and sets the previous one as the new active one.
    pub fn undo(&mut self, target: &mut E::Target) -> Option<E::Output> {
        self.record.undo(target)
    }

    /// Calls the [`Edit::redo`] method for the active edit
    /// and sets the next one as the new active one.
    pub fn redo(&mut self, target: &mut E::Target) -> Option<E::Output> {
        self.record.redo(target)
    }

    /// Marks the target as currently being in a saved or unsaved state.
    pub fn set_saved(&mut self, saved: bool) {
        self.saved = None;
        self.record.set_saved(saved);
    }

    /// Removes all edits from the history without undoing them.
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
        let saved = self.record.saved.filter(|&saved| saved > current);
        let tail = self.record.entries.split_off(current);
        self.record.entries.append(&mut branch.entries);
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
        match (self.record.saved, saved, self.saved) {
            (Some(_), None, None) | (None, None, Some(_)) => self.swap_saved(root, old, current),
            (None, Some(_), None) => {
                self.record.saved = saved;
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
            self.record.saved = Some(saved);
            self.record.socket.emit(Signal::Saved(true));
        } else if let Some(saved) = self.record.saved {
            self.saved = Some(At::new(old, saved));
            self.record.saved = None;
            self.record.socket.emit(Signal::Saved(false));
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

    fn mk_path(&mut self, mut to: usize) -> Option<impl Iterator<Item = (usize, Branch<E>)>> {
        debug_assert_ne!(self.branch(), to);
        let mut dest = self.branches.remove(&to)?;
        let mut i = dest.parent.branch;
        let mut path = alloc::vec![(to, dest)];
        while i != self.branch() {
            dest = self.branches.remove(&i).unwrap();
            to = i;
            i = dest.parent.branch;
            path.push((to, dest));
        }

        Some(path.into_iter().rev())
    }

    /// Repeatedly calls [`Edit::undo`] or [`Edit::redo`] until the edit in `branch` at `current` is reached.
    pub fn go_to(
        &mut self,
        target: &mut E::Target,
        branch: usize,
        current: usize,
    ) -> Vec<E::Output> {
        let root = self.root;
        if root == branch {
            return self.record.go_to(target, current);
        }

        // Walk the path from `root` to `branch`.
        let mut outputs = Vec::new();
        let  Some(path) = self.mk_path(branch) else {
            return Vec::new();
        };
        for (new, branch) in path {
            // Walk to `branch.current` either by undoing or redoing.
            let o = self.record.go_to(target, branch.parent.current);
            outputs.extend(o);
            // Apply the edits in the branch and move older edits into their own branch.
            for entry in branch.entries {
                let current = self.current();
                let saved = self.record.saved.filter(|&saved| saved > current);
                let (_, _, entries) = self.record.edit_inner(target, entry.edit);
                if !entries.is_empty() {
                    self.branches
                        .insert(self.root, Branch::new(new, current, entries));
                    self.set_root(new, current, saved);
                }
            }
        }
        let o = self.record.go_to(target, current);
        outputs.extend(o);
        outputs
    }
}

impl<E: ToString, S> History<E, S> {
    /// Returns the string of the edit which will be undone
    /// in the next call to [`History::undo`].
    pub fn undo_string(&self) -> Option<String> {
        self.record.undo_string()
    }

    /// Returns the string of the edit which will be redone
    /// in the next call to [`History::redo`].
    pub fn redo_string(&self) -> Option<String> {
        self.record.redo_string()
    }
}

impl<E> Default for History<E> {
    fn default() -> History<E> {
        History::new()
    }
}

impl<E, S> From<Record<E, S>> for History<E, S> {
    fn from(record: Record<E, S>) -> Self {
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
pub(crate) struct Branch<E> {
    pub(crate) parent: At,
    pub(crate) entries: VecDeque<Entry<E>>,
}

impl<E> Branch<E> {
    fn new(branch: usize, current: usize, entries: VecDeque<Entry<E>>) -> Branch<E> {
        Branch {
            parent: At::new(branch, current),
            entries,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    struct Push(char);

    impl Edit for Push {
        type Target = String;
        type Output = ();

        fn edit(&mut self, s: &mut String) {
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
        history.edit(&mut target, Push('a'));
        history.edit(&mut target, Push('b'));
        history.edit(&mut target, Push('c'));
        history.edit(&mut target, Push('d'));
        history.edit(&mut target, Push('e'));
        assert_eq!(target, "abcde");
        history.undo(&mut target).unwrap();
        history.undo(&mut target).unwrap();
        assert_eq!(target, "abc");
        let abcde = history.branch();
        history.edit(&mut target, Push('f'));
        history.edit(&mut target, Push('g'));
        assert_eq!(target, "abcfg");
        history.undo(&mut target).unwrap();
        let abcfg = history.branch();
        history.edit(&mut target, Push('h'));
        history.edit(&mut target, Push('i'));
        history.edit(&mut target, Push('j'));
        assert_eq!(target, "abcfhij");
        history.undo(&mut target).unwrap();
        let abcfhij = history.branch();
        history.edit(&mut target, Push('k'));
        assert_eq!(target, "abcfhik");
        history.undo(&mut target).unwrap();
        let abcfhik = history.branch();
        history.edit(&mut target, Push('l'));
        assert_eq!(target, "abcfhil");
        history.edit(&mut target, Push('m'));
        assert_eq!(target, "abcfhilm");
        let abcfhilm = history.branch();
        history.go_to(&mut target, abcde, 2);
        history.edit(&mut target, Push('n'));
        history.edit(&mut target, Push('o'));
        assert_eq!(target, "abno");
        history.undo(&mut target).unwrap();
        let abno = history.branch();
        history.edit(&mut target, Push('p'));
        history.edit(&mut target, Push('q'));
        assert_eq!(target, "abnpq");

        let abnpq = history.branch();
        history.go_to(&mut target, abcde, 5);
        assert_eq!(target, "abcde");
        history.go_to(&mut target, abcfg, 5);
        assert_eq!(target, "abcfg");
        history.go_to(&mut target, abcfhij, 7);
        assert_eq!(target, "abcfhij");
        history.go_to(&mut target, abcfhik, 7);
        assert_eq!(target, "abcfhik");
        history.go_to(&mut target, abcfhilm, 8);
        assert_eq!(target, "abcfhilm");
        history.go_to(&mut target, abno, 4);
        assert_eq!(target, "abno");
        history.go_to(&mut target, abnpq, 5);
        assert_eq!(target, "abnpq");
    }
}
