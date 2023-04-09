use super::Checkpoint;
use crate::{Edit, History, Slot};
use alloc::vec::Vec;

#[derive(Debug)]
enum QueueEntry<E> {
    Edit(E),
    Undo,
    Redo,
}

/// Wraps a [`History`] and gives it batch queue functionality.
///
/// # Examples
/// ```
/// # include!("../doctest.rs");
/// # fn main() {
/// # use undo::History;
/// let mut string = String::new();
/// let mut record = History::new();
/// let mut queue = record.queue();
///
/// queue.edit(Add('a'));
/// queue.edit(Add('b'));
/// queue.edit(Add('c'));
/// assert_eq!(string, "");
///
/// queue.commit(&mut string);
/// assert_eq!(string, "abc");
/// # }
/// ```
#[derive(Debug)]
pub struct Queue<'a, E, S> {
    history: &'a mut History<E, S>,
    entries: Vec<QueueEntry<E>>,
}

impl<E, S> Queue<'_, E, S> {
    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<E, S> {
        self.history.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<E, S> {
        self.history.checkpoint()
    }
}

impl<E: Edit, S: Slot> Queue<'_, E, S> {
    /// Queues a [`History::edit`] call.
    pub fn edit(&mut self, edit: E) {
        self.entries.push(QueueEntry::Edit(edit));
    }

    /// Queues a [`History::undo`] call.
    pub fn undo(&mut self) {
        self.entries.push(QueueEntry::Undo);
    }

    /// Queues a [`History::redo`] call.
    pub fn redo(&mut self) {
        self.entries.push(QueueEntry::Redo);
    }

    /// Applies the queued edits.
    pub fn commit(self, target: &mut E::Target) -> Vec<E::Output> {
        self.entries
            .into_iter()
            .filter_map(|entry| match entry {
                QueueEntry::Edit(edit) => Some(self.history.edit(target, edit)),
                QueueEntry::Undo => self.history.undo(target),
                QueueEntry::Redo => self.history.redo(target),
            })
            .collect()
    }

    /// Cancels the queued edits.
    pub fn cancel(self) {}
}

impl<'a, E, S> From<&'a mut History<E, S>> for Queue<'a, E, S> {
    fn from(history: &'a mut History<E, S>) -> Self {
        Queue {
            history,
            entries: Vec::new(),
        }
    }
}
