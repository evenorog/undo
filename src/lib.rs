//! An undo/redo library.
//!
//! It uses the [Command Pattern] where the user implements the `UndoCmd` trait for each command
//! and then the commands can be used with the `UndoStack`.
//!
//! The `UndoStack` has two different states, clean and dirty. The stack is in a clean state when
//! there are no more commands that can be redone, otherwise it's in a dirty state. The stack
//! can be configured to call a given method when this state changes, using the [on_clean] and
//! [on_dirty] methods.
//!
//! The `UndoStack` also supports automatic merging of commands that has the same [id].
//!
//! # Examples
//! ```
//! use std::rc::Rc;
//! use std::cell::RefCell;
//! use undo::{UndoCmd, UndoStack};
//!
//! /// Pops an element from a vector.
//! #[derive(Clone)]
//! struct PopCmd {
//!     vec: Rc<RefCell<Vec<i32>>>,
//!     e: Option<i32>,
//! }
//!
//! impl UndoCmd for PopCmd {
//!     fn redo(&mut self) {
//!         self.e = self.vec.borrow_mut().pop();
//!     }
//!
//!     fn undo(&mut self) {
//!         self.vec.borrow_mut().push(self.e.unwrap());
//!         self.e = None;
//!     }
//! }
//!
//! let vec = Rc::new(RefCell::new(vec![1, 2, 3]));
//! let mut stack = UndoStack::new();
//! let cmd = PopCmd { vec: vec.clone(), e: None };
//!
//! stack.push(cmd.clone());
//! stack.push(cmd.clone());
//! stack.push(cmd.clone());
//!
//! assert!(vec.borrow().is_empty());
//!
//! stack.undo();
//! stack.undo();
//! stack.undo();
//!
//! assert_eq!(vec.borrow().len(), 3);
//! ```
//!
//! [Command Pattern]: https://en.wikipedia.org/wiki/Command_pattern
//! [on_clean]: struct.UndoStack.html#method.on_clean
//! [on_dirty]: struct.UndoStack.html#method.on_dirty
//! [id]: trait.UndoCmd.html#method.id

extern crate fnv;

mod group;
mod stack;

pub use group::UndoGroup;
pub use stack::UndoStack;

use std::fmt;

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

/// Every command needs to implement the `UndoCmd` trait to be able to be used with the `UndoStack`.
pub trait UndoCmd {
    /// Executes the desired command.
    fn redo(&mut self);

    /// Restores the state as it was before [`redo`] was called.
    ///
    /// [`redo`]: trait.UndoCmd.html#tymethod.redo
    fn undo(&mut self);

    /// Used for merging of `UndoCmd`s.
    ///
    /// When two commands are merged together, undoing and redoing them are done in one step.
    /// An example where this is useful is a text editor where you might want to undo a whole word
    /// instead of each character.
    ///
    /// Two commands are merged together when a command is pushed on the `UndoStack`, and it has
    /// the same id as the top command already on the stack. It is normal to have an unique
    /// id for each implementation of `UndoCmd`, but this is not mandatory.
    ///
    /// Default implementation returns `None`, which means the command will never be merged.
    ///
    /// # Examples
    /// ```
    /// use std::rc::Rc;
    /// use std::cell::RefCell;
    /// use undo::{UndoCmd, UndoStack};
    ///
    /// /// Pops an element from a vector.
    /// #[derive(Clone)]
    /// struct PopCmd {
    ///     vec: Rc<RefCell<Vec<i32>>>,
    ///     e: Option<i32>,
    /// }
    ///
    /// impl UndoCmd for PopCmd {
    ///     fn redo(&mut self) {
    ///         self.e = self.vec.borrow_mut().pop();
    ///     }
    ///
    ///     fn undo(&mut self) {
    ///         self.vec.borrow_mut().push(self.e.unwrap());
    ///         self.e = None;
    ///     }
    ///
    ///     fn id(&self) -> Option<u64> {
    ///         Some(1)
    ///     }
    /// }
    ///
    /// fn main() {
    ///     let vec = Rc::new(RefCell::new(vec![1, 2, 3]));
    ///     let mut stack = UndoStack::new();
    ///     let cmd = PopCmd { vec: vec.clone(), e: None };
    ///
    ///     stack.push(cmd.clone());
    ///     stack.push(cmd.clone());
    ///     stack.push(cmd.clone());
    ///
    ///     assert!(vec.borrow().is_empty());
    ///
    ///     stack.undo();
    ///
    ///     assert_eq!(vec.borrow().len(), 3);
    ///
    ///     stack.redo();
    ///
    ///     assert!(vec.borrow().is_empty());
    /// }
    /// ```
    #[inline]
    fn id(&self) -> Option<u64> {
        None
    }
}

impl<'a> fmt::Debug for UndoCmd + 'a {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.id() {
            Some(id) => write!(f, "{}", id),
            None => write!(f, "_"),
        }
    }
}
