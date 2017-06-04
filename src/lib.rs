//! An undo/redo library with dynamic dispatch, state handling and automatic command merging.
//!
//! # About
//! It uses the [Command Pattern] where the user implements the `UndoCmd` trait for each command.
//!
//! The `UndoStack` has two states, clean and dirty. The stack is clean when no more commands can
//! be redone, otherwise it is dirty. The stack will notice when it's state changes to either dirty
//! or clean, and call the user defined methods set in [`on_clean`] and [`on_dirty`]. This is useful if
//! you want to trigger some event when the state changes, eg. enabling and disabling buttons in an ui.
//!
//! It also supports [automatic merging] of commands with the same id.
//!
//! # Redo vs Undo
//! |                 | Redo         | Undo            |
//! |-----------------|--------------|-----------------|
//! | Dispatch        | [Static]     | [Dynamic]       |
//! | State Handling  | Yes          | Yes             |
//! | Command Merging | Manual       | Auto            |
//!
//! Both supports command merging but `undo` will automatically merge commands with the same id
//! while in `redo` you need to implement the merge method yourself.
//!
//! # Examples
//!
//! ```
//! use undo::{self, UndoCmd, UndoStack};
//!
//! #[derive(Clone, Copy, Debug)]
//! struct PopCmd {
//!     vec: *mut Vec<i32>,
//!     e: Option<i32>,
//! }
//!
//! impl UndoCmd for PopCmd {
//!     fn redo(&mut self) -> undo::Result {
//!         self.e = unsafe {
//!             let ref mut vec = *self.vec;
//!             vec.pop()
//!         };
//!         Ok(())
//!     }
//!
//!     fn undo(&mut self) -> undo::Result {
//!         unsafe {
//!             let ref mut vec = *self.vec;
//!             vec.push(self.e.unwrap());
//!         }
//!         Ok(())
//!     }
//! }
//!
//! fn foo() -> undo::Result {
//!     let mut vec = vec![1, 2, 3];
//!     let mut stack = UndoStack::new();
//!     let cmd = PopCmd { vec: &mut vec, e: None };
//!
//!     stack.push(cmd)?;
//!     stack.push(cmd)?;
//!     stack.push(cmd)?;
//!
//!     assert!(vec.is_empty());
//!
//!     stack.undo()?;
//!     stack.undo()?;
//!     stack.undo()?;
//!
//!     assert_eq!(vec.len(), 3);
//!     Ok(())
//! }
//! # foo().unwrap();
//! ```
//!
//! [Command Pattern]: https://en.wikipedia.org/wiki/Command_pattern
//! [`on_clean`]: struct.UndoStack.html#method.on_clean
//! [`on_dirty`]: struct.UndoStack.html#method.on_dirty
//! [automatic merging]: trait.UndoCmd.html#method.id
//! [Static]: https://doc.rust-lang.org/stable/book/trait-objects.html#static-dispatch
//! [Dynamic]: https://doc.rust-lang.org/stable/book/trait-objects.html#dynamic-dispatch
//! [`redo`]: https://crates.io/crates/redo

#![forbid(unstable_features)]
#![deny(missing_docs,
        missing_debug_implementations,
        unused_import_braces,
        unused_qualifications)]

// TODO: serde?

extern crate fnv;

mod group;
mod stack;

pub use group::{UndoGroup, UndoGroupBuilder};
pub use stack::{UndoStack, UndoStackBuilder};

use std::fmt;
use std::result;
use std::error::Error;

/// A key for an `UndoStack` in an `UndoGroup`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Key(u32);

/// A specialized `Result` that does not carry any data on success.
pub type Result = result::Result<(), Box<Error>>;

/// Trait that defines the functionality of a command.
///
/// Every command needs to implement this trait to be able to be used with the `UndoStack`.
pub trait UndoCmd: fmt::Debug {
    /// Executes the desired command and returns `Ok` if everything went fine, and `Err` if
    /// something went wrong.
    fn redo(&mut self) -> Result;

    /// Restores the state as it was before [`redo`] was called and returns `Ok` if everything
    /// went fine, and `Err` if something went wrong.
    ///
    /// [`redo`]: trait.UndoCmd.html#tymethod.redo
    fn undo(&mut self) -> Result;

    /// Used for merging of `UndoCmd`s.
    ///
    /// Two commands are merged together when a command is pushed on the `UndoStack`, and it has
    /// the same id as the top command already on the stack. When commands are merged together,
    /// undoing and redoing them are done in one step. An example where this is useful is a text
    /// editor where you might want to undo a whole word instead of each character.
    ///
    /// Default implementation returns `None`, which means the command will never be merged.
    ///
    /// # Examples
    /// ```
    /// use undo::{UndoCmd, UndoStack};
    ///
    /// #[derive(Debug)]
    /// struct TxtCmd(char);
    ///
    /// impl UndoCmd for TxtCmd {
    ///     fn redo(&mut self) -> undo::Result {
    ///         Ok(())
    ///     }
    ///
    ///     fn undo(&mut self) -> undo::Result {
    ///         Ok(())
    ///     }
    ///
    ///     fn id(&self) -> Option<u64> {
    ///         // Merge cmd if not a space.
    ///         if self.0 == ' ' {
    ///             None
    ///         } else {
    ///             Some(1)
    ///         }
    ///     }
    /// }
    ///
    /// fn foo() -> undo::Result {
    ///     let mut stack = UndoStack::new();
    ///     stack.push(TxtCmd('a'))?;
    ///     stack.push(TxtCmd('b'))?; // 'a' and 'b' is merged.
    ///     stack.push(TxtCmd(' '))?;
    ///     stack.push(TxtCmd('c'))?;
    ///     stack.push(TxtCmd('d'))   // 'c' and 'd' is merged.
    /// }
    /// # foo().unwrap();
    /// ```
    #[inline]
    fn id(&self) -> Option<u64> {
        None
    }
}
