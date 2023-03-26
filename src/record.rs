//! A linear record of actions.

mod builder;
mod checkpoint;
mod display;
mod queue;

pub use builder::Builder;
pub use checkpoint::Checkpoint;
pub use display::Display;
pub use queue::Queue;

use crate::socket::{Nop, Slot, Socket};
use crate::{Action, Entry, History, Merged, Signal};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::time::SystemTime;

/// A linear record of actions.
///
/// The record can roll the targets state backwards and forwards by using
/// the undo and redo methods. In addition, the record can notify the user
/// about changes to the stack or the target through [`Signal`].
/// The user can give the record a function that is called each time the state
/// changes by using the [`Builder`].
///
/// # Examples
/// ```
/// # include!("doctest.rs");
/// # fn main() {
/// # use undo::Record;
/// let mut target = String::new();
/// let mut record = Record::new();
///
/// record.apply(&mut target, Push('a'));
/// record.apply(&mut target, Push('b'));
/// record.apply(&mut target, Push('c'));
/// assert_eq!(target, "abc");
///
/// record.undo(&mut target);
/// record.undo(&mut target);
/// record.undo(&mut target);
/// assert_eq!(target, "");
///
/// record.redo(&mut target);
/// record.redo(&mut target);
/// record.redo(&mut target);
/// assert_eq!(target, "abc");
/// # }
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct Record<A, S = Nop> {
    pub(crate) entries: VecDeque<Entry<A>>,
    pub(crate) limit: NonZeroUsize,
    pub(crate) current: usize,
    pub(crate) saved: Option<usize>,
    pub(crate) socket: Socket<S>,
}

impl<A> Record<A> {
    /// Returns a new record.
    pub fn new() -> Record<A> {
        Record::builder().build()
    }
}

impl<A, S> Record<A, S> {
    /// Returns a new record builder.
    pub fn builder() -> Builder<A, S> {
        Builder::new()
    }

