use super::Queue;
use crate::{Action, History, Slot};

#[derive(Debug)]
enum CheckpointEntry {
    Apply(usize),
    Undo,
    Redo,
}

/// Wraps a history and gives it checkpoint functionality.
#[derive(Debug)]
pub struct Checkpoint<'a, A, S> {
    history: &'a mut History<A, S>,
    entries: Vec<CheckpointEntry>,
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
        self.entries.push(CheckpointEntry::Apply(branch));
        self.history.apply(target, action)
    }

    /// Calls the `undo` method.
    pub fn undo(&mut self, target: &mut A::Target) -> Option<A::Output> {
        self.entries.push(CheckpointEntry::Undo);
        self.history.undo(target)
    }

    /// Calls the `redo` method.
    pub fn redo(&mut self, target: &mut A::Target) -> Option<A::Output> {
        self.entries.push(CheckpointEntry::Redo);
        self.history.redo(target)
    }

    /// Commits the changes and consumes the checkpoint.
    pub fn commit(self) {}

    /// Cancels the changes and consumes the checkpoint.
    pub fn cancel(self, target: &mut A::Target) -> Vec<A::Output> {
        self.entries
            .into_iter()
            .rev()
            .filter_map(|entry| match entry {
                CheckpointEntry::Apply(branch) => {
                    let output = self.history.undo(target)?;
                    let root = self.history.branch();
                    if root == branch {
                        self.history.record.entries.pop_back();
                    } else {
                        self.history.jump_to(branch);
                        self.history.branches.remove(&root).unwrap();
                    }
                    Some(output)
                }
                CheckpointEntry::Undo => self.history.redo(target),
                CheckpointEntry::Redo => self.history.undo(target),
            })
            .collect()
    }
}

impl<'a, A, S> From<&'a mut History<A, S>> for Checkpoint<'a, A, S> {
    fn from(history: &'a mut History<A, S>) -> Self {
        Checkpoint {
            history,
            entries: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    const A: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('a'));
    const B: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('b'));
    const C: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('c'));
    const D: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('d'));
    const E: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('e'));

    #[test]
    fn checkpoint() {
        let mut target = String::new();
        let mut history = History::new();
        let mut checkpoint = history.checkpoint();

        checkpoint.apply(&mut target, A);
        checkpoint.apply(&mut target, B);
        checkpoint.apply(&mut target, C);
        assert_eq!(target, "abc");

        checkpoint.undo(&mut target);
        checkpoint.undo(&mut target);
        assert_eq!(target, "a");

        checkpoint.apply(&mut target, D);
        checkpoint.apply(&mut target, E);
        assert_eq!(target, "ade");

        checkpoint.cancel(&mut target);
        assert_eq!(target, "");
    }
}
