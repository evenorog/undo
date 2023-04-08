use super::Checkpoint;
use crate::{Action, History, Slot};
use alloc::vec::Vec;

#[derive(Debug)]
enum QueueEntry<A> {
    Apply(A),
    Undo,
    Redo,
}

/// Wraps a record and gives it batch queue functionality.
///
/// # Examples
/// ```
/// # include!("../doctest.rs");
/// # fn main() {
/// # use undo::Record;
/// let mut string = String::new();
/// let mut record = Record::new();
/// let mut queue = record.queue();
///
/// queue.apply(Push('a'));
/// queue.apply(Push('b'));
/// queue.apply(Push('c'));
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

impl<A: Action, S: Slot> Queue<'_, A, S> {
    /// Queues an `apply` action.
    pub fn apply(&mut self, action: A) {
        self.entries.push(QueueEntry::Apply(action));
    }

    /// Queues an `undo` action.
    pub fn undo(&mut self) {
        self.entries.push(QueueEntry::Undo);
    }

    /// Queues a `redo` action.
    pub fn redo(&mut self) {
        self.entries.push(QueueEntry::Redo);
    }

    /// Applies the queued actions.
    pub fn commit(self, target: &mut A::Target) -> Vec<A::Output> {
        self.entries
            .into_iter()
            .filter_map(|entry| match entry {
                QueueEntry::Apply(action) => Some(self.history.apply(target, action)),
                QueueEntry::Undo => self.history.undo(target),
                QueueEntry::Redo => self.history.redo(target),
            })
            .collect()
    }

    /// Cancels the queued actions.
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
