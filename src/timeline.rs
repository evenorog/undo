//! A timeline of actions.

use crate::{Action, At, Entry, Merged, Result};
use arrayvec::ArrayVec;
use core::fmt::{self, Write};
#[cfg(feature = "serde")]
use serde_crate::{Deserialize, Serialize};
#[cfg(feature = "alloc")]
use {
    crate::Format,
    alloc::string::{String, ToString},
};
#[cfg(feature = "chrono")]
use {
    chrono::{DateTime, TimeZone, Utc},
    core::convert::identity,
};

/// A timeline of actions.
///
/// A timeline works mostly like a record but stores a fixed number of actions on the stack.
///
/// Can be used without the `alloc` feature.
///
/// # Examples
/// ```
/// # use undo::{Action, Timeline};
/// # struct Add(char);
/// # impl Action for Add {
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
/// let mut timeline = Timeline::<_, 32>::new();
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
    serde(
        crate = "serde_crate",
        bound(serialize = "A: Serialize", deserialize = "A: Deserialize<'de>")
    )
)]
#[derive(Clone)]
pub struct Timeline<A, const LIMIT: usize> {
    entries: ArrayVec<Entry<A>, LIMIT>,
    current: usize,
    saved: Option<usize>,
}

impl<A, const LIMIT: usize> Timeline<A, LIMIT> {
    /// Returns a new timeline.
    pub fn new() -> Timeline<A, LIMIT> {
        Timeline {
            entries: ArrayVec::new(),
            current: 0,
            saved: Some(0),
        }
    }
}

impl<A, const LIMIT: usize> Timeline<A, LIMIT> {
    /// Returns the number of actions in the timeline.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the timeline is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
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

    /// Returns the position of the current action.
    pub fn current(&self) -> usize {
        self.current
    }

    /// Returns a structure for configurable formatting of the record.
    #[cfg(feature = "alloc")]
    pub fn display(&self) -> Display<A, LIMIT> {
        Display::from(self)
    }
}

impl<A: Action, const LIMIT: usize> Timeline<A, LIMIT> {
    /// Pushes the action on top of the timeline and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    pub fn apply(&mut self, target: &mut A::Target, mut action: A) -> Result<A> {
        action.apply(target)?;
        let current = self.current();
        let was_saved = self.is_saved();
        // Pop off all elements after len from record.
        self.entries.truncate(current);
        // Check if the saved state was popped off.
        self.saved = self.saved.filter(|&saved| saved <= current);
        // Try to merge actions unless the target is in a saved state.
        let merged = match self.entries.last_mut() {
            Some(last) if !was_saved => last.action.merge(&mut action),
            _ => Merged::No,
        };
        match merged {
            Merged::Yes => (),
            Merged::Annul => {
                self.entries.pop();
            }
            // If actions are not merged or annulled push it onto the record.
            Merged::No => {
                // If limit is reached, pop off the first action.
                if LIMIT == self.current() {
                    self.entries.pop_at(0);
                    self.saved = self.saved.and_then(|saved| saved.checked_sub(1));
                } else {
                    self.current += 1;
                }
                self.entries.push(Entry::from(action));
            }
        };
        Ok(())
    }

    /// Calls the [`undo`] method for the active action and sets
    /// the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned.
    ///
    /// [`undo`]: ../trait.Command.html#tymethod.undo
    pub fn undo(&mut self, target: &mut A::Target) -> Result<A> {
        if !self.can_undo() {
            return Ok(());
        }
        self.entries[self.current - 1].undo(target)?;
        self.current -= 1;
        Ok(())
    }

    /// Calls the [`redo`] method for the active action and sets
    /// the next one as the new active one.
    ///
    /// # Errors
    /// If an error occur when applying [`redo`] the error is returned.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    pub fn redo(&mut self, target: &mut A::Target) -> Result<A> {
        if !self.can_redo() {
            return Ok(());
        }
        self.entries[self.current].redo(target)?;
        self.current += 1;
        Ok(())
    }

    /// Repeatedly calls [`undo`] or [`redo`] until the action at `current` is reached.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    /// [`redo`]: trait.Command.html#method.redo
    pub fn go_to(&mut self, target: &mut A::Target, current: usize) -> Option<Result<A>> {
        if current > self.len() {
            return None;
        }
        // Decide if we need to undo or redo to reach current.
        let f = if current > self.current() {
            Timeline::redo
        } else {
            Timeline::undo
        };
        while self.current() != current {
            if let Err(err) = f(self, target) {
                return Some(Err(err));
            }
        }
        Some(Ok(()))
    }

    /// Go back or forward in the record to the action that was made closest to the datetime provided.
    #[cfg(feature = "chrono")]
    pub fn time_travel(
        &mut self,
        target: &mut A::Target,
        to: &DateTime<impl TimeZone>,
    ) -> Option<Result<A>> {
        let to = to.with_timezone(&Utc);
        let current = self
            .entries
            .binary_search_by(|e| e.timestamp.cmp(&to))
            .unwrap_or_else(identity);
        self.go_to(target, current)
    }

    /// Marks the target as currently being in a saved or unsaved state.
    pub fn set_saved(&mut self, saved: bool) {
        self.saved = saved.then(|| self.current());
    }

    /// Revert the changes done to the target since the saved state.
    pub fn revert(&mut self, target: &mut A::Target) -> Option<Result<A>> {
        self.saved.and_then(|saved| self.go_to(target, saved))
    }

    /// Removes all actions from the timeline without undoing them.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.saved = self.is_saved().then(|| 0);
        self.current = 0;
    }
}

