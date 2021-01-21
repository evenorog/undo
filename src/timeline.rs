//! A timeline of commands.

#![allow(dead_code)]

#[cfg(feature = "alloc")]
use crate::format::Format;
use crate::{At, Command, Entry, Merge, Result, Signal, Slot};
#[cfg(feature = "alloc")]
use alloc::string::{String, ToString};
use arrayvec::ArrayVec;
#[cfg(feature = "chrono")]
use chrono::Utc;
#[cfg(feature = "chrono")]
use chrono::{DateTime, TimeZone};
use core::fmt::{self, Write};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A timeline of commands.
///
/// A timeline works mostly like a record but stores a fixed number of commands on the stack.
///
/// Can be used without the `alloc` feature.
///
/// # Examples
/// ```
/// # use undo::{Command, Timeline};
/// # struct Add(char);
/// # impl Command for Add {
/// #     type Target = String;
/// #     type Error = &'static str;
/// #     fn apply(&mut self, s: &mut String) -> undo::Result<Add> {
/// #         s.push(self.0);
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self, s: &mut String) -> undo::Result<Add> {
/// #         self.0 = s.pop().ok_or("s is empty")?;
/// #         Ok(())
/// #     }
/// # }
/// # fn main() -> undo::Result<Add> {
/// let mut target = String::new();
/// let mut timeline = Timeline::new();
/// timeline.apply(&mut target, Add('a'))?;
/// timeline.apply(&mut target, Add('b'))?;
/// timeline.apply(&mut target, Add('c'))?;
/// assert_eq!(target, "abc");
/// timeline.undo(&mut target)?;
/// timeline.undo(&mut target)?;
/// timeline.undo(&mut target)?;
/// assert_eq!(target, "");
/// timeline.redo(&mut target)?;
/// timeline.redo(&mut target)?;
/// timeline.redo(&mut target)?;
/// assert_eq!(target, "abc");
/// # Ok(())
/// # }
/// ```
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
    /// Returns a new timeline.
    pub fn new() -> Timeline<C> {
        Builder::new().build()
    }
}

impl<C, F> Timeline<C, F> {
    /// Returns the number of commands in the timeline.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the timeline is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the limit of the timeline.
    pub fn limit(&self) -> usize {
        self.entries.capacity()
    }

    /// Sets how the signal should be handled when the state changes.
    ///
    /// The previous slot is returned if it exists.
    pub fn connect(&mut self, slot: F) -> Option<F> {
        self.slot.f.replace(slot)
    }

    /// Removes and returns the slot if it exists.
    pub fn disconnect(&mut self) -> Option<F> {
        self.slot.f.take()
    }

    /// Returns `true` if the timeline can undo.
    pub fn can_undo(&self) -> bool {
        self.current() > 0
    }

    /// Returns `true` if the timeline can redo.
    pub fn can_redo(&self) -> bool {
        self.current() < self.len()
    }

    /// Returns `true` if the target is in a saved state, `false` otherwise.
    pub fn is_saved(&self) -> bool {
        self.saved.map_or(false, |saved| saved == self.current())
    }

    /// Returns the position of the current command.
    pub fn current(&self) -> usize {
        self.current
    }

    /// Returns a structure for configurable formatting of the record.
    #[cfg(feature = "alloc")]
    pub fn display(&self) -> Display<C, F> {
        Display::from(self)
    }
}

impl<C: Command, F: FnMut(Signal)> Timeline<C, F> {
    /// Pushes the command on top of the timeline and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
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

    /// Calls the [`undo`] method for the active command and sets
    /// the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned.
    ///
    /// [`undo`]: ../trait.Command.html#tymethod.undo
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

    /// Calls the [`redo`] method for the active command and sets
    /// the next one as the new active one.
    ///
    /// # Errors
    /// If an error occur when applying [`redo`] the error is returned.
    ///
    /// [`redo`]: trait.Command.html#method.redo
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

    /// Repeatedly calls [`undo`] or [`redo`] until the command at `current` is reached.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    /// [`redo`]: trait.Command.html#method.redo
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

    /// Go back or forward in the record to the command that was made closest to the datetime provided.
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

    /// Marks the target as currently being in a saved or unsaved state.
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

    /// Revert the changes done to the target since the saved state.
    pub fn revert(&mut self, target: &mut C::Target) -> Option<Result<C>> {
        self.saved.and_then(|saved| self.go_to(target, saved))
    }

    /// Removes all commands from the timeline without undoing them.
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
    /// Returns the string of the command which will be undone
    /// in the next call to [`undo`](struct.Timeline.html#method.undo).
    pub fn undo_text(&self) -> Option<String> {
        self.current.checked_sub(1).and_then(|i| self.text(i))
    }

