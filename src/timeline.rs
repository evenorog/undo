#![allow(dead_code)]

use crate::{Command, Entry, Merge, Result, Signal, Slot};
#[cfg(feature = "alloc")]
use alloc::string::{String, ToString};
use arrayvec::ArrayVec;
#[cfg(feature = "chrono")]
use chrono::Utc;
#[cfg(feature = "chrono")]
use chrono::{DateTime, TimeZone};
use core::fmt;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(bound(serialize = "C: Serialize", deserialize = "C: Deserialize<'de>"))
)]
#[derive(Clone)]
pub struct Timeline<C, F = fn(Signal)> {
    entries: ArrayVec<[Entry<C>; 32]>,
    current: usize,
    saved: Option<usize>,
    slot: Slot<F>,
}

impl<C> Timeline<C> {
    pub fn new() -> Timeline<C> {
        Builder::new().build()
    }
}

impl<C, F> Timeline<C, F> {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn limit(&self) -> usize {
        self.entries.capacity()
    }

    pub fn connect(&mut self, slot: F) -> Option<F> {
        self.slot.f.replace(slot)
    }

    pub fn disconnect(&mut self) -> Option<F> {
        self.slot.f.take()
    }

    pub fn can_undo(&self) -> bool {
        self.current() > 0
    }

    pub fn can_redo(&self) -> bool {
        self.current() < self.len()
    }

    pub fn is_saved(&self) -> bool {
        self.saved.map_or(false, |saved| saved == self.current())
    }

    pub fn current(&self) -> usize {
        self.current
    }
}

impl<C: Command, F: FnMut(Signal)> Timeline<C, F> {
    pub fn apply(&mut self, target: &mut C::Target, mut command: C) -> Result<C> {
        command.apply(target)?;
        let current = self.current();
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        // Pop off all elements after len from record.
        self.entries.truncate(current);
        // Check if the saved state was popped off.
        self.saved = self.saved.filter(|&saved| saved <= current);
        // Try to merge commands unless the target is in a saved state.
        let merged = match self.entries.last_mut() {
            Some(last) if !was_saved => last.command.merge(command),
            _ => Merge::No(command),
        };
        match merged {
            Merge::Yes => (),
            Merge::Annul => {
                self.entries.pop();
            }
            // If commands are not merged or annulled push it onto the record.
            Merge::No(command) => {
                // If limit is reached, pop off the first command.
                if self.limit() == self.current() {
                    self.entries.pop_at(0);
                    self.saved = self.saved.and_then(|saved| saved.checked_sub(1));
                } else {
                    self.current += 1;
                }
                self.entries.push(Entry::from(command));
            }
        };
        self.slot.emit_if(could_redo, Signal::Redo(false));
        self.slot.emit_if(!could_undo, Signal::Undo(true));
        self.slot.emit_if(was_saved, Signal::Saved(false));
        Ok(())
    }

    pub fn undo(&mut self, target: &mut C::Target) -> Result<C> {
        if !self.can_undo() {
            return Ok(());
        }
        let was_saved = self.is_saved();
        let old = self.current();
        self.entries[self.current - 1].undo(target)?;
        self.current -= 1;
        let len = self.len();
        let is_saved = self.is_saved();
        self.slot.emit_if(old == len, Signal::Redo(true));
        self.slot.emit_if(old == 1, Signal::Undo(false));
        self.slot
            .emit_if(was_saved != is_saved, Signal::Saved(is_saved));
        Ok(())
    }

    pub fn redo(&mut self, target: &mut C::Target) -> Result<C> {
        if !self.can_redo() {
            return Ok(());
        }
        let was_saved = self.is_saved();
        let old = self.current();
        self.entries[self.current].redo(target)?;
        self.current += 1;
        let len = self.len();
        let is_saved = self.is_saved();
        self.slot.emit_if(old == len - 1, Signal::Redo(false));
        self.slot.emit_if(old == 0, Signal::Undo(true));
        self.slot
            .emit_if(was_saved != is_saved, Signal::Saved(is_saved));
        Ok(())
    }

    pub fn go_to(&mut self, target: &mut C::Target, current: usize) -> Option<Result<C>> {
        if current > self.len() {
            return None;
        }
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        let was_saved = self.is_saved();
        // Temporarily remove slot so they are not called each iteration.
        let f = self.slot.f.take();
        // Decide if we need to undo or redo to reach current.
        let apply = if current > self.current() {
            Timeline::redo
        } else {
            Timeline::undo
        };
        while self.current() != current {
            if let Err(err) = apply(self, target) {
                self.slot.f = f;
                return Some(Err(err));
            }
        }
        // Add slot back.
        self.slot.f = f;
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

    #[cfg(feature = "chrono")]
    pub fn time_travel(
        &mut self,
        target: &mut C::Target,
        to: &DateTime<impl TimeZone>,
    ) -> Option<Result<C>> {
        let to = to.with_timezone(&Utc);
        match self.entries.binary_search_by(|e| e.timestamp.cmp(&to)) {
            Ok(current) | Err(current) => self.go_to(target, current),
        }
    }

    pub fn set_saved(&mut self, saved: bool) {
        let was_saved = self.is_saved();
        if saved {
            self.saved = Some(self.current());
            self.slot.emit_if(!was_saved, Signal::Saved(true));
        } else {
            self.saved = None;
            self.slot.emit_if(was_saved, Signal::Saved(false));
        }
    }

    pub fn revert(&mut self, target: &mut C::Target) -> Option<Result<C>> {
        self.saved.and_then(|saved| self.go_to(target, saved))
    }

    pub fn clear(&mut self) {
        let could_undo = self.can_undo();
        let could_redo = self.can_redo();
        self.entries.clear();
        self.saved = if self.is_saved() { Some(0) } else { None };
        self.current = 0;
        self.slot.emit_if(could_undo, Signal::Undo(false));
        self.slot.emit_if(could_redo, Signal::Redo(false));
    }
}

#[cfg(feature = "alloc")]
impl<C: ToString, F> Timeline<C, F> {
    pub fn undo_text(&self) -> Option<String> {
        self.current.checked_sub(1).and_then(|i| self.text(i))
    }

    pub fn redo_text(&self) -> Option<String> {
        self.text(self.current)
    }

    fn text(&self, i: usize) -> Option<String> {
        self.entries.get(i).map(|e| e.command.to_string())
    }
}

impl<C> Default for Timeline<C> {
    fn default() -> Timeline<C> {
        Timeline::new()
    }
}

impl<C: fmt::Debug, F> fmt::Debug for Timeline<C, F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Timeline")
            .field("entries", &self.entries)
            .field("current", &self.current)
            .field("saved", &self.saved)
            .field("slot", &self.slot)
            .finish()
    }
}

pub struct Builder<F = fn(Signal)> {
    saved: bool,
    slot: Slot<F>,
}

impl<F> Builder<F> {
    pub fn new() -> Builder<F> {
        Builder {
            saved: true,
            slot: Slot::default(),
        }
    }

    pub fn saved(mut self, saved: bool) -> Builder<F> {
        self.saved = saved;
        self
    }

    pub fn build<C>(self) -> Timeline<C, F> {
        Timeline {
            entries: ArrayVec::new(),
            current: 0,
            saved: if self.saved { Some(0) } else { None },
            slot: self.slot,
        }
    }
}

impl<F: FnMut(Signal)> Builder<F> {
    pub fn connect(mut self, f: F) -> Builder<F> {
        self.slot = Slot::from(f);
        self
    }
}

impl Default for Builder {
    fn default() -> Self {
        Builder::new()
    }
}
