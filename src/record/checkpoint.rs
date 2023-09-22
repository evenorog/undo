use super::Queue;
use crate::{Edit, Entry, Record, Slot};
use alloc::collections::VecDeque;
use alloc::vec::Vec;

#[derive(Debug)]
enum CheckpointEntry<E> {
    Edit(Option<usize>, VecDeque<Entry<E>>),
    Undo,
    Redo,
}

/// Wraps a [`Record`] and gives it checkpoint functionality.
#[derive(Debug)]
pub struct Checkpoint<'a, E, S> {
    record: &'a mut Record<E, S>,
    entries: Vec<CheckpointEntry<E>>,
}

impl<E, S> Checkpoint<'_, E, S> {
    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<E, S> {
        self.record.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<E, S> {
        self.record.checkpoint()
    }
}

impl<E: Edit, S: Slot> Checkpoint<'_, E, S> {
    /// Calls the `apply` method.
    pub fn edit(&mut self, target: &mut E::Target, edit: E) -> E::Output {
        let saved = self.record.saved;
        let (output, _, tail) = self.record.edit_and_push(target, edit.into());
        self.entries.push(CheckpointEntry::Edit(saved, tail));
        output
    }

    /// Calls the `undo` method.
    pub fn undo(&mut self, target: &mut E::Target) -> Option<E::Output> {
        let output = self.record.undo(target)?;
        self.entries.push(CheckpointEntry::Undo);
        Some(output)
    }

    /// Calls the `redo` method.
    pub fn redo(&mut self, target: &mut E::Target) -> Option<E::Output> {
        let output = self.record.redo(target)?;
        self.entries.push(CheckpointEntry::Redo);
        Some(output)
    }

    /// Commits the changes and consumes the checkpoint.
    pub fn commit(self) {}

    /// Cancels the changes and consumes the checkpoint.
    pub fn cancel(self, target: &mut E::Target) -> Vec<E::Output> {
        self.entries
            .into_iter()
            .rev()
            .filter_map(|entry| match entry {
                CheckpointEntry::Edit(saved, mut entries) => {
                    let output = self.record.undo(target)?;
                    self.record.entries.pop_back();
                    self.record.entries.append(&mut entries);
                    self.record.saved = saved;
                    Some(output)
                }
                CheckpointEntry::Undo => self.record.redo(target),
                CheckpointEntry::Redo => self.record.undo(target),
            })
            .collect()
    }
}

impl<'a, E, S> From<&'a mut Record<E, S>> for Checkpoint<'a, E, S> {
    fn from(record: &'a mut Record<E, S>) -> Self {
        Checkpoint {
            record,
            entries: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{add::Add, Record};

    const A: Add = Add('a');
    const B: Add = Add('b');
    const C: Add = Add('c');
    const D: Add = Add('d');
    const E: Add = Add('e');
    const F: Add = Add('f');
    const G: Add = Add('g');
    const H: Add = Add('h');
    const I: Add = Add('i');

    #[test]
    fn checkpoint_commit() {
        let mut target = String::new();
        let mut record = Record::new();
        let mut cp1 = record.checkpoint();
        cp1.edit(&mut target, A);
        cp1.edit(&mut target, B);
        cp1.edit(&mut target, C);
        assert_eq!(target, "abc");
        let mut cp2 = cp1.checkpoint();
        cp2.edit(&mut target, D);
        cp2.edit(&mut target, E);
        cp2.edit(&mut target, F);
        assert_eq!(target, "abcdef");
        let mut cp3 = cp2.checkpoint();
        cp3.edit(&mut target, G);
        cp3.edit(&mut target, H);
        cp3.edit(&mut target, I);
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
        cp1.edit(&mut target, A);
        cp1.edit(&mut target, B);
        cp1.edit(&mut target, C);
        let mut cp2 = cp1.checkpoint();
        cp2.edit(&mut target, D);
        cp2.edit(&mut target, E);
        cp2.edit(&mut target, F);
        let mut cp3 = cp2.checkpoint();
        cp3.edit(&mut target, G);
        cp3.edit(&mut target, H);
        cp3.edit(&mut target, I);
        assert_eq!(target, "abcdefghi");
        cp3.cancel(&mut target);
        assert_eq!(target, "abcdef");
        cp2.cancel(&mut target);
        assert_eq!(target, "abc");
        cp1.cancel(&mut target);
        assert_eq!(target, "");
    }

    #[test]
    fn checkpoint_saved() {
        let mut target = String::new();
        let mut record = Record::new();
        record.edit(&mut target, A);
        record.edit(&mut target, B);
        record.edit(&mut target, C);
        record.set_saved(true);
        record.undo(&mut target).unwrap();
        record.undo(&mut target).unwrap();
        record.undo(&mut target).unwrap();
        let mut cp = record.checkpoint();
        cp.edit(&mut target, D);
        cp.edit(&mut target, E);
        cp.edit(&mut target, F);
        assert_eq!(target, "def");
        cp.cancel(&mut target);
        assert_eq!(target, "");
        record.redo(&mut target).unwrap();
        record.redo(&mut target).unwrap();
        record.redo(&mut target).unwrap();
        assert!(record.is_saved());
        assert_eq!(target, "abc");
    }
}
