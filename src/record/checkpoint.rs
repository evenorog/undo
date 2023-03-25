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

impl<A, S> Checkpoint<'_, A, S> {
    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<A, S> {
        self.record.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, S> {
        self.record.checkpoint()
    }
}

impl<A: Action, S: Slot> Checkpoint<'_, A, S> {
    /// Calls the `apply` method.
    pub fn apply(&mut self, target: &mut A::Target, action: A) -> A::Output {
        let saved = self.record.saved;
        let (output, _, tail) = self.record.__apply(target, action);
        self.actions.push(CheckpointAction::Apply(saved, tail));
        output
    }

    /// Calls the `undo` method.
    pub fn undo(&mut self, target: &mut A::Target) -> Option<A::Output> {
        let output = self.record.undo(target)?;
        self.actions.push(CheckpointAction::Undo);
        Some(output)
    }

    /// Calls the `redo` method.
    pub fn redo(&mut self, target: &mut A::Target) -> Option<A::Output> {
        let output = self.record.redo(target)?;
        self.actions.push(CheckpointAction::Redo);
        Some(output)
    }

    /// Commits the changes and consumes the checkpoint.
    pub fn commit(self) {}

    /// Cancels the changes and consumes the checkpoint.
    pub fn cancel(self, target: &mut A::Target) -> Option<Vec<A::Output>> {
        let mut outputs = Vec::new();
        for action in self.actions.into_iter().rev() {
            let output = match action {
                CheckpointAction::Apply(saved, mut entries) => {
                    let output = self.record.undo(target)?;
                    self.record.entries.pop_back();
                    self.record.entries.append(&mut entries);
                    self.record.saved = saved;
                    output
                }
                CheckpointAction::Undo => self.record.redo(target)?,
                CheckpointAction::Redo => self.record.undo(target)?,
            };
            outputs.push(output);
        }
        Some(outputs)
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
