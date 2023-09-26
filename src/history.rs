//! A history tree of edit commands.

mod builder;
mod checkpoint;
mod display;
mod queue;

pub use builder::Builder;
pub use checkpoint::Checkpoint;
pub use display::Display;
pub use queue::Queue;

use crate::socket::Slot;
use crate::{At, Edit, Entry, Event, Record};
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
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct History<E, S = ()> {
    root: usize,
    next: usize,
    saved: Option<At>,
    record: Record<E, S>,
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

    /// Return the position of the saved state.
    pub fn saved(&self) -> Option<At> {
        self.record
            .saved
            .map(|index| At::new(self.root, index))
            .or(self.saved)
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

    /// Returns the edit at the index in the current root branch.
    ///
    /// Use [History::get_branch] if you want to get edits from other branches.
    pub fn get_edit(&self, index: usize) -> Option<&E> {
        self.record.get_edit(index)
    }

    /// Returns an iterator over the edits in the current root branch.
    pub fn edits(&self) -> impl Iterator<Item = &E> {
        self.record.edits()
    }

    /// Returns the branch with the given id.
    pub fn get_branch(&self, id: usize) -> Option<&Branch<E>> {
        self.branches.get(&id)
    }

    /// Returns an iterator over the branches in the history.
    ///
    /// This does not include the current root branch.
    pub fn branches(&self) -> impl Iterator<Item = (&usize, &Branch<E>)> {
        self.branches.iter()
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<E, S> {
        Queue::from(self)
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<E, S> {
        Checkpoint::from(self)
    }

    fn rm_child_of(&mut self, at: At) {
        // We need to check if any of the branches had the removed node as root.
        let mut dead: Vec<_> = self
            .branches
            .iter()
            .filter(|&(_, child)| child.parent == at)
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
}

impl<E, S: Slot> History<E, S> {
    /// Marks the target as currently being in a saved or unsaved state.
    pub fn set_saved(&mut self, saved: bool) {
        self.saved = None;
        self.record.set_saved(saved);
    }

    /// Removes all edits from the history without undoing them.
    pub fn clear(&mut self) {
        let old_root = self.root;
        self.root = 0;
        self.next = 1;
        self.saved = None;
        self.record.clear();
        self.branches.clear();
        self.record.socket.emit_if(old_root != 0, || Event::Root(0));
    }

    fn set_root(&mut self, new: At, rm_saved: Option<usize>) {
        debug_assert_ne!(self.root, new.root);

        // Update all branches that are now children of the new root.
        //
        // |           | <- The old root is now a child of the new root.
        // | |         | | <- This branch is still a child of the old root.
        // |/          |/
        // |    ->    /
        // o |       o | <- This branch is now a child of the new root.
        // |/        |/
        // |         |
        //
        // If we split at 'o' all branches that are children of 'o' or below should now
        // be children of the new root. All branches that are above should still be
        // children of the old root.
        self.branches
            .values_mut()
            .filter(|child| child.parent.root == self.root && child.parent.index <= new.index)
            .for_each(|child| child.parent.root = new.root);

        match (self.saved, rm_saved) {
            (Some(saved), None) if saved.root == new.root => {
                self.saved = None;
                self.record.saved = Some(saved.index);
            }
            (None, Some(saved)) => {
                self.saved = Some(At::new(self.root, saved));
            }
            _ => (),
        }

        debug_assert_ne!(self.saved.map(|s| s.root), Some(new.root));

        self.root = new.root;
        self.record.socket.emit(|| Event::Root(new.root));
    }

    fn jump_to(&mut self, root: usize) {
        let mut branch = self.branches.remove(&root).unwrap();
        debug_assert_eq!(branch.parent, self.head());

        let parent = At::new(root, self.record.index);
        let (tail, rm_saved) = self.record.rm_tail();
        self.record.entries.append(&mut branch.entries);
        self.branches.insert(self.root, Branch::new(parent, tail));
        self.set_root(parent, rm_saved);
    }
}

impl<E: Edit, S: Slot> History<E, S> {
    /// Pushes the [`Edit`] to the top of the history and executes its [`Edit::edit`] method.
    pub fn edit(&mut self, target: &mut E::Target, edit: E) -> E::Output {
        let head = self.head();
        let (output, merged, tail, rm_saved) = self.record.edit_and_push(target, edit.into());

        // Check if the limit has been reached.
        if !merged && head.index == self.record.index {
            let root = self.root;
            self.rm_child_of(At::new(root, 0));
            self.branches
                .values_mut()
                .filter(|child| child.parent.root == root)
                .for_each(|child| child.parent.index -= 1);
        }

        // Handle new branch.
        if !tail.is_empty() {
            let new = self.next;
            self.next += 1;
            let parent = At::new(new, head.index);
            self.branches.insert(head.root, Branch::new(parent, tail));
            self.set_root(parent, rm_saved);
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

    /// Repeatedly calls [`Edit::undo`] or [`Edit::redo`] until the edit at `at` is reached.
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
                let (_, _, entries, rm_saved) = self.record.redo_and_push(target, entry);
                if !entries.is_empty() {
                    let parent = At::new(new, index);
                    self.branches
                        .insert(self.root, Branch::new(parent, entries));
                    self.set_root(parent, rm_saved);
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

impl<E, F> From<History<E, F>> for Record<E, F> {
    fn from(history: History<E, F>) -> Record<E, F> {
        history.record
    }
}

/// A branch in the history.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct Branch<E> {
    parent: At,
    entries: VecDeque<Entry<E>>,
}

impl<E> Branch<E> {
    const fn new(parent: At, entries: VecDeque<Entry<E>>) -> Branch<E> {
        Branch { parent, entries }
    }

    /// Returns the parent edit of the branch.
    pub fn parent(&self) -> At {
        self.parent
    }

    /// Returns the edit at the index.
    pub fn get_edit(&self, index: usize) -> Option<&E> {
        self.entries.get(index).map(|e| &e.edit)
    }

    /// Returns an iterator over the edits in the branch.
    pub fn edits(&self) -> impl Iterator<Item = &E> {
        self.entries.iter().map(|e| &e.edit)
    }
}