    /// Reserves capacity for at least `additional` more actions.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    pub fn reserve(&mut self, additional: usize) {
        self.entries.reserve(additional);
    }

    /// Returns the capacity of the record.
    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }

    /// Shrinks the capacity of the record as much as possible.
    pub fn shrink_to_fit(&mut self) {
        self.entries.shrink_to_fit();
    }

    /// Returns the number of actions in the record.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the record is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the limit of the record.
    pub fn limit(&self) -> usize {
        self.limit.get()
    }

    /// Sets how the signal should be handled when the state changes.
    pub fn connect(&mut self, slot: S) -> Option<S> {
        self.socket.connect(Some(slot))
    }

    /// Removes and returns the slot if it exists.
    pub fn disconnect(&mut self) -> Option<S> {
        self.socket.disconnect()
    }

    /// Returns `true` if the record can undo.
    pub fn can_undo(&self) -> bool {
        self.current > 0
    }

    /// Returns `true` if the record can redo.
    pub fn can_redo(&self) -> bool {
        self.current < self.len()
    }

    /// Returns `true` if the target is in a saved state, `false` otherwise.
    pub fn is_saved(&self) -> bool {
        self.saved == Some(self.current)
    }

    /// Returns the position of the current action.
    pub fn current(&self) -> usize {
        self.current
    }

    /// Returns a structure for configurable formatting of the record.
    pub fn display(&self) -> Display<A, S> {
        Display::from(self)
    }

    /// Returns an iterator over the actions.
    pub fn actions(&self) -> impl Iterator<Item = &A> {
        self.entries.iter().map(|e| &e.action)
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<A, S> {
        Queue::from(self)
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<A, S> {
        Checkpoint::from(self)
    }
}

impl<A: Action, S: Slot> Record<A, S> {
    /// Pushes the action on top of the record and executes its [`Action::apply`] method.
    pub fn apply(&mut self, target: &mut A::Target, action: A) -> A::Output {
        let (output, _, _) = self.__apply(target, action);
        output
    }

    pub(crate) fn __apply(
        &mut self,
        target: &mut A::Target,
        mut action: A,
    ) -> (A::Output, bool, VecDeque<Entry<A>>) {
        let output = action.apply(target);
        // We store the state of the stack before adding the entry.
        let current = self.current;
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        // Pop off all elements after len from entries.
        let tail = self.entries.split_off(current);
        // Check if the saved state was popped off.
        self.saved = self.saved.filter(|&saved| saved <= current);
        // Try to merge actions unless the target is in a saved state.
        let merged = match self.entries.back_mut() {
            Some(last) if !was_saved => last.action.merge(action),
            _ => Merged::No(action),
        };

        let merged_or_annulled = match merged {
            Merged::Yes => true,
            Merged::Annul => {
                self.entries.pop_back();
                self.current -= 1;
                true
            }
            // If actions are not merged or annulled push it onto the storage.
            Merged::No(action) => {
                // If limit is reached, pop off the first action.
                if self.limit() == self.current {
                    self.entries.pop_front();
                    self.saved = self.saved.and_then(|saved| saved.checked_sub(1));
                } else {
                    self.current += 1;
                }
                self.entries.push_back(Entry::from(action));
                false
            }
        };

        self.socket.emit_if(could_redo, Signal::Redo(false));
        self.socket.emit_if(!could_undo, Signal::Undo(true));
        self.socket.emit_if(was_saved, Signal::Saved(false));
        (output, merged_or_annulled, tail)
    }

    /// Calls the [`Action::undo`] method for the active action and sets
    /// the previous one as the new active one.
    pub fn undo(&mut self, target: &mut A::Target) -> Option<A::Output> {
        self.can_undo().then(|| {
            let was_saved = self.is_saved();
            let old = self.current;
            let output = self.entries[self.current - 1].undo(target);
            self.current -= 1;
            let is_saved = self.is_saved();
            self.socket
                .emit_if(old == self.entries.len(), Signal::Redo(true));
            self.socket.emit_if(old == 1, Signal::Undo(false));
            self.socket
                .emit_if(was_saved != is_saved, Signal::Saved(is_saved));
            output
        })
    }

    /// Calls the [`Action::redo`] method for the active action and sets
    /// the next one as the new active one.
    pub fn redo(&mut self, target: &mut A::Target) -> Option<A::Output> {
        self.can_redo().then(|| {
            let was_saved = self.is_saved();
            let old = self.current;
            let output = self.entries[self.current].redo(target);
            self.current += 1;
            let is_saved = self.is_saved();
            self.socket
                .emit_if(old == self.len() - 1, Signal::Redo(false));
            self.socket.emit_if(old == 0, Signal::Undo(true));
            self.socket
                .emit_if(was_saved != is_saved, Signal::Saved(is_saved));
            output
        })
    }

    /// Marks the target as currently being in a saved or unsaved state.
    pub fn set_saved(&mut self, saved: bool) {
        let was_saved = self.is_saved();
        if saved {
            self.saved = Some(self.current);
            self.socket.emit_if(!was_saved, Signal::Saved(true));
        } else {
            self.saved = None;
            self.socket.emit_if(was_saved, Signal::Saved(false));
        }
    }

    /// Removes all actions from the record without undoing them.
    pub fn clear(&mut self) {
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        self.entries.clear();
        self.saved = self.is_saved().then_some(0);
        self.current = 0;
        self.socket.emit_if(could_undo, Signal::Undo(false));
        self.socket.emit_if(could_redo, Signal::Redo(false));
    }

    /// Revert the changes done to the target since the saved state.
    pub fn revert(&mut self, target: &mut A::Target) -> Option<Vec<A::Output>> {
        self.saved.and_then(|saved| self.go_to(target, saved))
    }

    /// Repeatedly calls [`Action::undo`] or [`Action::redo`] until the action at `current` is reached.
    pub fn go_to(&mut self, target: &mut A::Target, current: usize) -> Option<Vec<A::Output>> {
        if current > self.len() {
            return None;
        }

        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        // Temporarily remove slot so they are not called each iteration.
        let slot = self.socket.disconnect();
        // Decide if we need to undo or redo to reach current.
        let undo_or_redo = if current > self.current {
            Record::redo
        } else {
            Record::undo
        };

        let mut outputs = Vec::new();
        while self.current != current {
            let output = undo_or_redo(self, target)?;
            outputs.push(output);
        }

        let can_undo = self.can_undo();
        let can_redo = self.can_redo();
        let is_saved = self.is_saved();
        self.socket.connect(slot);
        self.socket
            .emit_if(could_undo != can_undo, Signal::Undo(can_undo));
        self.socket
            .emit_if(could_redo != can_redo, Signal::Redo(can_redo));
        self.socket
            .emit_if(was_saved != is_saved, Signal::Saved(is_saved));
        Some(outputs)
    }

    /// Go back or forward in the record to the action that was made closest to the system time provided.
    pub fn time_travel(
        &mut self,
        target: &mut A::Target,
        to: SystemTime,
    ) -> Option<Vec<A::Output>> {
        let current = self
            .entries
            .binary_search_by(|e| e.created_at.cmp(&to))
            .unwrap_or_else(std::convert::identity);
        self.go_to(target, current)
    }
}

impl<A: ToString, S> Record<A, S> {
    /// Returns the string of the action which will be undone
    /// in the next call to [`Record::undo`].
    pub fn undo_text(&self) -> Option<String> {
        self.current.checked_sub(1).and_then(|i| self.text(i))
    }

    /// Returns the string of the action which will be redone
    /// in the next call to [`Record::redo`].
    pub fn redo_text(&self) -> Option<String> {
        self.text(self.current)
    }

    fn text(&self, i: usize) -> Option<String> {
        self.entries.get(i).map(|e| e.action.to_string())
    }
}

impl<A> Default for Record<A> {
    fn default() -> Record<A> {
        Record::new()
    }
}

impl<A, F> From<History<A, F>> for Record<A, F> {
    fn from(history: History<A, F>) -> Record<A, F> {
        history.record
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    enum Edit {
        Push(Push),
        Pop(Pop),
    }

    impl Action for Edit {
        type Target = String;
        type Output = ();

        fn apply(&mut self, s: &mut String) {
            match self {
                Edit::Push(add) => add.apply(s),
                Edit::Pop(del) => del.apply(s),
            }
        }

        fn undo(&mut self, s: &mut String) {
            match self {
                Edit::Push(add) => add.undo(s),
                Edit::Pop(del) => del.undo(s),
            }
        }

        fn merge(&mut self, edit: Self) -> Merged<Self>
        where
            Self: Sized,
        {
            match (self, edit) {
                (Edit::Push(_), Edit::Pop(_)) => Merged::Annul,
                (Edit::Pop(Pop(Some(a))), Edit::Push(Push(b))) if a == &b => Merged::Annul,
                (_, edit) => Merged::No(edit),
            }
        }
    }

    #[derive(Debug, PartialEq)]
    struct Push(char);

    impl Action for Push {
        type Target = String;
        type Output = ();

        fn apply(&mut self, s: &mut String) {
            s.push(self.0);
        }

        fn undo(&mut self, s: &mut String) {
            self.0 = s.pop().unwrap();
        }
    }

    #[derive(Default)]
    struct Pop(Option<char>);

    impl Action for Pop {
        type Target = String;
        type Output = ();

        fn apply(&mut self, s: &mut String) {
            self.0 = s.pop();
        }

        fn undo(&mut self, s: &mut String) {
            let ch = self.0.unwrap();
            s.push(ch);
        }
    }

    #[test]
    fn go_to() {
        let mut target = String::new();
        let mut record = Record::new();
        record.apply(&mut target, Push('a'));
        record.apply(&mut target, Push('b'));
        record.apply(&mut target, Push('c'));
        record.apply(&mut target, Push('d'));
        record.apply(&mut target, Push('e'));

        record.go_to(&mut target, 0).unwrap();
        assert_eq!(record.current(), 0);
        assert_eq!(target, "");
        record.go_to(&mut target, 5).unwrap();
        assert_eq!(record.current(), 5);
        assert_eq!(target, "abcde");
        record.go_to(&mut target, 1).unwrap();
        assert_eq!(record.current(), 1);
        assert_eq!(target, "a");
        record.go_to(&mut target, 4).unwrap();
        assert_eq!(record.current(), 4);
        assert_eq!(target, "abcd");
        record.go_to(&mut target, 2).unwrap();
        assert_eq!(record.current(), 2);
        assert_eq!(target, "ab");
        record.go_to(&mut target, 3).unwrap();
        assert_eq!(record.current(), 3);
        assert_eq!(target, "abc");
        assert!(record.go_to(&mut target, 6).is_none());
        assert_eq!(record.current(), 3);
    }

    #[test]
    fn annul() {
        let mut target = String::new();
        let mut record = Record::new();
        record.apply(&mut target, Edit::Push(Push('a')));
        record.apply(&mut target, Edit::Pop(Pop::default()));
        record.apply(&mut target, Edit::Push(Push('b')));
        assert_eq!(record.len(), 1);
    }

    #[test]
    fn actions() {
        let mut target = String::new();
        let mut record = Record::new();
        record.apply(&mut target, Push('a'));
        record.apply(&mut target, Push('b'));
        let collected = record.actions().collect::<Vec<_>>();
        assert_eq!(&collected[..], &[&Push('a'), &Push('b')][..]);
    }
}
