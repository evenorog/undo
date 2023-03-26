use super::Queue;
use crate::{Action, Entry, Record, Slot};
use std::collections::VecDeque;

#[derive(Debug)]
enum CheckpointEntry<A> {
    Apply(Option<usize>, VecDeque<Entry<A>>),
    Undo,
    Redo,
}

/// Wraps a record and gives it checkpoint functionality.
#[derive(Debug)]
pub struct Checkpoint<'a, A, S> {
    record: &'a mut Record<A, S>,
    entries: Vec<CheckpointEntry<A>>,
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
        self.entries.push(CheckpointEntry::Apply(saved, tail));
        output
    }

    /// Calls the `undo` method.
    pub fn undo(&mut self, target: &mut A::Target) -> Option<A::Output> {
        let output = self.record.undo(target)?;
        self.entries.push(CheckpointEntry::Undo);
        Some(output)
    }

    /// Calls the `redo` method.
    pub fn redo(&mut self, target: &mut A::Target) -> Option<A::Output> {
        let output = self.record.redo(target)?;
        self.entries.push(CheckpointEntry::Redo);
        Some(output)
    }

    /// Commits the changes and consumes the checkpoint.
    pub fn commit(self) {}

    /// Cancels the changes and consumes the checkpoint.
    pub fn cancel(self, target: &mut A::Target) -> Vec<A::Output> {
        self.entries
            .into_iter()
            .rev()
            .filter_map(|entry| match entry {
                CheckpointEntry::Apply(saved, mut entries) => {
                    let output = self.record.undo(target)?;
                    self.record.entries.pop_back();
                    self.record.entries.append(&mut entries);
                    self.record.saved = saved;
                    Some(output)
                }
                CheckpointEntry::Undo => self.record.redo(target),
                CheckpointEntry::Redo => self.record.undo(target),
            })
            .collect()
    }
}

impl<'a, A, S> From<&'a mut Record<A, S>> for Checkpoint<'a, A, S> {
    fn from(record: &'a mut Record<A, S>) -> Self {
        Checkpoint {
            record,
            entries: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{FromFn, Record};

    const A: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('a'));
    const B: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('b'));
    const C: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('c'));
    const D: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('d'));
    const E: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('e'));
    const F: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('f'));
    const G: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('g'));
    const H: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('h'));
    const I: FromFn<fn(&mut String), String> = FromFn::new(|s| s.push('i'));

    #[test]
    fn checkpoint_commit() {
        let mut target = String::new();
        let mut record = Record::new();
        let mut cp1 = record.checkpoint();
        cp1.apply(&mut target, A);
        cp1.apply(&mut target, B);
        cp1.apply(&mut target, C);
        assert_eq!(target, "abc");
        let mut cp2 = cp1.checkpoint();
        cp2.apply(&mut target, D);
        cp2.apply(&mut target, E);
        cp2.apply(&mut target, F);
        assert_eq!(target, "abcdef");
        let mut cp3 = cp2.checkpoint();
        cp3.apply(&mut target, G);
        cp3.apply(&mut target, H);
        cp3.apply(&mut target, I);
        assert_eq!(target, "abcdefghi");
        cp3.commit();
        cp2.commit();
        cp1.commit();
        assert_eq!(target, "abcdefghi");
    }

    #[test]
    fn checkpoint_cancel() {
        let mut target = String::new();
        let mut record = Record::new();
        let mut cp1 = record.checkpoint();
        cp1.apply(&mut target, A);
        cp1.apply(&mut target, B);
        cp1.apply(&mut target, C);
        let mut cp2 = cp1.checkpoint();
        cp2.apply(&mut target, D);
        cp2.apply(&mut target, E);
        cp2.apply(&mut target, F);
        let mut cp3 = cp2.checkpoint();
        cp3.apply(&mut target, G);
        cp3.apply(&mut target, H);
        cp3.apply(&mut target, I);
        assert_eq!(target, "abcdefghi");
        cp3.cancel(&mut target);
        assert_eq!(target, "abcdef");
        cp2.cancel(&mut target);
        assert_eq!(target, "abc");
        cp1.cancel(&mut target);
        assert_eq!(target, "");
    }

    #[test]
    fn checkpoint_saved() {
        let mut target = String::new();
        let mut record = Record::new();
        record.apply(&mut target, A);
        record.apply(&mut target, B);
        record.apply(&mut target, C);
        record.set_saved(true);
        record.undo(&mut target).unwrap();
        record.undo(&mut target).unwrap();
        record.undo(&mut target).unwrap();
        let mut cp = record.checkpoint();
        cp.apply(&mut target, D);
        cp.apply(&mut target, E);
        cp.apply(&mut target, F);
        assert_eq!(target, "def");
        cp.cancel(&mut target);
        assert_eq!(target, "");
        record.redo(&mut target).unwrap();
        record.redo(&mut target).unwrap();
        record.redo(&mut target).unwrap();
        assert!(record.is_saved());
        assert_eq!(target, "abc");
    }
}
