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
/// # use undo::{Add, History};
/// let mut string = String::new();
/// let mut history = History::new();
/// let mut queue = history.queue();
///
/// queue.edit(Add('a'));
/// queue.edit(Add('b'));
/// queue.edit(Add('c'));
/// assert_eq!(string, "");
///
/// queue.commit(&mut string);
/// assert_eq!(string, "abc");
/// ```
#[derive(Debug)]
pub struct Queue<'a, E, S> {
    history: &'a mut History<E, S>,
    entries: Vec<QueueEntry<E>>,
}

impl<E, S> Queue<'_, E, S> {
    /// Reserves capacity for at least `additional` more entries in the queue.
    ///
    /// # Panics
    /// Panics if the new capacity exceeds `isize::MAX` bytes.
    pub fn reserve(&mut self, additional: usize) {
        self.entries.reserve(additional);
    }

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

    /// Cancels the queued edits.
    pub fn cancel(self) {}
}

impl<E: Edit, S: Slot> Queue<'_, E, S> {
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
}

impl<'a, E, S> From<&'a mut History<E, S>> for Queue<'a, E, S> {
    fn from(history: &'a mut History<E, S>) -> Self {
        Queue {
            history,
            entries: Vec::new(),
        }
    }
}
