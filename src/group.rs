use fnv::FnvHashMap;
use {Id, Key, UndoCmd, UndoStack};

/// A collection of `UndoStack`s.
///
/// An `UndoGroup` is useful when working with multiple `UndoStack`s and only one of them should
/// be active at a given time, eg. a text editor with multiple documents opened. However, if only
/// a single stack is needed, it is easier to just use the `UndoStack` directly.
#[derive(Debug, Default)]
pub struct UndoGroup<'a> {
    // The stacks in the group.
    group: FnvHashMap<Key, UndoStack<'a>>,
    // The active stack.
    active: Option<Key>,
    // Counter for generating new keys.
    key: Key,
}

impl<'a> UndoGroup<'a> {
    /// Creates a new `UndoGroup`.
    #[inline]
    pub fn new() -> Self {
        UndoGroup {
            group: FnvHashMap::default(),
            active: None,
            key: 0,
        }
    }

    /// Creates a new `UndoGroup` with the specified capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        UndoGroup {
            group: FnvHashMap::with_capacity_and_hasher(capacity, Default::default()),
            active: None,
            key: 0,
        }
    }

    /// Returns the capacity of the `UndoGroup`.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.group.capacity()
    }

    /// Reserves capacity for at least `additional` more stacks to be inserted in the given group.
    /// The group may reserve more space to avoid frequent reallocations.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.group.reserve(additional);
    }

    /// Shrinks the capacity of the `UndoGroup` as much as possible.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.group.shrink_to_fit();
    }

    /// Adds an `UndoStack` to the group and returns an unique id for this stack.
    #[inline]
    pub fn add(&mut self, stack: UndoStack<'a>) -> Id {
        let key = self.key;
        self.key += 1;
        self.group.insert(key, stack);
        Id(key)
    }

    /// Removes the `UndoStack` with the specified id and returns the stack.
    /// Returns `None` if the stack was not found.
    #[inline]
    pub fn remove(&mut self, Id(key): Id) -> Option<UndoStack<'a>> {
        // Check if it was the active stack that was removed.
        if let Some(active) = self.active {
            if active == key {
                self.clear_active();
            }
        }
        self.group.remove(&key)
    }

    /// Sets the `UndoStack` with the specified id as the current active one.
    #[inline]
    pub fn set_active(&mut self, &Id(key): &Id) {
        if self.group.contains_key(&key) {
            self.active = Some(key);
        }
    }

    /// Clears the current active `UndoStack`.
    #[inline]
    pub fn clear_active(&mut self) {
        self.active = None;
    }

    /// Calls [`is_clean`] on the active `UndoStack`, if there is one.
    /// Returns `None` if there is no active stack.
    ///
    /// [`is_clean`]: struct.UndoStack.html#method.is_clean
    #[inline]
    pub fn is_clean(&self) -> Option<bool> {
        self.active.map(|i| self.group[&i].is_clean())
    }

    /// Calls [`is_dirty`] on the active `UndoStack`, if there is one.
    /// Returns `None` if there is no active stack.
    ///
    /// [`is_dirty`]: struct.UndoStack.html#method.is_dirty
    #[inline]
    pub fn is_dirty(&self) -> Option<bool> {
        self.is_clean().map(|t| !t)
    }

    /// Calls [`push`] on the active `UndoStack`, if there is one.
    /// Does nothing if there is no active stack.
    ///
    /// [`push`]: struct.UndoStack.html#method.push
    #[inline]
    pub fn push<T>(&mut self, cmd: T)
        where T: UndoCmd + 'a,
    {
        if let Some(ref active) = self.active {
            let stack = self.group.get_mut(active).unwrap();
            stack.push(cmd);
        }
    }

    /// Calls [`redo`] on the active `UndoStack`, if there is one.
    /// Does nothing if there is no active stack.
    ///
    /// [`redo`]: struct.UndoStack.html#method.redo
    #[inline]
    pub fn redo(&mut self) {
        if let Some(ref active) = self.active {
            let stack = self.group.get_mut(active).unwrap();
            stack.redo();
        }
    }

    /// Calls [`undo`] on the active `UndoStack`, if there is one.
    /// Does nothing if there is no active stack.
    ///
    /// [`undo`]: struct.UndoStack.html#method.undo
    #[inline]
    pub fn undo(&mut self) {
        if let Some(ref active) = self.active {
            let stack = self.group.get_mut(active).unwrap();
            stack.undo();
        }
    }
}

#[cfg(test)]
mod test {
    use {UndoCmd, UndoStack, UndoGroup};

    struct PopCmd {
        vec: *mut Vec<i32>,
        e: Option<i32>,
    }

    impl UndoCmd for PopCmd {
        fn redo(&mut self) {
            self.e = unsafe {
                let ref mut vec = *self.vec;
                vec.pop()
            }
        }

        fn undo(&mut self) {
            unsafe {
                let ref mut vec = *self.vec;
                vec.push(self.e.unwrap());
            }
        }
    }

    #[test]
    fn pop() {
        let mut vec1 = vec![1, 2, 3];
        let mut vec2 = vec![1, 2, 3];

        let mut group = UndoGroup::new();

        let a = group.add(UndoStack::new());
        let b = group.add(UndoStack::new());

        group.set_active(&a);
        group.push(PopCmd { vec: &mut vec1, e: None });
        assert_eq!(vec1.len(), 2);

        group.set_active(&b);
        group.push(PopCmd { vec: &mut vec2, e: None });
        assert_eq!(vec2.len(), 2);

        group.set_active(&a);
        group.undo();
        assert_eq!(vec1.len(), 3);

        group.set_active(&b);
        group.undo();
        assert_eq!(vec2.len(), 3);

        group.remove(b);
        assert_eq!(group.group.len(), 1);

        group.redo();
        assert_eq!(vec2.len(), 3);
    }
}
