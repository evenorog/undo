use crate::{Action, Merged, Result, Signal, Slot};
#[cfg(feature = "alloc")]
use alloc::collections::VecDeque;
#[cfg(feature = "chrono")]
use chrono::{DateTime, Utc};
use core::fmt;
use core::num::NonZeroUsize;
use core::ops::{Index, IndexMut};
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

impl<A: fmt::Display> fmt::Display for Entry<A> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.action as &dyn fmt::Display).fmt(f)
    }
}

pub struct Entries<T, F> {
    entries: T,
    current: usize,
    saved: Option<usize>,
    slot: Slot<F>,
}

impl<T: Buf, F> Entries<T, F> {
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

impl<T, F> Entries<T, F>
where
    T: Buf,
    T::Item: Action,
    F: FnMut(Signal),
{
    #[allow(clippy::type_complexity)]
    pub fn apply(
        &mut self,
        target: &mut <T::Item as Action>::Target,
        mut action: T::Item,
    ) -> core::result::Result<(<T::Item as Action>::Output, bool, T), <T::Item as Action>::Error>
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

    pub fn undo(&mut self, target: &mut <T::Item as Action>::Target) -> Option<Result<T::Item>> {
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

    pub fn redo(&mut self, target: &mut <T::Item as Action>::Target) -> Option<Result<T::Item>> {
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

pub trait Buf: IndexMut<usize, Output = Entry<Self::Item>> {
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

#[cfg(feature = "alloc")]
struct LimitDeque<T> {
    deque: VecDeque<Entry<T>>,
    limit: NonZeroUsize,
}

#[cfg(feature = "alloc")]
impl<T> Index<usize> for LimitDeque<T> {
    type Output = Entry<T>;

    fn index(&self, index: usize) -> &Self::Output {
        self.deque.index(index)
    }
}

#[cfg(feature = "alloc")]
impl<T> IndexMut<usize> for LimitDeque<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.deque.index_mut(index)
    }
}

#[cfg(feature = "alloc")]
impl<T> Buf for LimitDeque<T> {
    type Item = T;

    fn limit(&self) -> usize {
        self.limit.get()
    }

    fn len(&self) -> usize {
        self.deque.len()
    }

    fn back_mut(&mut self) -> Option<&mut Entry<T>> {
        self.deque.back_mut()
    }

    fn push_back(&mut self, t: Entry<T>) {
        self.deque.push_back(t)
    }

    fn pop_front(&mut self) -> Option<Entry<T>> {
        self.deque.pop_front()
    }

    fn pop_back(&mut self) -> Option<Entry<T>> {
        self.deque.pop_back()
    }

    fn split_off(&mut self, at: usize) -> Self {
        LimitDeque {
            deque: self.deque.split_off(at),
            limit: self.limit,
        }
    }

    fn clear(&mut self) {
        self.deque.clear();
    }
}
