use super::Checkpoint;
use crate::{Action, Record, Slot};
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
    record: &'a mut Record<A, S>,
    entries: Vec<QueueEntry<A>>,
}

impl<A, S> Queue<'_, A, S> {
    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<A, S> {
        self.record.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, S> {
        self.record.checkpoint()
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
                QueueEntry::Apply(action) => Some(self.record.apply(target, action)),
                QueueEntry::Undo => self.record.undo(target),
                QueueEntry::Redo => self.record.redo(target),
            })
            .collect()
    }

    /// Cancels the queued actions.
    pub fn cancel(self) {}
}

impl<'a, A, S> From<&'a mut Record<A, S>> for Queue<'a, A, S> {
    fn from(record: &'a mut Record<A, S>) -> Self {
        Queue {
            record,
            entries: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{FromFn, Record};
    use alloc::string::String;

    const A: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('a'));
    const B: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('b'));
    const C: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('c'));

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
        q3.apply(A);
        q3.apply(B);
        q3.apply(C);
        assert_eq!(target, "");
        q3.commit(&mut target);
        assert_eq!(target, "abc");
        q2.commit(&mut target);
        assert_eq!(target, "");
        q1.commit(&mut target);
        assert_eq!(target, "abc");
    }
}