#[cfg(feature = "alloc")]
impl<A: ToString, const LIMIT: usize> Timeline<A, LIMIT> {
    /// Returns the string of the action which will be undone
    /// in the next call to [`undo`](struct.Timeline.html#method.undo).
    pub fn undo_text(&self) -> Option<String> {
        self.current.checked_sub(1).and_then(|i| self.text(i))
    }

    /// Returns the string of the action which will be redone
    /// in the next call to [`redo`](struct.Timeline.html#method.redo).
    pub fn redo_text(&self) -> Option<String> {
        self.text(self.current)
    }

    fn text(&self, i: usize) -> Option<String> {
        self.entries.get(i).map(|e| e.action.to_string())
    }
}

impl<A, const LIMIT: usize> Default for Timeline<A, LIMIT> {
    fn default() -> Timeline<A, LIMIT> {
        Timeline::new()
    }
}

impl<A: fmt::Debug, const LIMIT: usize> fmt::Debug for Timeline<A, LIMIT> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Timeline")
            .field("entries", &self.entries)
            .field("current", &self.current)
            .field("saved", &self.saved)
            .finish()
    }
}

/// Configurable display formatting for the timeline.
#[cfg(feature = "alloc")]
pub struct Display<'a, A, const LIMIT: usize> {
    timeline: &'a Timeline<A, LIMIT>,
    format: Format,
}

#[cfg(feature = "alloc")]
impl<A, const LIMIT: usize> Display<'_, A, LIMIT> {
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

    /// Show the position of the action (on by default).
    pub fn position(&mut self, on: bool) -> &mut Self {
        self.format.position = on;
        self
    }

    /// Show the saved action (on by default).
    pub fn saved(&mut self, on: bool) -> &mut Self {
        self.format.saved = on;
        self
    }
}

#[cfg(feature = "alloc")]
impl<A: fmt::Display, const LIMIT: usize> Display<'_, A, LIMIT> {
    fn fmt_list(&self, f: &mut fmt::Formatter, at: At, entry: Option<&Entry<A>>) -> fmt::Result {
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
impl<'a, A, const LIMIT: usize> From<&'a Timeline<A, LIMIT>> for Display<'a, A, LIMIT> {
    fn from(timeline: &'a Timeline<A, LIMIT>) -> Self {
        Display {
            timeline,
            format: Format::default(),
        }
    }
}

#[cfg(feature = "alloc")]
impl<A: fmt::Display, const LIMIT: usize> fmt::Display for Display<'_, A, LIMIT> {
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

    impl Action for Add {
        type Target = ArrayString<64>;
        type Error = &'static str;

        fn apply(&mut self, s: &mut ArrayString<64>) -> Result<Add> {
            s.push(self.0);
            Ok(())
        }

        fn undo(&mut self, s: &mut ArrayString<64>) -> Result<Add> {
            self.0 = s.pop().ok_or("s is empty")?;
            Ok(())
        }
    }

    #[test]
    fn limit() {
        let mut target = ArrayString::new();
        let mut timeline = Timeline::<_, 32>::new();
        for i in 64..128 {
            timeline.apply(&mut target, Add(char::from(i))).unwrap();
        }
        assert_eq!(target.len(), 64);
        assert_eq!(timeline.len(), 32);
    }
}
