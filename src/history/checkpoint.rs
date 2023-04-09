use super::Queue;
use crate::{Edit, History, Slot};
use alloc::vec::Vec;

#[derive(Debug)]
enum CheckpointEntry {
    Edit(usize),
    Undo,
    Redo,
}

/// Wraps a history and gives it checkpoint functionality.
#[derive(Debug)]
pub struct Checkpoint<'a, E, S> {
    history: &'a mut History<E, S>,
    entries: Vec<CheckpointEntry>,
}

impl<E, S> Checkpoint<'_, E, S> {
    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<E, S> {
        self.history.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<E, S> {
        self.history.checkpoint()
    }
}

impl<E: Edit, S: Slot> Checkpoint<'_, E, S> {
    /// Calls the [`History::edit`] method.
    pub fn edit(&mut self, target: &mut E::Target, edit: E) -> E::Output {
        let branch = self.history.branch();
        self.entries.push(CheckpointEntry::Edit(branch));
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

    /// Commits the changes and consumes the checkpoint.
    pub fn commit(self) {}

    /// Cancels the changes and consumes the checkpoint.
    pub fn cancel(self, target: &mut E::Target) -> Vec<E::Output> {
        self.entries
            .into_iter()
            .rev()
            .filter_map(|entry| match entry {
                CheckpointEntry::Edit(branch) => {
                    let output = self.history.undo(target)?;
                    let root = self.history.branch();
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

#[cfg(test)]
mod tests {
    use crate::*;

    const A: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('a'));
    const B: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('b'));
    const C: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('c'));
    const D: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('d'));
    const E: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('e'));

    #[test]
    fn checkpoint() {
        let mut target = String::new();
        let mut history = History::new();
        let mut checkpoint = history.checkpoint();

        checkpoint.edit(&mut target, A);
        checkpoint.edit(&mut target, B);
        checkpoint.edit(&mut target, C);
        assert_eq!(target, "abc");

        checkpoint.undo(&mut target);
        checkpoint.undo(&mut target);
        assert_eq!(target, "a");

        checkpoint.edit(&mut target, D);
        checkpoint.edit(&mut target, E);
        assert_eq!(target, "ade");

        checkpoint.cancel(&mut target);
        assert_eq!(target, "");
    }
}
