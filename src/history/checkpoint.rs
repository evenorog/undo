use super::Queue;
use crate::{Action, History, Slot};

#[derive(Debug)]
enum CheckpointAction {
    Apply(usize),
    Undo,
    Redo,
}

/// Wraps a history and gives it checkpoint functionality.
#[derive(Debug)]
pub struct Checkpoint<'a, A, S> {
    history: &'a mut History<A, S>,
    actions: Vec<CheckpointAction>,
}

impl<A, S> Checkpoint<'_, A, S> {
    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<A, S> {
        self.history.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, S> {
        self.history.checkpoint()
    }
}

impl<A: Action, S: Slot> Checkpoint<'_, A, S> {
    /// Calls the `apply` method.
    pub fn apply(&mut self, target: &mut A::Target, action: A) -> A::Output {
        let branch = self.history.branch();
        self.actions.push(CheckpointAction::Apply(branch));
        self.history.apply(target, action)
    }

    /// Calls the `undo` method.
    pub fn undo(&mut self, target: &mut A::Target) -> Option<A::Output> {
        self.actions.push(CheckpointAction::Undo);
        self.history.undo(target)
    }

    /// Calls the `redo` method.
    pub fn redo(&mut self, target: &mut A::Target) -> Option<A::Output> {
        self.actions.push(CheckpointAction::Redo);
        self.history.redo(target)
    }

    /// Commits the changes and consumes the checkpoint.
    pub fn commit(self) {}

    /// Cancels the changes and consumes the checkpoint.
    pub fn cancel(self, target: &mut A::Target) -> Option<Vec<A::Output>> {
        let mut outputs = Vec::new();
        for action in self.actions.into_iter().rev() {
            let output = match action {
                CheckpointAction::Apply(branch) => {
                    let output = self.history.undo(target)?;
                    let root = self.history.branch();
                    self.history.jump_to(branch);
                    if root == branch {
                        self.history.record.entries.pop_back();
                    } else {
                        self.history.branches.remove(&root).unwrap();
                    }
                    output
                }
                CheckpointAction::Undo => self.history.redo(target)?,
                CheckpointAction::Redo => self.history.undo(target)?,
            };
            outputs.push(output);
        }
        Some(outputs)
    }
}

impl<'a, A, S> From<&'a mut History<A, S>> for Checkpoint<'a, A, S> {
    fn from(history: &'a mut History<A, S>) -> Self {
        Checkpoint {
            history,
            actions: Vec::new(),
        }
    }
}
