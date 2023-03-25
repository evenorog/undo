use super::Queue;
use crate::{Action, Entry, Record, Slot};
use std::collections::VecDeque;

#[derive(Debug)]
enum CheckpointAction<A> {
    Apply(Option<usize>, VecDeque<Entry<A>>),
    Undo,
    Redo,
}

/// Wraps a record and gives it checkpoint functionality.
#[derive(Debug)]
pub struct Checkpoint<'a, A, S> {
    record: &'a mut Record<A, S>,
    actions: Vec<CheckpointAction<A>>,
}

impl<A: Action<Output = ()>, S: Slot> Checkpoint<'_, A, S> {
    /// Calls the `apply` method.
    pub fn apply(&mut self, target: &mut A::Target, action: A) {
        let saved = self.record.saved;
        let (_, _, tail) = self.record.__apply(target, action);
        self.actions.push(CheckpointAction::Apply(saved, tail));
    }

    /// Calls the `undo` method.
    pub fn undo(&mut self, target: &mut A::Target) -> Option<()> {
        self.record.undo(target)?;
        self.actions.push(CheckpointAction::Undo);
        Some(())
    }

    /// Calls the `redo` method.
    pub fn redo(&mut self, target: &mut A::Target) -> Option<()> {
        self.record.redo(target)?;
        self.actions.push(CheckpointAction::Redo);
        Some(())
    }

    /// Commits the changes and consumes the checkpoint.
    pub fn commit(self) {}

    /// Cancels the changes and consumes the checkpoint.
    pub fn cancel(self, target: &mut A::Target) -> Option<()> {
        for action in self.actions.into_iter().rev() {
            match action {
                CheckpointAction::Apply(saved, mut entries) => {
                    self.record.undo(target)?;
                    self.record.entries.pop_back();
                    self.record.entries.append(&mut entries);
                    self.record.saved = saved;
                }
                CheckpointAction::Undo => self.record.redo(target)?,
                CheckpointAction::Redo => self.record.undo(target)?,
            };
        }
        Some(())
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<A, S> {
        self.record.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, S> {
        self.record.checkpoint()
    }
}

impl<'a, A, S> From<&'a mut Record<A, S>> for Checkpoint<'a, A, S> {
    fn from(record: &'a mut Record<A, S>) -> Self {
        Checkpoint {
            record,
            actions: Vec::new(),
        }
    }
}
