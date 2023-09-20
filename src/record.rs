//! A linear record of edit commands.

mod builder;
mod checkpoint;
mod display;
mod queue;

pub use builder::Builder;
pub use checkpoint::Checkpoint;
pub use display::Display;
pub use queue::Queue;

use crate::socket::{Slot, Socket};
use crate::{Edit, Entry, Event, History, Merged};
use alloc::collections::VecDeque;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::num::NonZeroUsize;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A linear record of [`Edit`] commands.
///
/// The record can roll the targets state backwards and forwards by using
/// the undo and redo methods. In addition, the record can notify the user
/// about changes to the stack or the target through [`Event`].
/// The user can give the record a function that is called each time the state
/// changes by using the [`Builder`].
///
/// When adding a new edit command to the record the previously undone commands
/// will be discarded. If you want to keep all edits you can use [`History`] instead.
///
/// # Examples
/// ```
/// # fn main() {
/// # use undo::{Add, Record};
/// let mut target = String::new();
/// let mut record = Record::new();
///
/// record.edit(&mut target, Add('a'));
/// record.edit(&mut target, Add('b'));
/// record.edit(&mut target, Add('c'));
/// assert_eq!(target, "abc");
///
/// record.undo(&mut target);
/// record.undo(&mut target);
/// record.undo(&mut target);
/// assert_eq!(target, "");
///
/// record.redo(&mut target);
/// record.redo(&mut target);
/// assert_eq!(target, "ab");
///
/// // 'c' will be discarded.
/// record.edit(&mut target, Add('d'));
/// assert_eq!(target, "abd");
/// # }
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct Record<E, S = ()> {
    pub(crate) limit: NonZeroUsize,
    pub(crate) index: usize,
    pub(crate) saved: Option<usize>,
    pub(crate) socket: Socket<S>,
    pub(crate) entries: VecDeque<Entry<E>>,
}

impl<E> Record<E> {
    /// Returns a new record.
    pub fn new() -> Record<E> {
        Record::builder().build()
    }
}

impl<E, S> Record<E, S> {
    /// Returns a new record builder.
    pub fn builder() -> Builder<E, S> {
        Builder::default()
    }

    /// Reserves capacity for at least `additional` more edits.
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

    /// Returns the number of edits in the record.
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

    /// Sets how the event should be handled when the state changes.
    pub fn connect(&mut self, slot: S) -> Option<S> {
        self.socket.connect(Some(slot))
    }

    /// Removes and returns the slot if it exists.
    pub fn disconnect(&mut self) -> Option<S> {
        self.socket.disconnect()
    }

    /// Returns `true` if the record can undo.
    pub fn can_undo(&self) -> bool {
        self.index > 0
    }

    /// Returns `true` if the record can redo.
    pub fn can_redo(&self) -> bool {
        self.index < self.len()
    }

    /// Returns `true` if the target is in a saved state, `false` otherwise.
    pub fn is_saved(&self) -> bool {
        self.saved == Some(self.index)
    }

    /// Returns the index of the next edit.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Returns a structure for configurable formatting of the record.
    pub fn display(&self) -> Display<E, S> {
        Display::from(self)
    }

