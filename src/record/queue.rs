use super::Checkpoint;
use crate::{Edit, Record, Slot};
use alloc::vec::Vec;

#[derive(Debug)]
enum QueueEntry<E> {
    Edit(E),
    Undo,
    Redo,
}

/// Wraps a [`Record`] and gives it batch queue functionality.
///
/// # Examples
/// ```
/// # fn main() {
/// # use undo::{Add, Record};
/// let mut string = String::new();
/// let mut record = Record::new();
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
    record: &'a mut Record<E, S>,
    entries: Vec<QueueEntry<E>>,
}

impl<E, S> Queue<'_, E, S> {
    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<E, S> {
        self.record.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<E, S> {
        self.record.checkpoint()
    }
}

impl<E: Edit, S: Slot> Queue<'_, E, S> {
    /// Queues a [`Record::edit`] call.
    pub fn edit(&mut self, edit: E) {
        self.entries.push(QueueEntry::Edit(edit));
    }

    /// Queues a [`Record::undo`] call.
    pub fn undo(&mut self) {
        self.entries.push(QueueEntry::Undo);
    }

    /// Queues a [`Record::redo`] call.
    pub fn redo(&mut self) {
        self.entries.push(QueueEntry::Redo);
    }

    /// Applies the queued edits.
    pub fn commit(self, target: &mut E::Target) -> Vec<E::Output> {
        self.entries
            .into_iter()
            .filter_map(|entry| match entry {
                QueueEntry::Edit(edit) => Some(self.record.edit(target, edit)),
                QueueEntry::Undo => self.record.undo(target),
                QueueEntry::Redo => self.record.redo(target),
            })
            .collect()
    }

    /// Cancels the queued edits.
    pub fn cancel(self) {}
}

impl<'a, E, S> From<&'a mut Record<E, S>> for Queue<'a, E, S> {
    fn from(record: &'a mut Record<E, S>) -> Self {
        Queue {
            record,
            entries: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Add, Record};
    use alloc::string::String;

    const A: Add = Add('a');
    const B: Add = Add('b');
    const C: Add = Add('c');

    #[test]
    fn queue_commit() {
        let mut target = String::new();
        let mut record = Record::new();
        let mut q1 = record.queue();
        q1.redo();
        q1.redo();
        q1.redo();
        let mut q2 = q1.queue();
        q2.undo();
        q2.undo();
        q2.undo();
        let mut q3 = q2.queue();
        q3.edit(A);
        q3.edit(B);
        q3.edit(C);
        assert_eq!(target, "");
        q3.commit(&mut target);
        assert_eq!(target, "abc");
        q2.commit(&mut target);
        assert_eq!(target, "");
        q1.commit(&mut target);
        assert_eq!(target, "abc");
    }
}
