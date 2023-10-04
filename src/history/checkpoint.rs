use crate::{Edit, History, Slot};
use alloc::vec::Vec;

#[derive(Debug)]
enum CheckpointEntry {
    Edit(usize),
    Undo,
    Redo,
}

/// Wraps a [`History`] and gives it checkpoint functionality.
#[derive(Debug)]
pub struct Checkpoint<'a, E, S> {
    history: &'a mut History<E, S>,
    entries: Vec<CheckpointEntry>,
}

impl<E, S> Checkpoint<'_, E, S> {
    /// Reserves capacity for at least `additional` more entries in the checkpoint.
    ///
    /// # Panics
    /// Panics if the new capacity exceeds `isize::MAX` bytes.
    pub fn reserve(&mut self, additional: usize) {
        self.entries.reserve(additional);
    }

    /// Commits the changes and consumes the checkpoint.
    pub fn commit(self) {}
}

impl<E: Edit, S: Slot> Checkpoint<'_, E, S> {
    /// Calls the [`History::edit`] method.
    pub fn edit(&mut self, target: &mut E::Target, edit: E) -> E::Output {
        let root = self.history.root;
        self.entries.push(CheckpointEntry::Edit(root));
        self.history.edit(target, edit)
    }

    /// Calls the [`History::undo`] method.
    pub fn undo(&mut self, target: &mut E::Target) -> Option<E::Output> {
        self.entries.push(CheckpointEntry::Undo);
        self.history.undo(target)
    }

    /// Calls the [`History::redo`] method.
    pub fn redo(&mut self, target: &mut E::Target) -> Option<E::Output> {
        self.entries.push(CheckpointEntry::Redo);
        self.history.redo(target)
    }

    /// Cancels the changes and consumes the checkpoint.
    pub fn cancel(self, target: &mut E::Target) -> Vec<E::Output> {
        self.entries
            .into_iter()
            .rev()
            .filter_map(|entry| match entry {
                CheckpointEntry::Edit(branch) => {
                    let output = self.history.undo(target)?;
                    let root = self.history.root;
                    if root == branch {
                        self.history.record.entries.pop_back();
                    } else {
                        self.history.jump_to(branch);
                        self.history.branches.remove(&root).unwrap();
                    }
                    Some(output)
                }
                CheckpointEntry::Undo => self.history.redo(target),
                CheckpointEntry::Redo => self.history.undo(target),
            })
            .collect()
    }
}

impl<'a, E, S> From<&'a mut History<E, S>> for Checkpoint<'a, E, S> {
    fn from(history: &'a mut History<E, S>) -> Self {
        Checkpoint {
            history,
            entries: Vec::new(),
        }
    }
}
