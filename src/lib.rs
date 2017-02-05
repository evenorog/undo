//! An undo/redo library.
//!
//! # About
//! It uses the [Command Pattern] where the user implements the `UndoCmd` trait for each command.
//!
//! The `UndoStack` has two states, clean and dirty. The stack is clean when no more commands can
//! be redone, otherwise it is dirty. The stack will notice when it's state changes to either dirty
//! or clean, and call the user defined methods set in [`on_clean`] and [`on_dirty`]. This is useful if
//! you want to trigger some event when the state changes, eg. enabling and disabling buttons in an ui.
//!
//! It also supports [automatic merging] of commands that has the same id.
//!
//! # Examples
//! ```
//! use undo::{self, UndoCmd, UndoStack};
//!
//! #[derive(Clone, Copy)]
//! struct PopCmd {
//!     vec: *mut Vec<i32>,
//!     e: Option<i32>,
//! }
//!
//! impl UndoCmd for PopCmd {
//!     type Err = ();
//!
//!     fn redo(&mut self) -> undo::Result<()> {
//!         self.e = unsafe {
//!             let ref mut vec = *self.vec;
//!             vec.pop()
//!         };
//!         Ok(())
//!     }
//!
//!     fn undo(&mut self) -> undo::Result<()> {
//!         unsafe {
//!             let ref mut vec = *self.vec;
//!             let e = self.e.ok_or(())?;
//!             vec.push(e);
//!         }
//!         Ok(())
//!     }
//! }
//!
//! fn foo() -> undo::Result<()> {
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
//! *An unsafe implementation of `redo` and `undo` is used in examples since it is less verbose and
//! makes the examples easier to follow.*
//!
//! [Command Pattern]: https://en.wikipedia.org/wiki/Command_pattern
//! [`on_clean`]: struct.UndoStack.html#method.on_clean
//! [`on_dirty`]: struct.UndoStack.html#method.on_dirty
//! [automatic merging]: trait.UndoCmd.html#method.id

extern crate fnv;

mod group;
mod stack;

pub use group::UndoGroup;
pub use stack::UndoStack;

use std::fmt;
use std::result;

type Key = u32;

/// An unique id for an `UndoStack`.
///
/// This id is returned from the [add] method and consumed when calling the [remove] method in
/// `UndoGroup`.
///
/// [add]: struct.UndoGroup.html#method.add
/// [remove]: struct.UndoGroup.html#method.remove
#[derive(Debug)]
pub struct Id(Key);

/// A specialized `Result` that does not carry any data on success.
pub type Result<E> = result::Result<(), E>;

/// Trait that defines the functionality of a command.
///
/// Every command needs to implement this trait to be able to be used with the `UndoStack`.
pub trait UndoCmd {
    /// The error type.
    ///
    /// This needs to be the same for all `UndoCmd`s that is going to be used in the same stack or
    /// group.
    type Err;

    /// Executes the desired command and returns `Ok` if everything went fine, and `Err` if
    /// something went wrong.
    fn redo(&mut self) -> Result<Self::Err>;

    /// Restores the state as it was before [`redo`] was called and returns `Ok` if everything
    /// went fine, and `Err` if something went wrong.
    ///
    /// [`redo`]: trait.UndoCmd.html#tymethod.redo
    fn undo(&mut self) -> Result<Self::Err>;

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
    /// use undo::{self, UndoCmd, UndoStack};
    ///
    /// #[derive(Clone, Copy)]
    /// struct PopCmd {
    ///     vec: *mut Vec<i32>,
    ///     e: Option<i32>,
    /// }
    ///
    /// impl UndoCmd for PopCmd {
    ///     type Err = ();
    ///
    ///     fn redo(&mut self) -> undo::Result<()> {
    ///         self.e = unsafe {
    ///             let ref mut vec = *self.vec;
    ///             vec.pop()
    ///         };
    ///         Ok(())
    ///     }
    ///
    ///     fn undo(&mut self) -> undo::Result<()> {
    ///         unsafe {
    ///             let ref mut vec = *self.vec;
    ///             let e = self.e.ok_or(())?;
    ///             vec.push(e);
    ///         }
    ///         Ok(())
    ///     }
    ///
    ///     fn id(&self) -> Option<u64> {
    ///         Some(1)
    ///     }
    /// }
    ///
    /// fn foo() -> undo::Result<()> {
    ///     let mut vec = vec![1, 2, 3];
    ///     let mut stack = UndoStack::new();
    ///     let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    ///     stack.push(cmd)?;
    ///     stack.push(cmd)?;
    ///     stack.push(cmd)?;
    ///
    ///     assert!(vec.is_empty());
    ///     stack.undo()?;
    ///     assert_eq!(vec.len(), 3);
    ///     stack.redo()?;
    ///     assert!(vec.is_empty());
    ///     Ok(())
    /// }
    /// # foo().unwrap();
    /// ```
    #[inline]
    fn id(&self) -> Option<u64> {
        None
    }
}

impl<'a, E> fmt::Debug for UndoCmd<Err = E> + 'a {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.id() {
            Some(id) => write!(f, "{}", id),
            None => write!(f, "_"),
        }
    }
}
