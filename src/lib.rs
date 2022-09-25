//! **A undo-redo library.**
//!
//! It is an implementation of the action pattern, where all modifications are done
//! by creating objects of actions that applies the modifications. All actions knows
//! how to undo the changes it applies, and by using the provided data structures
//! it is easy to apply, undo, and redo changes made to a target.
//!
//! # Features
//!
//! * [Action](trait.Action.html) provides the base functionality for all actions.
//! * [Record](record/struct.Record.html) provides basic undo-redo functionality.
//! * [Timeline](timeline/struct.Timeline.html) provides basic undo-redo functionality using a fixed size.
//! * [History](history/struct.History.html) provides non-linear undo-redo functionality that allows you to jump between different branches.
//! * A queues that wraps a record or history and extends them with queue functionality.
//! * A checkpoints that wraps a record or history and extends them with checkpoint functionality.
//! * Actions can be merged into a single action by implementing the
//!   [merge](trait.Action.html#method.merge) method on the action.
//!   This allows smaller actions to be used to build more complex operations, or smaller incremental changes to be
//!   merged into larger changes that can be undone and redone in a single step.
//! * The target can be marked as being saved to disk and the data-structures can track the saved state and notify
//!   when it changes.
//! * The amount of changes being tracked can be configured by the user so only the `N` most recent changes are stored.
//! * Configurable display formatting using the display structure.
//! * The library can be used as `no_std`.
//!
//! # Cargo Feature Flags
//!
//! * `alloc`: Enables the use of the alloc crate, enabled by default.
//! * `arrayvec`: Required for the timeline module, enabled by default.
//! * `chrono`: Enables time stamps and time travel.
//! * `serde`: Enables serialization and deserialization.
//! * `colored`: Enables colored output when visualizing the display structures.
//!
//! # Examples
//!
//! ```rust
//! use undo::{Action, History};
//!
//! struct Add(char);
//!
//! impl Action for Add {
//!     type Target = String;
//!     type Output = ();
//!     type Error = &'static str;
//!
//!     fn apply(&mut self, s: &mut String) -> undo::Result<Add> {
//!         s.push(self.0);
//!         Ok(())
//!     }
//!
//!     fn undo(&mut self, s: &mut String) -> undo::Result<Add> {
//!         self.0 = s.pop().ok_or("s is empty")?;
//!         Ok(())
//!     }
//! }
//!
//! fn main() {
//!     let mut target = String::new();
//!     let mut history = History::new();
//!     history.apply(&mut target, Add('a')).unwrap();
//!     history.apply(&mut target, Add('b')).unwrap();
//!     history.apply(&mut target, Add('c')).unwrap();
//!     assert_eq!(target, "abc");
//!     history.undo(&mut target).unwrap().unwrap();
//!     history.undo(&mut target).unwrap().unwrap();
//!     history.undo(&mut target).unwrap().unwrap();
//!     assert_eq!(target, "");
//!     history.redo(&mut target).unwrap().unwrap();
//!     history.redo(&mut target).unwrap().unwrap();
//!     history.redo(&mut target).unwrap().unwrap();
//!     assert_eq!(target, "abc");
//! }
//! ```

#![no_std]
#![doc(html_root_url = "https://docs.rs/undo")]
#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![cfg_attr(not(feature = "alloc"), allow(dead_code))]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
mod format;
#[cfg(feature = "alloc")]
pub mod history;
#[cfg(feature = "alloc")]
pub mod record;
#[cfg(feature = "arrayvec")]
pub mod timeline;

use crate::format::Format;
#[cfg(feature = "chrono")]
use chrono::{DateTime, Utc};
use core::fmt;
use core::ops::DerefMut;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "arrayvec")]
pub use self::timeline::Timeline;
#[cfg(feature = "alloc")]
pub use self::{history::History, record::Record};

/// A specialized Result type for undo-redo operations.
pub type Result<A> = core::result::Result<<A as Action>::Output, <A as Action>::Error>;

/// Base functionality for all actions.
pub trait Action {
    /// The target type.
    type Target;
    /// The output type.
    type Output;
    /// The error type.
    type Error;

