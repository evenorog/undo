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
//! * [History](history/struct.History.html) provides non-linear undo-redo functionality that allows you to jump between different branches.
//! * Queues that wraps a record or history and extends them with queue functionality.
//! * Checkpoints that wraps a record or history and extends them with checkpoint functionality.
//! * Actions can be merged into a single action by implementing the
//!   [merge](trait.Action.html#method.merge) method on the action.
//!   This allows smaller actions to be used to build more complex operations, or smaller incremental changes to be
//!   merged into larger changes that can be undone and redone in a single step.
//! * The target can be marked as being saved to disk and the data-structures can track the saved state and notify
//!   when it changes.
//! * The amount of changes being tracked can be configured by the user so only the `N` most recent changes are stored.
//! * Configurable display formatting using the display structure.
//!
//! # Cargo Feature Flags
//!
//! | Name    | Default | Description                                                     |
//! |---------|---------|-----------------------------------------------------------------|
//! | alloc   | ✓       | Enables the use of the alloc crate.                             |
//! | colored | ✓       | Enables colored output when visualizing the display structures. |
//! | time    |         | Enables time stamps and time travel.                            |
//! | serde   |         | Enables serialization and deserialization.                      |
//!
//! # Examples
//!
//! ```rust
//! use undo::{Action, History};
//!
//! struct Push(char);
//!
//! impl Action for Push {
//!     type Target = String;
//!     type Output = ();
//!
//!     fn apply(&mut self, s: &mut String) {
//!         s.push(self.0);
//!     }
//!
//!     fn undo(&mut self, s: &mut String) {
//!         self.0 = s.pop().expect("s is empty");
//!     }
//! }
//!
//! fn main() {
//!     let mut target = String::new();
//!     let mut history = History::new();
//!     history.apply(&mut target, Push('a'));
//!     history.apply(&mut target, Push('b'));
//!     history.apply(&mut target, Push('c'));
//!     assert_eq!(target, "abc");
//!     history.undo(&mut target);
//!     history.undo(&mut target);
//!     history.undo(&mut target);
//!     assert_eq!(target, "");
//!     history.redo(&mut target);
//!     history.redo(&mut target);
//!     history.redo(&mut target);
//!     assert_eq!(target, "abc");
//! }
//! ```

#![no_std]
#![doc(html_root_url = "https://docs.rs/undo")]
#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![cfg_attr(not(feature = "alloc"), allow(dead_code, unused_imports))]

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDocTest;

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
mod any;
mod entry;
#[cfg(feature = "alloc")]
mod format;
#[cfg(feature = "alloc")]
pub mod history;
#[cfg(feature = "alloc")]
pub mod record;
mod slot;
mod timeline;

#[cfg(feature = "alloc")]
use crate::format::Format;

use entry::Entry;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use timeline::Timeline;

#[cfg(feature = "alloc")]
pub use self::{
    any::AnyAction,
    history::History,
    record::Record,
    slot::{NoOp, Signal, Slot},
};

/// Base functionality for all actions.
pub trait Action {
    /// The target type.
    type Target;
    /// The output type.
    type Output;

    /// Applies the action on the target and returns `Ok` if everything went fine,
    /// and `Err` if something went wrong.
    fn apply(&mut self, target: &mut Self::Target) -> Self::Output;

    /// Restores the state of the target as it was before the action was applied
    /// and returns `Ok` if everything went fine, and `Err` if something went wrong.
    fn undo(&mut self, target: &mut Self::Target) -> Self::Output;

    /// Reapplies the action on the target and return `Ok` if everything went fine,
    /// and `Err` if something went wrong.
    ///
    /// The default implementation uses the [`apply`](trait.Action.html#tymethod.apply) implementation.
    fn redo(&mut self, target: &mut Self::Target) -> Self::Output {
        self.apply(target)
    }

    /// Used for manual merging of actions.
    ///
    /// You should return:
    /// * `Yes` if you have merged the two actions.
    /// The `other` action will not be added to the stack.
    /// * `No` if you have not merged the two actions.
    /// The `other` action will be added to the stack.
    /// * `Annul` if the two actions cancels each other out.
    /// This will removed both `self` and `other` from the stack.
    fn merge(&mut self, other: Self) -> Merged<Self>
    where
        Self: Sized,
    {
        Merged::No(other)
    }
}

/// Says if the action have been merged with another action.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Merged<A> {
    /// The actions have been merged.
    ///
    /// This means that the `other` action will not be added to the stack.
    Yes,
    /// The actions have not been merged.
    ///
    /// We need to return the `other` action so it can be added to the stack.
    No(A),
    /// The two actions cancels each other out.
    ///
    /// This means that both action will be removed from the stack.
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
