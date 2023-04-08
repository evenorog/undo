use super::Checkpoint;
use crate::{Edit, History, Slot};
use alloc::vec::Vec;

#[derive(Debug)]
enum QueueEntry<A> {
    Edit(A),
    Undo,
    Redo,
}

/// Wraps a history and gives it batch queue functionality.
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
/// queue.edit(Push('a'));
/// queue.edit(Push('b'));
/// queue.edit(Push('c'));
/// assert_eq!(string, "");
///
/// queue.commit(&mut string);
/// assert_eq!(string, "abc");
/// # }
/// ```
#[derive(Debug)]
pub struct Queue<'a, A, S> {
    history: &'a mut History<A, S>,
    entries: Vec<QueueEntry<A>>,
}

impl<A, S> Queue<'_, A, S> {
    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<A, S> {
        self.history.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, S> {
        self.history.checkpoint()
    }
}

impl<A: Edit, S: Slot> Queue<'_, A, S> {
    /// Queues a [`History::edit`] call.
    pub fn edit(&mut self, edit: A) {
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
    pub fn commit(self, target: &mut A::Target) -> Vec<A::Output> {
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

impl<'a, A, S> From<&'a mut History<A, S>> for Queue<'a, A, S> {
    fn from(history: &'a mut History<A, S>) -> Self {
        Queue {
            history,
            entries: Vec::new(),
        }
    }
}