    /// Returns the string of the command which will be redone
    /// in the next call to [`redo`](struct.Timeline.html#method.redo).
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

/// Builder for a Timeline.
///
/// # Examples
/// ```
/// # use undo::{Command, timeline::Builder, Timeline};
/// # struct Add(char);
/// # impl Command for Add {
/// #     type Target = String;
/// #     type Error = &'static str;
/// #     fn apply(&mut self, s: &mut String) -> undo::Result<Add> {
/// #         s.push(self.0);
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self, s: &mut String) -> undo::Result<Add> {
/// #         self.0 = s.pop().ok_or("s is empty")?;
/// #         Ok(())
/// #     }
/// # }
/// let _ = Builder::new()
///     .saved(false)
///     .connect(|s| { dbg!(s); })
///     .build::<Add>();
/// ```
pub struct Builder<F = fn(Signal)> {
    saved: bool,
    slot: Slot<F>,
}

impl<F> Builder<F> {
    /// Returns a builder for a timeline.
    pub fn new() -> Builder<F> {
        Builder {
            saved: true,
            slot: Slot::default(),
        }
    }

    /// Sets if the target is initially in a saved state.
    /// By default the target is in a saved state.
    pub fn saved(mut self, saved: bool) -> Builder<F> {
        self.saved = saved;
        self
    }

    /// Builds the timeline.
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
    /// Connects the slot.
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

/// Configurable display formatting for the timeline.
#[cfg(feature = "alloc")]
pub struct Display<'a, C, F> {
    timeline: &'a Timeline<C, F>,
    format: Format,
}

#[cfg(feature = "alloc")]
impl<C, F> Display<'_, C, F> {
    /// Show colored output (on by default).
    ///
    /// Requires the `colored` feature to be enabled.
    #[cfg(feature = "colored")]
    pub fn colored(&mut self, on: bool) -> &mut Self {
        self.format.colored = on;
        self
    }

    /// Show the current position in the output (on by default).
    pub fn current(&mut self, on: bool) -> &mut Self {
        self.format.current = on;
        self
    }

    /// Show detailed output (on by default).
    pub fn detailed(&mut self, on: bool) -> &mut Self {
        self.format.detailed = on;
        self
    }

    /// Show the position of the command (on by default).
    pub fn position(&mut self, on: bool) -> &mut Self {
        self.format.position = on;
        self
    }

    /// Show the saved command (on by default).
    pub fn saved(&mut self, on: bool) -> &mut Self {
        self.format.saved = on;
        self
    }
}

#[cfg(feature = "alloc")]
impl<C: fmt::Display, F> Display<'_, C, F> {
    fn fmt_list(&self, f: &mut fmt::Formatter, at: At, entry: Option<&Entry<C>>) -> fmt::Result {
        self.format.position(f, at, false)?;

        #[cfg(feature = "chrono")]
        if let Some(entry) = entry {
            if self.format.detailed {
                self.format.timestamp(f, &entry.timestamp)?;
            }
        }

        self.format.labels(
            f,
            at,
            At::new(0, self.timeline.current()),
            self.timeline.saved.map(|saved| At::new(0, saved)),
        )?;
        if let Some(entry) = entry {
            if self.format.detailed {
                writeln!(f)?;
                self.format.message(f, entry, None)?;
            } else {
                f.write_char(' ')?;
                self.format.message(f, entry, None)?;
                writeln!(f)?;
            }
        }
        Ok(())
    }
}

#[cfg(feature = "alloc")]
impl<'a, C, F> From<&'a Timeline<C, F>> for Display<'a, C, F> {
    fn from(timeline: &'a Timeline<C, F>) -> Self {
        Display {
            timeline,
            format: Format::default(),
        }
    }
}

#[cfg(feature = "alloc")]
impl<C: fmt::Display, F> fmt::Display for Display<'_, C, F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, entry) in self.timeline.entries.iter().enumerate().rev() {
            let at = At::new(0, i + 1);
            self.fmt_list(f, at, Some(entry))?;
        }
        self.fmt_list(f, At::new(0, 0), None)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use arrayvec::ArrayString;

    struct Add(char);

    impl Command for Add {
        type Target = ArrayString<[u8; 64]>;
        type Error = &'static str;

        fn apply(&mut self, s: &mut ArrayString<[u8; 64]>) -> Result<Add> {
            s.push(self.0);
            Ok(())
        }

        fn undo(&mut self, s: &mut ArrayString<[u8; 64]>) -> Result<Add> {
            self.0 = s.pop().ok_or("s is empty")?;
            Ok(())
        }
    }

    #[test]
    fn limit() {
        let mut target = ArrayString::new();
        let mut timeline = Timeline::new();
        for i in 64..128 {
            timeline.apply(&mut target, Add(char::from(i))).unwrap();
        }
        assert_eq!(target.len(), 64);
        assert_eq!(timeline.len(), 32);
    }
}