    /// Applies the action on the target and returns `Ok` if everything went fine,
    /// and `Err` if something went wrong.
    fn apply(&mut self, target: &mut Self::Target) -> Result<Self>;

    /// Restores the state of the target as it was before the action was applied
    /// and returns `Ok` if everything went fine, and `Err` if something went wrong.
    fn undo(&mut self, target: &mut Self::Target) -> Result<Self>;

    /// Reapplies the action on the target and return `Ok` if everything went fine,
    /// and `Err` if something went wrong.
    ///
    /// The default implementation uses the [`apply`](trait.Action.html#tymethod.apply) implementation.
    fn redo(&mut self, target: &mut Self::Target) -> Result<Self> {
        self.apply(target)
    }

    /// Used for manual merging of actions.
    fn merge(&mut self, _: &mut Self) -> Merged
    where
        Self: Sized,
    {
        Merged::No
    }
}

impl<A, D> Action for D
where
    A: Action + ?Sized,
    D: DerefMut<Target = A>,
{
    type Target = A::Target;
    type Output = A::Output;
    type Error = A::Error;

    fn apply(&mut self, target: &mut A::Target) -> Result<Self> {
        self.deref_mut().apply(target)
    }

    fn undo(&mut self, target: &mut A::Target) -> Result<Self> {
        self.deref_mut().undo(target)
    }

    fn redo(&mut self, target: &mut A::Target) -> Result<Self> {
        self.deref_mut().redo(target)
    }
}

/// Says if the action have been merged with another action.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Merged {
    /// The actions have been merged.
    Yes,
    /// The actions have not been merged.
    No,
    /// The two actions cancels each other out.
    Annul,
}

/// A position in a history tree.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Default, Hash, Eq, PartialEq)]
struct At {
    branch: usize,
    current: usize,
}

impl At {
    const ROOT: At = At::new(0, 0);

    const fn new(branch: usize, current: usize) -> At {
        At { branch, current }
    }
}

/// The signal used for communicating state changes.
///
/// For example, if the record can no longer redo any actions, it sends a `Redo(false)`
/// signal to tell the user.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Signal {
    /// Says if the structures can undo.
    Undo(bool),
    /// Says if the structures can redo.
    Redo(bool),
    /// Says if the target is in a saved state.
    Saved(bool),
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone)]
struct Slot<F> {
    #[cfg_attr(feature = "serde", serde(default = "Option::default", skip))]
    f: Option<F>,
}

impl<F: FnMut(Signal)> Slot<F> {
    fn emit(&mut self, signal: Signal) {
        if let Some(ref mut f) = self.f {
            f(signal);
        }
    }

    fn emit_if(&mut self, cond: bool, signal: Signal) {
        if cond {
            self.emit(signal);
        }
    }
}

impl<F> From<F> for Slot<F> {
    fn from(f: F) -> Slot<F> {
        Slot { f: Some(f) }
    }
}

impl<F> Default for Slot<F> {
    fn default() -> Self {
        Slot { f: None }
    }
}

impl<F> fmt::Debug for Slot<F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.f {
            Some(_) => f.pad("Slot { .. }"),
            None => f.pad("Empty"),
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
struct Entry<A> {
    action: A,
    #[cfg(feature = "chrono")]
    timestamp: DateTime<Utc>,
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

impl<A: Action> Action for Entry<A> {
    type Target = A::Target;
    type Output = A::Output;
    type Error = A::Error;

    fn apply(&mut self, target: &mut Self::Target) -> Result<Self> {
        self.action.apply(target)
    }

    fn undo(&mut self, target: &mut Self::Target) -> Result<Self> {
        self.action.undo(target)
    }

    fn redo(&mut self, target: &mut Self::Target) -> Result<Self> {
        self.action.redo(target)
    }

    fn merge(&mut self, entry: &mut Self) -> Merged {
        self.action.merge(&mut entry.action)
    }
}

impl<A: fmt::Display> fmt::Display for Entry<A> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.action as &dyn fmt::Display).fmt(f)
    }
}
