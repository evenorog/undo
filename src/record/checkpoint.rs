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
