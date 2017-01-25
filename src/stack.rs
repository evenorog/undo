use UndoCmd;

/// `UndoStack` maintains a stack of `UndoCmd`s that can be undone and redone by using methods
/// on the `UndoStack`.
///
/// `UndoStack` will notice when it's state changes to either dirty or clean, and the user can
/// set methods that should be called for either state change. This is useful for example if
/// you want to automatically enable or disable undo or redo buttons based on there are any
/// more actions that can be undone or redone.
pub struct UndoStack<'a> {
    // All commands on the stack.
    stack: Vec<Box<UndoCmd + 'a>>,
    // Current position in the stack.
    len: usize,
    // Called when the state changes from dirty to clean.
    on_clean: Box<FnMut() + 'a>,
    // Called when the state changes from clean to dirty.
    on_dirty: Box<FnMut() + 'a>,
}

impl<'a> UndoStack<'a> {
    /// Creates a new `UndoStack`.
    #[inline]
    pub fn new() -> Self {
        UndoStack {
            stack: Vec::new(),
            len: 0,
            on_clean: Box::new(|| {}),
            on_dirty: Box::new(|| {}),
        }
    }

    /// Creates a new `UndoStack` with the specified capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        UndoStack {
            stack: Vec::with_capacity(capacity),
            len: 0,
            on_clean: Box::new(|| {}),
            on_dirty: Box::new(|| {}),
        }
    }

    /// Returns the capacity of the `UndoStack`.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.stack.capacity()
    }

    /// Reserves capacity for at least `additional` more commands to be inserted in the given stack.
    /// The stack may reserve more space to avoid frequent reallocations.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.stack.reserve(additional);
    }

    /// Shrinks the capacity of the `UndoStack` as much as possible.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.stack.shrink_to_fit();
    }

    /// Sets what should happen if the state changes from dirty to clean.
    /// By default the `UndoStack` does nothing when the state changes.
    ///
    /// Note: An empty stack is clean, so the first push will not trigger this method.
    ///
    /// # Examples
    /// ```
    /// # #![allow(unused_variables)]
    /// # use undo::UndoStack;
    /// let mut x = 0;
    /// let stack = UndoStack::new()
    ///     .on_clean(|| x += 1);
    /// ```
    #[inline]
    pub fn on_clean<F>(mut self, f: F) -> Self
        where F: FnMut() + 'a,
    {
        self.on_clean = Box::new(f);
        self
    }

    /// Sets what should happen if the state changes from clean to dirty.
    /// By default the `UndoStack` does nothing when the state changes.
    ///
    /// # Examples
    /// ```
    /// # #![allow(unused_variables)]
    /// # use undo::UndoStack;
    /// let mut x = 0;
    /// let stack = UndoStack::new()
    ///     .on_dirty(|| x += 1);
    /// ```
    #[inline]
    pub fn on_dirty<F>(mut self, f: F) -> Self
        where F: FnMut() + 'a,
    {
        self.on_dirty = Box::new(f);
        self
    }

    /// Returns `true` if the state of the stack is clean, `false` otherwise.
    #[inline]
    pub fn is_clean(&self) -> bool {
        self.len == self.stack.len()
    }

    /// Returns `true` if the state of the stack is dirty, `false` otherwise.
    #[inline]
    pub fn is_dirty(&self) -> bool {
        !self.is_clean()
    }

    /// Pushes a `UndoCmd` to the top of the `UndoStack` and executes its [`redo`] method.
    /// This pops off all `UndoCmd`s that is above the active command from the `UndoStack`.
    ///
    /// If `cmd`s id is equal to the current top command, the two commands are merged.
    ///
    /// [`redo`]: trait.UndoCmd.html#tymethod.redo
    pub fn push<T>(&mut self, mut cmd: T)
        where T: UndoCmd + 'a,
    {
        let is_dirty = self.is_dirty();
        let len = self.len;
        // Pop off all elements after len from stack.
        self.stack.truncate(len);
        cmd.redo();

        // Check if we should merge cmd with the top command on stack.
        let id = cmd.id();
        if len > 0 && id.is_some() && id == unsafe { self.stack.get_unchecked(len - 1).id() } {

            // MergeCmd is the result of the merging.
            struct MergeCmd<'a> {
                cmd1: Box<UndoCmd + 'a>,
                cmd2: Box<UndoCmd + 'a>,
            }

            impl<'a> UndoCmd for MergeCmd<'a> {
                #[inline]
                fn redo(&mut self) {
                    self.cmd1.redo();
                    self.cmd2.redo();
                }

                #[inline]
                fn undo(&mut self) {
                    self.cmd2.undo();
                    self.cmd1.undo();
                }

                #[inline]
                fn id(&self) -> Option<u64> {
                    self.cmd1.id()
                }
            }

            // Merge the command with the one on the top of the stack.
            let cmd = MergeCmd {
                cmd1: unsafe {
                    // Unchecked pop.
                    self.stack.set_len(len - 1);
                    ::std::ptr::read(self.stack.get_unchecked(self.stack.len()))
                },
                cmd2: Box::new(cmd),
            };
            self.stack.push(Box::new(cmd));
        } else {
            self.len += 1;
            self.stack.push(Box::new(cmd));
        }

        // State is always clean after a push, check if it was dirty before.
        if is_dirty {
            let ref mut f = self.on_clean;
            f();
        }
    }

    /// Calls the [`redo`] method for the active `UndoCmd` and sets the next `UndoCmd` as the new
    /// active one.
    ///
    /// Calling this method when there are no more commands to redo does nothing.
    ///
    /// [`redo`]: trait.UndoCmd.html#tymethod.redo
    pub fn redo(&mut self) {
        if self.len < self.stack.len() {
            let is_dirty = self.is_dirty();
            unsafe {
                let cmd = self.stack.get_unchecked_mut(self.len);
                cmd.redo();
            }
            self.len += 1;
            // Check if stack went from dirty to clean.
            if is_dirty && self.is_clean() {
                let ref mut f = self.on_clean;
                f();
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
        if self.len > 0 {
            let is_clean = self.is_clean();
            self.len -= 1;
            unsafe {
                let cmd = self.stack.get_unchecked_mut(self.len);
                cmd.undo();
            }
            // Check if stack went from clean to dirty.
            if is_clean && self.is_dirty() {
                let ref mut f = self.on_dirty;
                f();
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::rc::Rc;
    use std::cell::{Cell, RefCell};
    use {UndoStack, UndoCmd};

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

    #[test]
    fn pop() {
        let x = Cell::new(0);
        let vec = Rc::new(RefCell::new(vec![1, 2, 3]));
        let mut undo_stack = UndoStack::with_capacity(3)
            .on_clean(|| x.set(0))
            .on_dirty(|| x.set(1));

        let cmd = PopCmd { vec: vec.clone(), e: None };
        undo_stack.push(cmd.clone());
        undo_stack.push(cmd.clone());
        undo_stack.push(cmd.clone());
        assert_eq!(x.get(), 0);
        assert!(vec.borrow().is_empty());

        undo_stack.undo();
        undo_stack.undo();
        undo_stack.undo();
        assert_eq!(x.get(), 1);
        assert_eq!(vec, Rc::new(RefCell::new(vec![1, 2, 3])));

        undo_stack.push(cmd.clone());
        assert_eq!(x.get(), 0);
        assert_eq!(vec, Rc::new(RefCell::new(vec![1, 2])));

        undo_stack.undo();
        assert_eq!(x.get(), 1);
        assert_eq!(vec, Rc::new(RefCell::new(vec![1, 2, 3])));

        undo_stack.redo();
        assert_eq!(x.get(), 0);
        assert_eq!(vec, Rc::new(RefCell::new(vec![1, 2])));
    }
}
