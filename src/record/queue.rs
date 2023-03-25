use super::Checkpoint;
use crate::{Action, Record, Slot};

#[derive(Debug)]
enum QueueAction<A> {
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
/// queue.commit(&mut string).unwrap();
/// assert_eq!(string, "abc");
/// # }
/// ```
#[derive(Debug)]
pub struct Queue<'a, A, S> {
    record: &'a mut Record<A, S>,
    actions: Vec<QueueAction<A>>,
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
        self.actions.push(QueueAction::Apply(action));
    }

    /// Queues an `undo` action.
    pub fn undo(&mut self) {
        self.actions.push(QueueAction::Undo);
    }

    /// Queues a `redo` action.
    pub fn redo(&mut self) {
        self.actions.push(QueueAction::Redo);
    }

    /// Applies the queued actions.
    pub fn commit(self, target: &mut A::Target) -> Option<Vec<A::Output>> {
        let mut outputs = Vec::new();
        for action in self.actions {
            let output = match action {
                QueueAction::Apply(action) => self.record.apply(target, action),
                QueueAction::Undo => self.record.undo(target)?,
                QueueAction::Redo => self.record.redo(target)?,
            };
            outputs.push(output);
        }
        Some(outputs)
    }

    /// Cancels the queued actions.
    pub fn cancel(self) {}
}

impl<'a, A, S> From<&'a mut Record<A, S>> for Queue<'a, A, S> {
    fn from(record: &'a mut Record<A, S>) -> Self {
        Queue {
            record,
            actions: Vec::new(),
        }
    }
}
