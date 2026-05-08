//! An undo-redo library.
//!
//! An implementation of the [command pattern](https://en.wikipedia.org/wiki/Command_pattern).
//! Each modification to a target value is represented as a struct that knows how to apply
//! itself and how to reverse the change. The provided data structures wrap these commands
//! and give you linear or tree-shaped undo-redo navigation over any target type.
//!
//! # Getting started
//!
//! Implement [`Edit`] for your command type, then push edits into a [`Record`]:
//!
//! ```
//! use undo::{Edit, Record};
//!
//! struct Push(char);
//!
//! impl Edit for Push {
//!     type Target = String;
//!     type Output = ();
//!
//!     fn edit(&mut self, target: &mut String) {
//!         target.push(self.0);
//!     }
//!
//!     fn undo(&mut self, target: &mut String) {
//!         self.0 = target.pop().unwrap();
//!     }
//! }
//!
//! let mut target = String::new();
//! let mut record = Record::new();
//!
//! record.edit(&mut target, Push('a'));
//! record.edit(&mut target, Push('b'));
//! record.edit(&mut target, Push('c'));
//! assert_eq!(target, "abc");
//!
//! record.undo(&mut target);
//! record.undo(&mut target);
//! assert_eq!(target, "a");
//!
//! record.redo(&mut target);
//! assert_eq!(target, "ab");
//! ```
//!
//! # Record vs History
//!
//! Use [`Record`] for a simple linear undo stack: applying a new edit after undoing
//! discards the now-unreachable edits. Use [`History`] when you need a full undo tree —
//! undoing and then making a new edit creates a branch, and you can navigate back to any
//! prior state at any time with [`History::go_to`].
//!
//! # Key features
//!
//! * **Edit merging** — implement [`Edit::merge`] to combine consecutive edits into a
//!   single undo step (e.g. merging individual keystrokes into a word).
//! * **Saved state** — call [`Record::set_saved`] to mark a checkpoint; [`Event::Saved`]
//!   fires whenever the target transitions in or out of that state.
//! * **Edit limit** — cap the undo stack with [`record::Builder::limit`] so only the
//!   *N* most recent edits are retained.
//! * **Queue** — batch edits and commit them atomically with [`Record::queue`].
//! * **Checkpoint** — apply tentative edits and commit or cancel them atomically with
//!   [`Record::checkpoint`].
//! * **Events** — receive [`Event`] notifications via a [`Slot`] (closure, channel, or
//!   custom impl) whenever undo/redo availability or saved state changes.
//! * **Display** — visualize the edit stack or history tree with the built-in display
//!   structures, with optional colored output via the `colored` feature.
//! * **`no_std`** — works without the standard library when the `alloc` feature is enabled.
//!
//! # Cargo feature flags
//!
//! | Flag      | Default | Enables  | Description                                                    |
//! |-----------|---------|----------|----------------------------------------------------------------|
//! | `std`     | ✓       | `alloc`  | Enables the standard library and timestamps on [`Entry`].      |
//! | `alloc`   |         |          | Enables the `alloc` crate (required for all data structures).  |
//! | `colored` |         |          | Enables colored terminal output for the display structures.    |
//! | `serde`   |         |          | Enables serialization and deserialization of all public types. |

#![doc(html_root_url = "https://docs.rs/undo")]
#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDocTest;

#[cfg(feature = "alloc")]
mod add;
#[cfg(feature = "alloc")]
mod entry;
#[cfg(feature = "alloc")]
mod format;
#[cfg(feature = "alloc")]
pub mod history;
#[cfg(feature = "alloc")]
pub mod record;
#[cfg(feature = "alloc")]
mod socket;

#[doc(hidden)]
#[cfg(feature = "alloc")]
pub use add::Add;
#[cfg(feature = "alloc")]
pub use entry::Entry;
#[cfg(feature = "alloc")]
pub use history::History;
#[cfg(feature = "alloc")]
pub use record::Record;
#[cfg(feature = "alloc")]
pub use socket::{Event, Slot};

#[cfg(feature = "alloc")]
use format::Format;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Base functionality for all edit commands.
pub trait Edit {
    /// The target type.
    type Target;
    /// The output type.
    type Output;

    /// Applies the edit command on the target.
    fn edit(&mut self, target: &mut Self::Target) -> Self::Output;

    /// Restores the state of the target as it was before the edit was applied.
    fn undo(&mut self, target: &mut Self::Target) -> Self::Output;

    /// Reapplies the edit on the target.
    ///
    /// The default implementation calls [`Edit::edit`]. Override this if your
    /// edit has external side effects (e.g. file I/O, network calls) that must
    /// not be repeated on redo — in that case `redo` should replay only the
    /// in-memory state change without re-triggering the side effect.
    fn redo(&mut self, target: &mut Self::Target) -> Self::Output {
        self.edit(target)
    }

    /// Attempts to merge `other` into `self`, combining two consecutive edits into one.
    ///
    /// Implement this to collapse small incremental changes (e.g. individual keystrokes)
    /// into a single undo step. The default implementation never merges.
    ///
    /// See [`Merged`] for the possible return values.
    fn merge(&mut self, other: Self) -> Merged<Self>
    where
        Self: Sized,
    {
        Merged::No(other)
    }
}

/// Returned by [`Edit::merge`] to indicate how two consecutive edits were combined.
#[derive(Copy, Clone, Debug)]
pub enum Merged<E> {
    /// The edits were merged: `other` is absorbed into `self` and will not be
    /// pushed onto the stack separately.
    Yes,
    /// The edits were not merged: `other` is returned so it can be pushed onto
    /// the stack as a distinct edit.
    No(E),
    /// The two edits cancel each other out and both are removed from the stack.
    Annul,
}

/// A position in a history tree.
#[cfg(feature = "alloc")]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct At {
    /// The branch identifier.
    pub root: usize,
    /// The index of the edit within the branch.
    pub index: usize,
}

#[cfg(feature = "alloc")]
impl At {
    const NIL: At = At::new(0, 0);

    /// Creates a new `At` with the provided root and index.
    pub const fn new(root: usize, index: usize) -> At {
        At { root, index }
    }

    const fn no_root(index: usize) -> At {
        At::new(0, index)
    }
}
