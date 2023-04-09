//! **An undo-redo library.**
//!
//! It is an implementation of the [command pattern](https://en.wikipedia.org/wiki/Command_pattern),
//! where all modifications are done by creating objects that applies the modifications.
//! All objects knows how to undo the changes it applies, and by using the provided data
//! structures it is easy to undo and redo edits made to a target.
//!
//! # Features
//!
//! * [`Edit`] provides the base functionality for all edit commands. Multiple [`Edit`]s can be merged into a single edit
//!   by implementing the [`Edit::merge`] method on the edit. This allows smaller edits to be used to build
//!   more complex operations, or smaller incremental changes to be merged into larger changes that can be undone and
//!   redone in a single step.
//! * [`Record`] provides basic stack based undo-redo functionality.
//! * [`History`] provides full tree based undo-redo functionality.
//! * Queue and checkpoint functionality is supported for both [`Record`] and [`History`].
//! * The target can be marked as saved to disk and the user will be notified when it changes.
//! * The amount of changes being tracked can be configured by the user so only the `N` most recent changes are stored.
//! * Configurable display formatting using the display structures.
//!
//! # Cargo Feature Flags
//!
//! | Name    | Default | Enables | Description                                                     |
//! |---------|---------|---------|-----------------------------------------------------------------|
//! | std     | ✓       | alloc   | Enables the standard library.                                   |
//! | alloc   |         |         | Enables the `alloc` crate.                                      |
//! | colored |         |         | Enables colored output when visualizing the display structures. |
//! | serde   |         |         | Enables serialization and deserialization.                      |

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
mod any;
#[cfg(feature = "alloc")]
mod entry;
#[cfg(feature = "alloc")]
mod format;
mod from_fn;
#[cfg(feature = "alloc")]
pub mod history;
mod join;
#[cfg(feature = "alloc")]
pub mod record;
#[cfg(feature = "alloc")]
mod socket;

#[cfg(feature = "alloc")]
use entry::Entry;
#[cfg(feature = "alloc")]
use format::Format;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "alloc")]
pub use any::Any;
pub use from_fn::{FromFn, TryFromFn};
#[cfg(feature = "alloc")]
pub use history::History;
pub use join::{Join, TryJoin};
#[cfg(feature = "alloc")]
pub use record::Record;
#[cfg(feature = "alloc")]
pub use socket::{Nop, Signal, Slot};

/// Base functionality for all edit commands.
pub trait Edit {
    /// The target type.
    type Target;
    /// The output type.
    type Output;

    /// Applies the edit on the target.
    fn edit(&mut self, target: &mut Self::Target) -> Self::Output;

    /// Restores the state of the target as it was before the edit was applied.
    fn undo(&mut self, target: &mut Self::Target) -> Self::Output;

    /// Reapplies the edit on the target.
    ///
    /// The default implementation uses the [`Edit::edit`] implementation.
    fn redo(&mut self, target: &mut Self::Target) -> Self::Output {
        self.edit(target)
    }

    /// Used for manual merging of edits. See [`Merged`] for more information.
    fn merge(&mut self, other: Self) -> Merged<Self>
    where
        Self: Sized,
    {
        Merged::No(other)
    }
}

/// Says if the [Edit] command have been merged with another command.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Merged<E> {
    /// The edits have been merged.
    ///
    /// This means that the `other` edit will not be added to the stack.
    Yes,
    /// The edits have not been merged.
    ///
    /// We need to return the `other` edit so it can be added to the stack.
    No(E),
    /// The two edits cancels each other out.
    ///
    /// This means that both edits will be removed from the stack.
    Annul,
}

/// A position in a history tree.
#[cfg(feature = "alloc")]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Default, Hash, Eq, PartialEq)]
struct At {
    branch: usize,
    current: usize,
}

#[cfg(feature = "alloc")]
impl At {
    const fn new(branch: usize, current: usize) -> At {
        At { branch, current }
    }

    const fn root(current: usize) -> At {
        At::new(0, current)
    }
}
