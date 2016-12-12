//! A Undo/Redo library.
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
//! undo_stack.undo();
//! undo_stack.undo();
//! undo_stack.undo();
//!
//! assert_eq!(vec.borrow().len(), 10);
//! ```

/// Every command needs to implement the `UndoCmd` trait to be able to be used with the `UndoStack`.
pub trait UndoCmd {
    /// Executes the desired command.
    fn redo(&mut self);
    /// Restores the state as it was before `redo` was called.
    fn undo(&mut self);
}

/// `UndoStack` maintains a stack of `UndoCmd`s that can be undone and redone by using methods
/// on the `UndoStack`.
///
/// `UndoStack` will notice when it's state changes to either dirty or clean, and the user can
/// set methods that should be called for either state change. This is useful for example if
/// you want to automatically enable or disable undo or redo buttons based on there are any
/// more actions that can be undone or redone.
pub struct UndoStack<T: UndoCmd> {
    stack: Vec<T>,
    len: usize,
    on_clean: Option<Box<FnMut()>>,
    on_dirty: Option<Box<FnMut()>>,
}

impl<T: UndoCmd> UndoStack<T> {
    pub fn new() -> Self {
        UndoStack {
            stack: Vec::new(),
            len: 0,
            on_clean: None,
            on_dirty: None,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        UndoStack {
            stack: Vec::with_capacity(capacity),
            len: 0,
            on_clean: None,
            on_dirty: None,
        }
    }

    pub fn on_clean<F: FnMut() + 'static>(mut self, f: F) -> Self {
        self.on_clean = Some(Box::new(f));
        self
    }

    pub fn on_dirty<F: FnMut() + 'static>(mut self, f: F) -> Self {
        self.on_dirty = Some(Box::new(f));
        self
    }

    pub fn is_clean(&self) -> bool {
        self.len == self.stack.len()
    }

    pub fn is_dirty(&self) -> bool {
        !self.is_clean()
    }

    pub fn push(&mut self, mut cmd: T) {
        let is_dirty = self.is_dirty();
        self.stack.truncate(self.len);
        cmd.redo();
        self.stack.push(cmd);
        self.len += 1;
        // Check if stack went from dirty to clean.
        if is_dirty == self.is_clean() {
            if let Some(ref mut f) = self.on_clean {
                f();
            }
        }
    }

    pub fn redo(&mut self) {
        let len = self.stack.len();
        if len != 0 && self.len < len {
            let is_dirty = self.is_dirty();
            {
                let ref mut cmd = self.stack[self.len - 1];
                cmd.redo();
            }
            self.len += 1;
            // Check if stack went from dirty to clean.
            if is_dirty == self.is_clean() {
                if let Some(ref mut f) = self.on_clean {
                    f();
                }
            }
        }
    }

    pub fn undo(&mut self) {
        if !self.stack.is_empty() {
            let is_clean = self.is_clean();
            {
                let ref mut cmd = self.stack[self.len - 1];
                cmd.undo();
            }
            self.len -= 1;
            // Check if stack went from clean to dirty.
            if is_clean == self.is_dirty() {
                if let Some(ref mut f) = self.on_dirty {
                    f();
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use {UndoStack, UndoCmd};

    #[test]
    fn pop() {
        use std::rc::Rc;
        use std::cell::RefCell;

        /// Pops an element from a vector.
        #[derive(Clone)]
        struct PopCmd {
            vec: Rc<RefCell<Vec<i32>>>,
            e: Option<i32>,
        }

        impl UndoCmd for PopCmd {
            fn redo(&mut self) {
                self.e = self.vec.borrow_mut().pop();
            }

            fn undo(&mut self) {
                self.vec.borrow_mut().push(self.e.unwrap());
                self.e = None;
            }
        }

        // We need to use Rc<RefCell> since all commands are going to mutate the vec.
        let vec = Rc::new(RefCell::new(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]));
        let mut undo_stack = UndoStack::with_capacity(3);

        let cmd = PopCmd { vec: vec.clone(), e: None };
        undo_stack.push(cmd.clone());
        undo_stack.push(cmd.clone());
        undo_stack.push(cmd.clone());
        undo_stack.push(cmd.clone());
        undo_stack.push(cmd.clone());
        undo_stack.push(cmd.clone());
        undo_stack.push(cmd.clone());
        undo_stack.push(cmd.clone());
        undo_stack.push(cmd.clone());
        undo_stack.push(cmd.clone());

        assert!(vec.borrow().is_empty());

        undo_stack.undo();
        undo_stack.undo();
        undo_stack.undo();
        undo_stack.undo();
        undo_stack.undo();
        undo_stack.undo();
        undo_stack.undo();
        undo_stack.undo();
        undo_stack.undo();
        undo_stack.undo();

        assert_eq!(vec, Rc::new(RefCell::new(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9])));
    }
}
