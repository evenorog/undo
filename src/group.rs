use fnv::FnvHashMap;
use {UndoCmd, UndoStack};

/// Unique id for a `UndoStack`.
pub struct Uid(u64);

/// A collection of `UndoStack`s.
pub struct UndoGroup<'a, T: UndoCmd + 'a> {
    group: FnvHashMap<u64, UndoStack<'a, T>>,
    active: Option<&'a mut UndoStack<'a, T>>,
    id: u64,
}

impl<'a, T: UndoCmd> UndoGroup<'a, T> {
    /// Creates a new `UndoGroup`.
    pub fn new() -> Self {
        UndoGroup {
            group: FnvHashMap::default(),
            active: None,
            id: 0,
        }
    }

    /// Creates a new `UndoGroup` with the specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        UndoGroup {
            group: FnvHashMap::with_capacity_and_hasher(capacity, Default::default()),
            active: None,
            id: 0,
        }
    }

    /// Returns the capacity of the `UndoGroup`.
    pub fn capacity(&self) -> usize {
        self.group.capacity()
    }

    /// Returns the number of `UndoStack`s in the `UndoGroup`.
    pub fn len(&self) -> usize {
        self.group.len()
    }

    /// Adds a `UndoStack` to the group and returns an unique id for this stack.
    pub fn add_stack(&mut self, stack: UndoStack<'a, T>) -> Uid {
        let id = self.id;
        self.id += 1;
        self.group.insert(id, stack);
        Uid(id)
    }

    /// Removes the `UndoStack` with the specified id.
    ///
    /// # Panics
    ///
    /// Panics if the id does not exist in the `UndoGroup`.
    /// However, this can only happen if the id is from another `UndoGroup`.
    pub fn remove_stack(&mut self, Uid(id): Uid) -> UndoStack<'a, T> {
        let stack = self.group.remove(&id).unwrap();
        // Check if it was the active stack that was removed.
        let is_active = match self.active {
            Some(ref active) => {
                *active as *const _ == &stack as *const _
            },
            None => return stack,
        };
        // If it was, we remove it from the active stack.
        if is_active {
            self.active = None;
        }
        stack
    }

    /// Set the `UndoStack` with the specified id as the current active one.
    pub fn set_active_stack(&'a mut self, &Uid(ref id): &Uid) {
        self.active = self.group.get_mut(id);
    }

    /// Clear the current active `UndoStack`.
    pub fn clear_active_stack(&mut self) {
        self.active = None;
    }

    /// Calls [`is_clean`] on the active `UndoStack`, if there is one.
    /// Returns `None` if there is no active stack.
    ///
    /// [`is_clean`]: struct.UndoStack.html#method.is_clean
    pub fn is_clean(&self) -> Option<bool> {
        self.active.as_ref().map(|t| t.is_clean())
    }

    /// Calls [`is_dirty`] on the active `UndoStack`, if there is one.
    /// Returns `None` if there is no active stack.
    ///
    /// [`is_dirty`]: struct.UndoStack.html#method.is_dirty
    pub fn is_dirty(&self) -> Option<bool> {
        self.is_clean().map(|t| !t)
    }

    /// Calls [`push`] on the active `UndoStack`, if there is one.
    ///
    /// [`push`]: struct.UndoStack.html#method.push
    pub fn push(&mut self, cmd: T) {
        if let Some(ref mut stack) = self.active {
            stack.push(cmd);
        }
    }

    /// Calls [`redo`] on the active `UndoStack`, if there is one.
    ///
    /// [`redo`]: struct.UndoStack.html#method.redo
    pub fn redo(&mut self) {
        if let Some(ref mut stack) = self.active {
            stack.redo();
        }
    }

    /// Calls [`undo`] on the active `UndoStack`, if there is one.
    ///
    /// [`undo`]: struct.UndoStack.html#method.undo
    pub fn undo(&mut self) {
        if let Some(ref mut stack) = self.active {
            stack.undo();
        }
    }
}
