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
//! | Dispatch        | Static       | Dynamic         |
//! | State Handling  | Yes          | Yes             |
//! | Command Merging | Yes (manual) | Yes (automatic) |
//!
//! `undo` uses [dynamic dispatch] instead of [static dispatch] to store the commands, which means
//! it has some additional overhead compared to [`redo`]. However, this has the benefit that you
//! can store multiple types of commands in a `UndoStack` at a time. Both supports state handling
//! and command merging but `undo` will automatically merge commands with the same id, while
//! in `redo` you need to implement the merge method yourself. If state handling is not needed, it
//! can be disabled by setting the `no_state` feature flag.
//!
//! # Examples
//!
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
//! [Command Pattern]: https://en.wikipedia.org/wiki/Command_pattern
//! [`on_clean`]: struct.UndoStack.html#method.on_clean
//! [`on_dirty`]: struct.UndoStack.html#method.on_dirty
//! [automatic merging]: trait.UndoCmd.html#method.id
//! [static dispatch]: https://doc.rust-lang.org/stable/book/trait-objects.html#static-dispatch
//! [dynamic dispatch]: https://doc.rust-lang.org/stable/book/trait-objects.html#dynamic-dispatch
//! [`redo`]: https://crates.io/crates/redo

#![forbid(unstable_features)]
#![deny(missing_docs,
        missing_debug_implementations,
        unused_import_braces,
        unused_qualifications)]

extern crate fnv;

mod group;
mod stack;

pub use group::UndoGroup;
pub use stack::UndoStack;

use std::fmt;
use std::result;

/// An unique id for an `UndoStack`.
#[derive(Debug)]
pub struct Id(u32);

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
    /// use undo::{UndoCmd, UndoStack};
    ///
    /// struct TxtCmd(char);
    ///
    /// impl UndoCmd for TxtCmd {
    ///     type Err = ();
    ///
    ///     fn redo(&mut self) -> undo::Result<()> {
    ///         Ok(())
    ///     }
    ///
    ///     fn undo(&mut self) -> undo::Result<()> {
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
    /// fn foo() -> undo::Result<()> {
    ///     let mut stack = UndoStack::new();
    ///     stack.push(TxtCmd('a'))?;
    ///     stack.push(TxtCmd('b'))?; // 'a' and 'b' is merged.
    ///     stack.push(TxtCmd(' '))?;
    ///     stack.push(TxtCmd('c'))?;
    ///     stack.push(TxtCmd('d'))?; // 'c' and 'd' is merged.
    ///
    ///     println!("{:#?}", stack);
    ///     Ok(())
    /// }
    /// # foo().unwrap();
    /// ```
    ///
    /// Output:
    ///
    /// ```txt
    /// UndoStack {
    ///     stack: [
    ///         1,
    ///         _,
    ///         1
    ///     ],
    ///     idx: 3,
    ///     limit: None
    /// }
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
