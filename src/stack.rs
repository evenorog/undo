use UndoCmd;

/// `UndoStack` maintains a stack of `UndoCmd`s that can be undone and redone by using methods
/// on the `UndoStack`.
///
/// `UndoStack` will notice when it's state changes to either dirty or clean, and the user can
/// set methods that should be called for either state change. This is useful for example if
/// you want to automatically enable or disable undo or redo buttons based on there are any
/// more actions that can be undone or redone.
///
/// Note: An empty `UndoStack` is clean, so the first push will not trigger the `on_clean` method.
pub struct UndoStack<'a> {
    stack: Vec<Box<UndoCmd + 'a>>,
    len: usize,
    on_clean: Option<Box<FnMut() + 'a>>,
    on_dirty: Option<Box<FnMut() + 'a>>,
}

impl<'a> UndoStack<'a> {
    /// Creates a new `UndoStack`.
    pub fn new() -> Self {
        UndoStack {
            stack: Vec::new(),
            len: 0,
            on_clean: None,
            on_dirty: None,
        }
    }

    /// Creates a new `UndoStack` with the specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        UndoStack {
            stack: Vec::with_capacity(capacity),
            len: 0,
            on_clean: None,
            on_dirty: None,
        }
    }

    /// Returns the capacity of the `UndoStack`.
    pub fn capacity(&self) -> usize {
        self.stack.capacity()
    }

    /// Shrinks the capacity of the `UndoStack` as much as possible.
    pub fn shrink_to_fit(&mut self) {
        self.stack.shrink_to_fit()
    }

    /// Returns the number of `UndCmd`s in the `UndoStack`.
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// Returns `true` if the `UndoStack` contains no `UndoCmd`s.
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// Sets what should happen if the state changes from dirty to clean.
    /// By default the `UndoStack` does nothing when the state changes.
    ///
    /// Consumes the `UndoStack` so this method should be called when creating the `UndoStack`.
    pub fn on_clean<F: FnMut() + 'a>(mut self, f: F) -> Self {
        self.on_clean = Some(Box::new(f));
        self
    }

    /// Sets what should happen if the state changes from clean to dirty.
    /// By default the `UndoStack` does nothing when the state changes.
    ///
    /// Consumes the `UndoStack` so this method should be called when creating the `UndoStack`.
    pub fn on_dirty<F: FnMut() + 'a>(mut self, f: F) -> Self {
        self.on_dirty = Some(Box::new(f));
        self
    }

    /// Returns `true` if the state of `UndoStack` is clean.
    pub fn is_clean(&self) -> bool {
        self.len == self.stack.len()
    }

    /// Returns `true` if the state of `UndoStack` is dirty.
    pub fn is_dirty(&self) -> bool {
        !self.is_clean()
    }

    /// Pushes a `UndoCmd` to the top of the `UndoStack` and executes its [`redo`] method.
    /// This pops off all `UndoCmd`s that is above the active command from the `UndoStack`.
    ///
    /// [`redo`]: trait.UndoCmd.html#tymethod.redo
    pub fn push<'b, T>(&mut self, mut cmd: T)
        where T: UndoCmd + 'a,
    {
        let is_dirty = self.is_dirty();
        // Pop off all elements after len from stack.
        self.stack.truncate(self.len);
        cmd.redo();
        self.stack.push(Box::new(cmd));
        self.len = self.stack.len();
        // State is always clean after a push, check if it was dirty before.
        if is_dirty {
            if let Some(ref mut f) = self.on_clean {
                f();
            }
        }
    }

    /// Calls the [`redo`] method for the active `UndoCmd` and sets the next `UndoCmd` as the new
    /// active one.
    ///
    /// Calling this method when the state is clean does nothing.
    ///
    /// [`redo`]: trait.UndoCmd.html#tymethod.redo
    pub fn redo(&mut self) {
        if self.len < self.stack.len() {
            let is_dirty = self.is_dirty();
            {
                let ref mut cmd = self.stack[self.len];
                cmd.redo();
            }
            self.len += 1;
            // Check if stack went from dirty to clean.
            if is_dirty && self.is_clean() {
                if let Some(ref mut f) = self.on_clean {
                    f();
                }
            }
        }
    }

    /// Calls the [`undo`] method for the active `UndoCmd` and sets the previous `UndoCmd` as the
    /// new active one.
    ///
    /// Calling this method when there are no more commands to undo does nothing.
    ///
    /// [`undo`]: trait.UndoCmd.html#tymethod.undo
    pub fn undo(&mut self) {
        if self.len != 0 {
            let is_clean = self.is_clean();
            self.len -= 1;
            {
                let ref mut cmd = self.stack[self.len];
                cmd.undo();
            }
            // Check if stack went from clean to dirty.
            if is_clean && self.is_dirty() {
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
        use std::cell::{Cell, RefCell};

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

        let a = Cell::new(0);
        let b = Cell::new(0);
        // We need to use Rc<RefCell> since all commands are going to mutate the vec.
        let vec = Rc::new(RefCell::new(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]));
        let mut undo_stack = UndoStack::with_capacity(3)
            .on_clean(|| a.set(1))
            .on_dirty(|| b.set(1));

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

        assert_eq!(b.get(), 0);
        undo_stack.undo();
        assert_eq!(b.get(), 1);
        b.set(0);

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

        assert_eq!(a.get(), 0);
        undo_stack.push(cmd.clone());
        assert_eq!(a.get(), 1);
        a.set(0);

        assert_eq!(vec, Rc::new(RefCell::new(vec![0, 1, 2, 3, 4, 5, 6, 7, 8])));

        assert_eq!(b.get(), 0);
        undo_stack.undo();
        assert_eq!(b.get(), 1);
        b.set(0);

        assert_eq!(vec, Rc::new(RefCell::new(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9])));

        assert_eq!(a.get(), 0);
        undo_stack.redo();
        assert_eq!(a.get(), 1);
        a.set(0);

        assert_eq!(vec, Rc::new(RefCell::new(vec![0, 1, 2, 3, 4, 5, 6, 7, 8])));
    }
}
