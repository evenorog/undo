//! A undo/redo library.
//!
//! It uses the [Command Pattern](https://en.wikipedia.org/wiki/Command_pattern) where the user
//! implements the `UndoCmd` trait for each command and then the commands can be used with the
//! `UndoStack`.
//!
//! The `UndoStack` has two different states, the clean state and the dirty state. The `UndoStack`
//! is in a clean state when there are no more commands that can be redone, otherwise it's in a dirty
//! state.
//!
//! # Example
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
//! // We need to use Rc<RefCell> since all commands are going to mutate the vec.
//! let vec = Rc::new(RefCell::new(vec![1, 2, 3]));
//! let mut stack = UndoStack::new()
//!     .on_clean(|| println!("This is called when the stack changes from dirty to clean!"))
//!     .on_dirty(|| println!("This is called when the stack changes from clean to dirty!"));
//!
//! let cmd = PopCmd { vec: vec.clone(), e: None };
//! stack.push(cmd.clone());
//! stack.push(cmd.clone());
//! stack.push(cmd.clone());
//!
//! assert!(vec.borrow().is_empty());
//!
//! stack.undo(); // on_dirty is going to be called here.
//! stack.undo();
//! stack.undo();
//!
//! assert_eq!(vec.borrow().len(), 3);
//! ```

extern crate fnv;

mod group;
mod stack;

pub use group::{Uid, UndoGroup};
pub use stack::UndoStack;

/// Every command needs to implement the `UndoCmd` trait to be able to be used with the `UndoStack`.
pub trait UndoCmd {
    /// Executes the desired command.
    fn redo(&mut self);

    /// Restores the state as it was before [`redo`] was called.
    ///
    /// [`redo`]: trait.UndoCmd.html#tymethod.redo
    fn undo(&mut self);
}
