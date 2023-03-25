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

impl<A: Action<Output = ()>, S: Slot> Checkpoint<'_, A, S> {
    /// Calls the `apply` method.
    pub fn apply(&mut self, target: &mut A::Target, action: A) {
        let branch = self.history.branch();
        self.history.apply(target, action);
        self.actions.push(CheckpointAction::Apply(branch));
    }

    /// Calls the `undo` method.
    pub fn undo(&mut self, target: &mut A::Target) -> Option<()> {
        self.history.undo(target)?;
        self.actions.push(CheckpointAction::Undo);
        Some(())
    }

    /// Calls the `redo` method.
    pub fn redo(&mut self, target: &mut A::Target) -> Option<()> {
        self.history.redo(target)?;
        self.actions.push(CheckpointAction::Redo);
        Some(())
    }

    /// Commits the changes and consumes the checkpoint.
    pub fn commit(self) {}

    /// Cancels the changes and consumes the checkpoint.
    pub fn cancel(self, target: &mut A::Target) -> Option<()> {
        for action in self.actions.into_iter().rev() {
            match action {
                CheckpointAction::Apply(branch) => {
                    let root = self.history.branch();
                    self.history.jump_to(branch);
                    if root == branch {
                        self.history.record.entries.pop_back();
                    } else {
                        self.history.branches.remove(&root).unwrap();
                    }
                }
                CheckpointAction::Undo => self.history.redo(target)?,
                CheckpointAction::Redo => self.history.undo(target)?,
            };
        }
        Some(())
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<A, S> {
        self.history.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, S> {
        self.history.checkpoint()
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
