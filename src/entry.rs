use crate::slot::{Signal, Slot, SW};
use crate::{Action, Merged, Result};
#[cfg(feature = "chrono")]
use chrono::{DateTime, Utc};
use core::fmt::{self, Debug, Display, Formatter};
use core::ops::IndexMut;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Wrapper around an action that contains additional metadata.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Entry<A> {
    pub action: A,
    #[cfg(feature = "chrono")]
    pub timestamp: DateTime<Utc>,
}

impl<A> From<A> for Entry<A> {
    fn from(action: A) -> Self {
        Entry {
            action,
            #[cfg(feature = "chrono")]
            timestamp: Utc::now(),
        }
    }
}

impl<A: Display> Display for Entry<A> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        (&self.action as &dyn Display).fmt(f)
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub(crate) struct Stack<E, F> {
    pub entries: E,
    pub current: usize,
    pub saved: Option<usize>,
    pub slot: SW<F>,
}

impl<E: Entries, F> Stack<E, F> {
    pub fn can_undo(&self) -> bool {
        self.current > 0
    }

    pub fn can_redo(&self) -> bool {
        self.current < self.entries.len()
    }

    pub fn is_saved(&self) -> bool {
        self.saved.map_or(false, |saved| saved == self.current)
    }
}

impl<E, F> Stack<E, F>
where
    E: Entries,
    E::Item: Action,
    F: Slot,
{
    #[allow(clippy::type_complexity)]
    pub fn apply(
        &mut self,
        target: &mut <E::Item as Action>::Target,
        mut action: E::Item,
    ) -> core::result::Result<(<E::Item as Action>::Output, bool, E), <E::Item as Action>::Error>
    {
        let output = action.apply(target)?;
        // We store the state of the stack before adding the entry.
        let current = self.current;
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        // Pop off all elements after len from record.
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
                if self.entries.limit() == self.current {
                    self.entries.pop_front();
                    self.saved = self.saved.and_then(|saved| saved.checked_sub(1));
                } else {
                    self.current += 1;
                }
                self.entries.push_back(Entry::from(action));
                false
            }
        };
        self.slot.emit_if(could_redo, Signal::Redo(false));
        self.slot.emit_if(!could_undo, Signal::Undo(true));
        self.slot.emit_if(was_saved, Signal::Saved(false));
        Ok((output, merged_or_annulled, tail))
    }

    pub fn undo(&mut self, target: &mut <E::Item as Action>::Target) -> Option<Result<E::Item>> {
        self.can_undo().then(|| {
            let was_saved = self.is_saved();
            let old = self.current;
            let output = self.entries[self.current - 1].action.undo(target)?;
            self.current -= 1;
            let is_saved = self.is_saved();
            self.slot
                .emit_if(old == self.entries.len(), Signal::Redo(true));
            self.slot.emit_if(old == 1, Signal::Undo(false));
            self.slot
                .emit_if(was_saved != is_saved, Signal::Saved(is_saved));
            Ok(output)
        })
    }

    pub fn redo(&mut self, target: &mut <E::Item as Action>::Target) -> Option<Result<E::Item>> {
        self.can_redo().then(|| {
            let was_saved = self.is_saved();
            let old = self.current;
            let output = self.entries[self.current].action.redo(target)?;
            self.current += 1;
            let is_saved = self.is_saved();
            self.slot
                .emit_if(old == self.entries.len() - 1, Signal::Redo(false));
            self.slot.emit_if(old == 0, Signal::Undo(true));
            self.slot
                .emit_if(was_saved != is_saved, Signal::Saved(is_saved));
            Ok(output)
        })
    }

    pub fn set_saved(&mut self, saved: bool) {
        let was_saved = self.is_saved();
        if saved {
            self.saved = Some(self.current);
            self.slot.emit_if(!was_saved, Signal::Saved(true));
        } else {
            self.saved = None;
            self.slot.emit_if(was_saved, Signal::Saved(false));
        }
    }

    pub fn clear(&mut self) {
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        self.entries.clear();
        self.saved = self.is_saved().then_some(0);
        self.current = 0;
        self.slot.emit_if(could_undo, Signal::Undo(false));
        self.slot.emit_if(could_redo, Signal::Redo(false));
    }
}

impl<E, F> Stack<E, F>
where
    E: Entries,
    E::Item: Action<Output = ()>,
    F: Slot,
{
    pub fn go_to(
        &mut self,
        target: &mut <E::Item as Action>::Target,
        current: usize,
    ) -> Option<Result<E::Item>> {
        if current > self.entries.len() {
            return None;
        }
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        // Temporarily remove slot so they are not called each iteration.
        let f = self.slot.disconnect();
        // Decide if we need to undo or redo to reach current.
        let undo_or_redo = if current > self.current {
            Stack::redo
        } else {
            Stack::undo
        };
        while self.current != current {
            if let Some(Err(err)) = undo_or_redo(self, target) {
                self.slot.connect(f);
                return Some(Err(err));
            }
        }
        // Add slot back.
        self.slot.connect(f);
        let can_undo = self.can_undo();
        let can_redo = self.can_redo();
        let is_saved = self.is_saved();
        self.slot
            .emit_if(could_undo != can_undo, Signal::Undo(can_undo));
        self.slot
            .emit_if(could_redo != can_redo, Signal::Redo(can_redo));
        self.slot
            .emit_if(was_saved != is_saved, Signal::Saved(is_saved));
        Some(Ok(()))
    }

    pub fn revert(&mut self, target: &mut <E::Item as Action>::Target) -> Option<Result<E::Item>> {
        self.saved.and_then(|saved| self.go_to(target, saved))
    }
}

pub trait Entries: IndexMut<usize, Output = Entry<Self::Item>> {
    type Item;

    fn limit(&self) -> usize;
    fn len(&self) -> usize;
    fn back_mut(&mut self) -> Option<&mut Entry<Self::Item>>;
    fn push_back(&mut self, e: Entry<Self::Item>);
    fn pop_front(&mut self) -> Option<Entry<Self::Item>>;
    fn pop_back(&mut self) -> Option<Entry<Self::Item>>;
    fn split_off(&mut self, at: usize) -> Self;
    fn clear(&mut self);
}
