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
//! let vec = Rc::new(RefCell::new(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]));
//! let mut undo_stack = UndoStack::new()
//!     .on_clean(|| println!("This is called when the stack changes from dirty to clean!"))
//!     .on_dirty(|| println!("This is called when the stack changes from clean to dirty!"));
//!
//! let cmd = PopCmd { vec: vec.clone(), e: None };
//! undo_stack.push(cmd.clone());
//! undo_stack.push(cmd.clone());
//! undo_stack.push(cmd.clone());
//!
//! assert_eq!(vec.borrow().len(), 7);
//!
//! undo_stack.undo(); // on_dirty is going to be called here.
//! undo_stack.undo();
//! undo_stack.undo();
//!
//! assert_eq!(vec.borrow().len(), 10);
//! ```

extern crate fnv;

mod group;
mod stack;

pub use group::UndoGroup;
pub use stack::UndoStack;

/// Every command needs to implement the `UndoCmd` trait to be able to be used with the `UndoStack`.
pub trait UndoCmd {
    /// Executes the desired command.
    fn redo(&mut self);
    /// Restores the state as it was before `redo` was called.
    fn undo(&mut self);
}
