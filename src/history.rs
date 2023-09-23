//! A history tree of edit commands.

mod builder;
mod checkpoint;
mod display;
mod queue;

pub use builder::Builder;
pub use checkpoint::Checkpoint;
pub use display::Display;
pub use queue::Queue;

use crate::socket::{Event, Slot};
use crate::{At, Edit, Entry, Record};
use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A history tree of [`Edit`] commands.
///
/// Unlike [`Record`] which maintains a linear undo history,
/// [`History`] maintains an undo tree containing every edit made to the target.
///
/// See [this](https://github.com/evenorog/undo/blob/master/examples/history.rs)
/// example for an interactive view of the history tree.
///
/// # Examples
/// ```
/// # fn main() {
/// # use undo::{Add, History};
/// let mut target = String::new();
/// let mut history = History::new();
///
/// history.edit(&mut target, Add('a'));
/// history.edit(&mut target, Add('b'));
/// history.edit(&mut target, Add('c'));
/// let abc = history.head();
///
/// history.undo(&mut target);
/// history.undo(&mut target);
/// assert_eq!(target, "a");
///
/// // Instead of discarding 'b' and 'c', a new branch is created.
/// history.edit(&mut target, Add('f'));
/// history.edit(&mut target, Add('g'));
/// assert_eq!(target, "afg");
///
/// // We can now switch back to the original branch.
/// history.go_to(&mut target, abc);
/// assert_eq!(target, "abc");
/// # }
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct History<E, S = ()> {
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
        Builder::default()
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

    /// Sets how the event should be handled when the state changes.
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

    /// Returns the current position in the history.
    pub fn head(&self) -> At {
        At::new(self.root, self.record.index)
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
}

impl<E: Edit, S: Slot> History<E, S> {
    /// Pushes the [`Edit`] to the top of the history and executes its [`Edit::edit`] method.
    pub fn edit(&mut self, target: &mut E::Target, edit: E) -> E::Output {
        let head = self.head();
        let was_saved = self.record.saved.filter(|&saved| saved > head.index);
        let (output, merged, tail) = self.record.edit_and_push(target, edit.into());

        // Check if the limit has been reached.
        if !merged && head.index == self.record.index {
            let root = self.root;
            self.rm_child(root, 0);
            self.branches
                .values_mut()
                .filter(|branch| branch.parent.root == root)
                .for_each(|branch| branch.parent.index -= 1);
        }

        // Handle new branch.
        if !tail.is_empty() {
            let new = self.next;
            self.next += 1;
            let parent = At::new(new, head.index);
            self.branches.insert(head.root, Branch::new(parent, tail));
            self.set_root(parent, was_saved);
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
        debug_assert_eq!(branch.parent, self.head());
        let parent = At::new(root, self.record.index);
        let was_saved = self.record.saved.filter(|&saved| saved > parent.index);
        let tail = self.record.entries.split_off(parent.index);
        self.record.entries.append(&mut branch.entries);
        self.branches.insert(self.root, Branch::new(parent, tail));
        self.set_root(parent, was_saved);
    }

    fn set_root(&mut self, at: At, was_saved: Option<usize>) {
        let old = self.root;
        self.root = at.root;
        debug_assert_ne!(old, at.root);
        // Handle the child branches.
        self.branches
            .values_mut()
            .filter(|branch| branch.parent.root == old && branch.parent.index <= at.index)
            .for_each(|branch| branch.parent.root = at.root);

        match (self.record.saved, was_saved, self.saved) {
            (Some(_), None, None) | (None, None, Some(_)) => {
                self.swap_saved(at.root, old, at.index)
            }
            (None, Some(_), None) => {
                self.record.saved = was_saved;
                self.swap_saved(old, at.root, at.index);
            }
            _ => (),
        }
    }

    fn swap_saved(&mut self, old: usize, new: usize, index: usize) {
        debug_assert_ne!(old, new);
        let saved_in_new_root = self.saved.filter(|at| at.root == new && at.index <= index);
        if let Some(saved) = saved_in_new_root {
            self.saved = None;
            self.record.saved = Some(saved.index);
            self.record.socket.emit(|| Event::Saved(true));
        } else if let Some(saved) = self.record.saved {
            self.saved = Some(At::new(old, saved));
            self.record.saved = None;
            self.record.socket.emit(|| Event::Saved(false));
        }
    }

    fn rm_child(&mut self, branch: usize, index: usize) {
        // We need to check if any of the branches had the removed node as root.
        let mut dead: Vec<_> = self
            .branches
            .iter()
            .filter(|&(_, child)| child.parent == At::new(branch, index))
            .map(|(&id, _)| id)
            .collect();
        while let Some(parent) = dead.pop() {
            // Remove the dead branch.
            self.branches.remove(&parent).unwrap();
            self.saved = self.saved.filter(|saved| saved.root != parent);
            // Add the children of the dead branch so they are removed too.
            dead.extend(
                self.branches
                    .iter()
                    .filter(|&(_, child)| child.parent.root == parent)
                    .map(|(&id, _)| id),
            )
        }
    }

    fn mk_path(&mut self, mut to: usize) -> Option<impl Iterator<Item = (usize, Branch<E>)>> {
        debug_assert_ne!(self.root, to);
        let mut dest = self.branches.remove(&to)?;
        let mut i = dest.parent.root;
        let mut path = alloc::vec![(to, dest)];
        while i != self.root {
            dest = self.branches.remove(&i).unwrap();
            to = i;
            i = dest.parent.root;
            path.push((to, dest));
        }

        Some(path.into_iter().rev())
    }

    /// Repeatedly calls [`Edit::undo`] or [`Edit::redo`] until the edit in `branch` at `index` is reached.
    pub fn go_to(&mut self, target: &mut E::Target, at: At) -> Vec<E::Output> {
        let root = self.root;
        if root == at.root {
            return self.record.go_to(target, at.index);
        }

        // Walk the path from `root` to `branch`.
        let mut outputs = Vec::new();
        let Some(path) = self.mk_path(at.root) else {
            return Vec::new();
        };

        for (new, branch) in path {
            let mut outs = self.record.go_to(target, branch.parent.index);
            outputs.append(&mut outs);
            // Apply the edits in the branch and move older edits into their own branch.
            for entry in branch.entries {
                let index = self.record.index;
                let was_saved = self.record.saved.filter(|&saved| saved > index);
                let (_, _, entries) = self.record.redo_and_push(target, entry);
                if !entries.is_empty() {
                    let parent = At::new(new, index);
                    self.branches
                        .insert(self.root, Branch::new(parent, entries));
                    self.set_root(parent, was_saved);
                }
            }
        }
        let mut outs = self.record.go_to(target, at.index);
        outputs.append(&mut outs);
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
#[derive(Clone, Debug)]
pub(crate) struct Branch<E> {
    pub(crate) parent: At,
    pub(crate) entries: VecDeque<Entry<E>>,
}

impl<E> Branch<E> {
    fn new(parent: At, entries: VecDeque<Entry<E>>) -> Branch<E> {
        Branch { parent, entries }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

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
        history.edit(&mut target, Add('a'));
        history.edit(&mut target, Add('b'));
        history.edit(&mut target, Add('c'));
        history.edit(&mut target, Add('d'));
        history.edit(&mut target, Add('e'));
        assert_eq!(target, "abcde");
        history.undo(&mut target).unwrap();
        history.undo(&mut target).unwrap();
        assert_eq!(target, "abc");
        let abc = history.head();

        history.edit(&mut target, Add('f'));
        history.edit(&mut target, Add('g'));
        assert_eq!(target, "abcfg");
        let abcfg = history.head();

        history.undo(&mut target).unwrap();
        history.edit(&mut target, Add('h'));
        history.edit(&mut target, Add('i'));
        history.edit(&mut target, Add('j'));
        assert_eq!(target, "abcfhij");
        let abcfhij = history.head();

        history.undo(&mut target).unwrap();
        history.edit(&mut target, Add('k'));
        assert_eq!(target, "abcfhik");
        let abcfhik = history.head();

        history.undo(&mut target).unwrap();
        history.edit(&mut target, Add('l'));
        assert_eq!(target, "abcfhil");
        history.edit(&mut target, Add('m'));
        assert_eq!(target, "abcfhilm");
        let abcfhilm = history.head();
        history.go_to(&mut target, At::new(abc.root, 2));
        history.edit(&mut target, Add('n'));
        history.edit(&mut target, Add('o'));
        assert_eq!(target, "abno");
        let abno = history.head();

        history.undo(&mut target).unwrap();
        history.edit(&mut target, Add('p'));
        history.edit(&mut target, Add('q'));
        assert_eq!(target, "abnpq");

        let abnpq = history.head();
        history.go_to(&mut target, abc);
        assert_eq!(target, "abc");
        history.go_to(&mut target, abcfg);
        assert_eq!(target, "abcfg");
        history.go_to(&mut target, abcfhij);
        assert_eq!(target, "abcfhij");
        history.go_to(&mut target, abcfhik);
        assert_eq!(target, "abcfhik");
        history.go_to(&mut target, abcfhilm);
        assert_eq!(target, "abcfhilm");
        history.go_to(&mut target, abno);
        assert_eq!(target, "abno");
        history.go_to(&mut target, abnpq);
        assert_eq!(target, "abnpq");
    }
}