    /// Returns an iterator over the edits.
    pub fn edits(&self) -> impl Iterator<Item = &E> {
        self.entries.iter().map(|e| &e.edit)
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<E, S> {
        Queue::from(self)
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<E, S> {
        Checkpoint::from(self)
    }

    /// Remove all elements after the index.
    fn rm_tail(&mut self) -> VecDeque<Entry<E>> {
        // Check if the saved edit will be removed.
        self.saved = self.saved.filter(|&saved| saved <= self.index);
        self.entries.split_off(self.index)
    }
}

impl<E: Edit, S: Slot> Record<E, S> {
    /// Pushes the edit on top of the record and executes its [`Edit::edit`] method.
    pub fn edit(&mut self, target: &mut E::Target, edit: E) -> E::Output {
        let (output, _, _) = self.edit_and_push(target, edit.into());
        output
    }

    pub(crate) fn edit_and_push(
        &mut self,
        target: &mut E::Target,
        mut entry: Entry<E>,
    ) -> (E::Output, bool, VecDeque<Entry<E>>) {
        let output = entry.edit(target);
        let (merged_or_annulled, tail) = self.push(entry);
        (output, merged_or_annulled, tail)
    }

    pub(crate) fn redo_and_push(
        &mut self,
        target: &mut E::Target,
        mut entry: Entry<E>,
    ) -> (E::Output, bool, VecDeque<Entry<E>>) {
        let output = entry.redo(target);
        let (merged_or_annulled, tail) = self.push(entry);
        (output, merged_or_annulled, tail)
    }

    fn push(&mut self, entry: Entry<E>) -> (bool, VecDeque<Entry<E>>) {
        let old_index = self.index;
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();

        let tail = self.rm_tail();
        // Try to merge unless the target is in a saved state.
        let merged = match self.entries.back_mut() {
            Some(last) if !was_saved => last.merge(entry),
            _ => Merged::No(entry),
        };

        let merged_or_annulled = match merged {
            Merged::Yes => true,
            Merged::Annul => {
                self.entries.pop_back();
                self.index -= 1;
                true
            }
            Merged::No(entry) => {
                // If limit is reached, pop off the first edit command.
                if self.limit() == self.index {
                    self.entries.pop_front();
                    self.saved = self.saved.and_then(|saved| saved.checked_sub(1));
                } else {
                    self.index += 1;
                }
                self.entries.push_back(entry);
                false
            }
        };

        self.socket.emit_if(could_redo, || Event::Redo(false));
        self.socket.emit_if(!could_undo, || Event::Undo(true));
        self.socket.emit_if(was_saved, || Event::Saved(false));
        self.socket
            .emit_if(old_index != self.index, || Event::Index(self.index));
        (merged_or_annulled, tail)
    }

    /// Calls the [`Edit::undo`] method for the active edit and sets
    /// the previous one as the new active one.
    pub fn undo(&mut self, target: &mut E::Target) -> Option<E::Output> {
        self.can_undo().then(|| {
            let old_index = self.index;
            let was_saved = self.is_saved();
            let output = self.entries[self.index - 1].undo(target);
            self.index -= 1;
            let is_saved = self.is_saved();
            self.socket.emit_if(old_index == 1, || Event::Undo(false));
            self.socket
                .emit_if(old_index == self.entries.len(), || Event::Redo(true));
            self.socket
                .emit_if(was_saved != is_saved, || Event::Saved(is_saved));
            self.socket.emit(|| Event::Index(self.index));
            output
        })
    }

    /// Calls the [`Edit::redo`] method for the active edit and sets
    /// the next one as the new active one.
    pub fn redo(&mut self, target: &mut E::Target) -> Option<E::Output> {
        self.can_redo().then(|| {
            let old_index = self.index;
            let was_saved = self.is_saved();
            let output = self.entries[self.index].redo(target);
            self.index += 1;
            let is_saved = self.is_saved();
            self.socket.emit_if(old_index == 0, || Event::Undo(true));
            self.socket
                .emit_if(old_index == self.len() - 1, || Event::Redo(false));
            self.socket
                .emit_if(was_saved != is_saved, || Event::Saved(is_saved));
            self.socket.emit(|| Event::Index(self.index));
            output
        })
    }

    /// Marks the target as currently being in a saved or unsaved state.
    pub fn set_saved(&mut self, saved: bool) {
        let was_saved = self.is_saved();
        if saved {
            self.saved = Some(self.index);
            self.socket.emit_if(!was_saved, || Event::Saved(true));
        } else {
            self.saved = None;
            self.socket.emit_if(was_saved, || Event::Saved(false));
        }
    }

    /// Removes all edits from the record without undoing them.
    pub fn clear(&mut self) {
        let old_index = self.index;
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        self.entries.clear();
        self.saved = self.is_saved().then_some(0);
        self.index = 0;
        self.socket.emit_if(could_undo, || Event::Undo(false));
        self.socket.emit_if(could_redo, || Event::Redo(false));
        self.socket.emit_if(old_index != 0, || Event::Index(0));
    }

    /// Revert the changes done to the target since the saved state.
    pub fn revert(&mut self, target: &mut E::Target) -> Vec<E::Output> {
        self.saved
            .map_or_else(Vec::new, |saved| self.go_to(target, saved))
    }

    /// Repeatedly calls [`Edit::undo`] or [`Edit::redo`] until the edit at `index` is reached.
    pub fn go_to(&mut self, target: &mut E::Target, index: usize) -> Vec<E::Output> {
        if index > self.len() {
            return Vec::new();
        }

        let old_index = self.index;
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        // Temporarily remove slot so they are not called each iteration.
        let slot = self.socket.disconnect();
        // Decide if we need to undo or redo to reach index.
        let undo_or_redo = if index > self.index {
            Record::redo
        } else {
            Record::undo
        };

        let capacity = self.index.abs_diff(index);
        let mut outputs = Vec::with_capacity(capacity);
        while self.index != index {
            let output = undo_or_redo(self, target).unwrap();
            outputs.push(output);
        }

        let can_undo = self.can_undo();
        let can_redo = self.can_redo();
        let is_saved = self.is_saved();
        self.socket.connect(slot);
        self.socket
            .emit_if(could_undo != can_undo, || Event::Undo(can_undo));
        self.socket
            .emit_if(could_redo != can_redo, || Event::Redo(can_redo));
        self.socket
            .emit_if(was_saved != is_saved, || Event::Saved(is_saved));
        self.socket
            .emit_if(old_index != self.index, || Event::Index(self.index));

        outputs
    }
}

impl<E: ToString, S> Record<E, S> {
    /// Returns the string of the edit which will be undone
    /// in the next call to [`Record::undo`].
    pub fn undo_string(&self) -> Option<String> {
        self.index.checked_sub(1).and_then(|i| self.string_at(i))
    }

    /// Returns the string of the edit which will be redone
    /// in the next call to [`Record::redo`].
    pub fn redo_string(&self) -> Option<String> {
        self.string_at(self.index)
    }

    fn string_at(&self, i: usize) -> Option<String> {
        self.entries.get(i).map(|e| e.edit.to_string())
    }
}

impl<E> Default for Record<E> {
    fn default() -> Record<E> {
        Record::new()
    }
}

impl<E, F> From<History<E, F>> for Record<E, F> {
    fn from(history: History<E, F>) -> Record<E, F> {
        history.record
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use alloc::string::String;
    use alloc::vec::Vec;

    enum Op {
        Add(Add),
        Del(Del),
    }

    impl Edit for Op {
        type Target = String;
        type Output = ();

        fn edit(&mut self, s: &mut String) {
            match self {
                Op::Add(add) => add.edit(s),
                Op::Del(del) => del.edit(s),
            }
        }

        fn undo(&mut self, s: &mut String) {
            match self {
                Op::Add(add) => add.undo(s),
                Op::Del(del) => del.undo(s),
            }
        }

        fn merge(&mut self, edit: Self) -> Merged<Self>
        where
            Self: Sized,
        {
            match (self, edit) {
                (Op::Add(_), Op::Del(_)) => Merged::Annul,
                (Op::Del(Del(Some(a))), Op::Add(Add(b))) if a == &b => Merged::Annul,
                (_, edit) => Merged::No(edit),
            }
        }
    }

    #[derive(Debug, PartialEq)]
    struct Add(char);

    impl Edit for Add {
        type Target = String;
        type Output = ();

        fn edit(&mut self, s: &mut String) {
            s.push(self.0);
        }

        fn undo(&mut self, s: &mut String) {
            self.0 = s.pop().unwrap();
        }
    }

    #[derive(Default)]
    struct Del(Option<char>);

    impl Edit for Del {
        type Target = String;
        type Output = ();

        fn edit(&mut self, s: &mut String) {
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
        record.edit(&mut target, Add('a'));
        record.edit(&mut target, Add('b'));
        record.edit(&mut target, Add('c'));
        record.edit(&mut target, Add('d'));
        record.edit(&mut target, Add('e'));

        record.go_to(&mut target, 0);
        assert_eq!(record.index(), 0);
        assert_eq!(target, "");
        record.go_to(&mut target, 5);
        assert_eq!(record.index(), 5);
        assert_eq!(target, "abcde");
        record.go_to(&mut target, 1);
        assert_eq!(record.index(), 1);
        assert_eq!(target, "a");
        record.go_to(&mut target, 4);
        assert_eq!(record.index(), 4);
        assert_eq!(target, "abcd");
        record.go_to(&mut target, 2);
        assert_eq!(record.index(), 2);
        assert_eq!(target, "ab");
        record.go_to(&mut target, 3);
        assert_eq!(record.index(), 3);
        assert_eq!(target, "abc");
        assert!(record.go_to(&mut target, 6).is_empty());
        assert_eq!(record.index(), 3);
    }

    #[test]
    fn annul() {
        let mut target = String::new();
        let mut record = Record::new();
        record.edit(&mut target, Op::Add(Add('a')));
        record.edit(&mut target, Op::Del(Del::default()));
        record.edit(&mut target, Op::Add(Add('b')));
        assert_eq!(record.len(), 1);
    }

    #[test]
    fn edits() {
        let mut target = String::new();
        let mut record = Record::new();
        record.edit(&mut target, Add('a'));
        record.edit(&mut target, Add('b'));
        let collected = record.edits().collect::<Vec<_>>();
        assert_eq!(&collected[..], &[&Add('a'), &Add('b')][..]);
    }
}
