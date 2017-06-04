use std::fmt;
use fnv::FnvHashMap;
use {Id, Result, UndoCmd, UndoStack};

/// A collection of `UndoStack`s.
///
/// An `UndoGroup` is useful when working with multiple stacks and only one of them should
/// be active at a given time, eg. a text editor with multiple documents opened. However, if only
/// a single stack is needed, it is easier to just use the stack directly.
#[derive(Default)]
pub struct UndoGroup<'a> {
    // The stacks in the group.
    group: FnvHashMap<u32, UndoStack<'a>>,
    // The active stack.
    active: Option<u32>,
    // Counter for generating new keys.
    key: u32,
    // Called when the active stack changes.
    on_stack_change: Option<Box<FnMut(Option<bool>) + 'a>>,
}

impl<'a> UndoGroup<'a> {
    /// Creates a new `UndoGroup`.
    ///
    /// # Examples
    /// ```
    /// # #![allow(unused_variables)]
    /// # use undo::UndoGroup;
    /// let group = UndoGroup::new();
    /// ```
    #[inline]
    pub fn new() -> UndoGroup<'a> {
        Default::default()
    }

    /// Creates a new `UndoGroup` with the specified capacity.
    ///
    /// # Examples
    /// ```
    /// # use undo::UndoGroup;
    /// let group = UndoGroup::with_capacity(10);
    /// assert!(group.capacity() >= 10);
    /// ```
    #[inline]
    pub fn with_capacity(capacity: usize) -> UndoGroup<'a> {
        UndoGroup {
            group: FnvHashMap::with_capacity_and_hasher(capacity, Default::default()),
            ..Default::default()
        }
    }

    /// Returns the capacity of the `UndoGroup`.
    ///
    /// # Examples
    /// ```
    /// # use undo::UndoGroup;
    /// let group = UndoGroup::with_capacity(10);
    /// assert!(group.capacity() >= 10);
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        self.group.capacity()
    }

    /// Reserves capacity for at least `additional` more stacks to be inserted in the given group.
    /// The group may reserve more space to avoid frequent reallocations.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    ///
    /// # Examples
    /// ```
    /// # use undo::UndoGroup;
    /// let mut group = UndoGroup::new();
    /// group.add_default();
    /// group.reserve(10);
    /// assert!(group.capacity() >= 11);
    /// ```
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.group.reserve(additional);
    }

    /// Shrinks the capacity of the `UndoGroup` as much as possible.
    ///
    /// # Examples
    /// ```
    /// # use undo::UndoGroup;
    /// let mut group = UndoGroup::with_capacity(10);
    /// group.add_default();
    /// group.add_default();
    /// group.add_default();
    ///
    /// assert!(group.capacity() >= 10);
    /// group.shrink_to_fit();
    /// assert!(group.capacity() >= 3);
    /// ```
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.group.shrink_to_fit();
    }

    /// Adds an `UndoStack` to the group and returns an unique id for this stack.
    ///
    /// # Examples
    /// ```
    /// # #![allow(unused_variables)]
    /// # use undo::{UndoStack, UndoGroup};
    /// let mut group = UndoGroup::new();
    /// let a = group.add(UndoStack::new());
    /// let b = group.add(UndoStack::new());
    /// let c = group.add(UndoStack::new());
    /// ```
    #[inline]
    pub fn add(&mut self, stack: UndoStack<'a>) -> Id {
        let key = self.key;
        self.key += 1;
        self.group.insert(key, stack);
        Id(key)
    }

    /// Adds a default `UndoStack` to the group and returns an unique id for this stack.
    ///
    /// # Examples
    /// ```
    /// # #![allow(unused_variables)]
    /// # use undo::UndoGroup;
    /// let mut group = UndoGroup::new();
    /// let a = group.add_default();
    /// let b = group.add_default();
    /// let c = group.add_default();
    /// ```
    #[inline]
    pub fn add_default(&mut self) -> Id {
        self.add(Default::default())
    }

    /// Removes the `UndoStack` with the specified id and returns the stack.
    /// Returns `None` if the stack was not found.
    ///
    /// # Examples
    /// ```
    /// # use undo::UndoGroup;
    /// let mut group = UndoGroup::new();
    /// let a = group.add_default();
    /// let stack = group.remove(a);
    /// assert!(stack.is_some());
    /// ```
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
    ///
    /// # Examples
    /// ```
    /// # use undo::UndoGroup;
    /// let mut group = UndoGroup::new();
    /// let a = group.add_default();
    /// group.set_active(&a);
    /// ```
    #[inline]
    pub fn set_active(&mut self, &Id(key): &Id) {
        if let Some(is_clean) = self.group.get(&key).map(|stack| stack.is_clean()) {
            self.active = Some(key);
            if let Some(ref mut f) = self.on_stack_change {
                f(Some(is_clean));
            }
        }
    }

    /// Clears the current active `UndoStack`.
    ///
    /// # Examples
    /// ```
    /// # use undo::UndoGroup;
    /// let mut group = UndoGroup::new();
    /// let a = group.add_default();
    /// group.set_active(&a);
    /// group.clear_active();
    /// ```
    #[inline]
    pub fn clear_active(&mut self) {
        self.active = None;
        if let Some(ref mut f) = self.on_stack_change {
            f(None);
        }
    }

    /// Calls [`is_clean`] on the active `UndoStack`, if there is one.
    /// Returns `None` if there is no active stack.
    ///
    /// # Examples
    /// ```
    /// # use undo::{self, UndoCmd, UndoGroup};
    /// # #[derive(Clone, Copy, Debug)]
    /// # struct PopCmd {
    /// #   vec: *mut Vec<i32>,
    /// #   e: Option<i32>,
    /// # }
    /// # impl UndoCmd for PopCmd {
    /// #   fn redo(&mut self) -> undo::Result {
    /// #       self.e = unsafe {
    /// #           let ref mut vec = *self.vec;
    /// #           vec.pop()
    /// #       };
    /// #       Ok(())
    /// #   }
    /// #   fn undo(&mut self) -> undo::Result {
    /// #       unsafe {
    /// #           let ref mut vec = *self.vec;
    /// #           vec.push(self.e.unwrap());
    /// #       }
    /// #       Ok(())
    /// #   }
    /// # }
    /// let mut vec = vec![1, 2, 3];
    /// let mut group = UndoGroup::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// let a = group.add_default();
    /// assert_eq!(group.is_clean(), None);
    /// group.set_active(&a);
    ///
    /// assert_eq!(group.is_clean(), Some(true)); // An empty stack is always clean.
    /// group.push(cmd);
    /// assert_eq!(group.is_clean(), Some(true));
    /// group.undo();
    /// assert_eq!(group.is_clean(), Some(false));
    /// ```
    ///
    /// [`is_clean`]: struct.UndoStack.html#method.is_clean
    #[inline]
    pub fn is_clean(&self) -> Option<bool> {
        self.active.map(|i| self.group[&i].is_clean())
    }

    /// Calls [`is_dirty`] on the active `UndoStack`, if there is one.
    /// Returns `None` if there is no active stack.
    ///
    /// # Examples
    /// ```
    /// # use undo::{self, UndoCmd, UndoGroup};
    /// # #[derive(Clone, Copy, Debug)]
    /// # struct PopCmd {
    /// #   vec: *mut Vec<i32>,
    /// #   e: Option<i32>,
    /// # }
    /// # impl UndoCmd for PopCmd {
    /// #   fn redo(&mut self) -> undo::Result {
    /// #       self.e = unsafe {
    /// #           let ref mut vec = *self.vec;
    /// #           vec.pop()
    /// #       };
    /// #       Ok(())
    /// #   }
    /// #   fn undo(&mut self) -> undo::Result {
    /// #       unsafe {
    /// #           let ref mut vec = *self.vec;
    /// #           vec.push(self.e.unwrap());
    /// #       }
    /// #       Ok(())
    /// #   }
    /// # }
    /// let mut vec = vec![1, 2, 3];
    /// let mut group = UndoGroup::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// let a = group.add_default();
    /// assert_eq!(group.is_dirty(), None);
    /// group.set_active(&a);
    ///
    /// assert_eq!(group.is_dirty(), Some(false)); // An empty stack is always clean.
    /// group.push(cmd);
    /// assert_eq!(group.is_dirty(), Some(false));
    /// group.undo();
    /// assert_eq!(group.is_dirty(), Some(true));
    /// ```
    ///
    /// [`is_dirty`]: struct.UndoStack.html#method.is_dirty
    #[inline]
    pub fn is_dirty(&self) -> Option<bool> {
        self.active.map(|i| self.group[&i].is_dirty())
    }

    /// Calls [`push`] on the active `UndoStack`, if there is one.
    ///
    /// Returns `Some(Ok)` if everything went fine, `Some(Err)` if something went wrong, and `None`
    /// if there is no active stack.
    ///
    /// # Examples
    /// ```
    /// # use undo::{self, UndoCmd, UndoGroup};
    /// # #[derive(Clone, Copy, Debug)]
    /// # struct PopCmd {
    /// #   vec: *mut Vec<i32>,
    /// #   e: Option<i32>,
    /// # }
    /// # impl UndoCmd for PopCmd {
    /// #   fn redo(&mut self) -> undo::Result {
    /// #       self.e = unsafe {
    /// #           let ref mut vec = *self.vec;
    /// #           vec.pop()
    /// #       };
    /// #       Ok(())
    /// #   }
    /// #   fn undo(&mut self) -> undo::Result {
    /// #       unsafe {
    /// #           let ref mut vec = *self.vec;
    /// #           vec.push(self.e.unwrap());
    /// #       }
    /// #       Ok(())
    /// #   }
    /// # }
    /// let mut vec = vec![1, 2, 3];
    /// let mut group = UndoGroup::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// let a = group.add_default();
    /// group.set_active(&a);
    ///
    /// group.push(cmd);
    /// group.push(cmd);
    /// group.push(cmd);
    ///
    /// assert!(vec.is_empty());
    /// ```
    ///
    /// [`push`]: struct.UndoStack.html#method.push
    #[inline]
    pub fn push<T>(&mut self, cmd: T) -> Option<Result>
        where T: UndoCmd + 'a
    {
        self.active
            .and_then(|active| self.group.get_mut(&active))
            .map(|stack| stack.push(cmd))
    }

    /// Calls [`redo`] on the active `UndoStack`, if there is one.
    ///
    /// Returns `Some(Ok)` if everything went fine, `Some(Err)` if something went wrong, and `None`
    /// if there is no active stack.
    ///
    /// # Examples
    /// ```
    /// # use undo::{self, UndoCmd, UndoGroup};
    /// # #[derive(Clone, Copy, Debug)]
    /// # struct PopCmd {
    /// #   vec: *mut Vec<i32>,
    /// #   e: Option<i32>,
    /// # }
    /// # impl UndoCmd for PopCmd {
    /// #   fn redo(&mut self) -> undo::Result {
    /// #       self.e = unsafe {
    /// #           let ref mut vec = *self.vec;
    /// #           vec.pop()
    /// #       };
    /// #       Ok(())
    /// #   }
    /// #   fn undo(&mut self) -> undo::Result {
    /// #       unsafe {
    /// #           let ref mut vec = *self.vec;
    /// #           vec.push(self.e.unwrap());
    /// #       }
    /// #       Ok(())
    /// #   }
    /// # }
    /// let mut vec = vec![1, 2, 3];
    /// let mut group = UndoGroup::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// let a = group.add_default();
    /// group.set_active(&a);
    ///
    /// group.push(cmd);
    /// group.push(cmd);
    /// group.push(cmd);
    ///
    /// assert!(vec.is_empty());
    ///
    /// group.undo();
    /// group.undo();
    /// group.undo();
    ///
    /// assert_eq!(vec, vec![1, 2, 3]);
    ///
    /// group.redo();
    /// group.redo();
    /// group.redo();
    ///
    /// assert!(vec.is_empty());
    /// ```
    ///
    /// [`redo`]: struct.UndoStack.html#method.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result> {
        self.active
            .and_then(|active| self.group.get_mut(&active))
            .map(|stack| stack.redo())
    }

    /// Calls [`undo`] on the active `UndoStack`, if there is one.
    ///
    /// Returns `Some(Ok)` if everything went fine, `Some(Err)` if something went wrong, and `None`
    /// if there is no active stack.
    ///
    /// # Examples
    /// ```
    /// # use undo::{self, UndoCmd, UndoGroup};
    /// # #[derive(Clone, Copy, Debug)]
    /// # struct PopCmd {
    /// #   vec: *mut Vec<i32>,
    /// #   e: Option<i32>,
    /// # }
    /// # impl UndoCmd for PopCmd {
    /// #   fn redo(&mut self) -> undo::Result {
    /// #       self.e = unsafe {
    /// #           let ref mut vec = *self.vec;
    /// #           vec.pop()
    /// #       };
    /// #       Ok(())
    /// #   }
    /// #   fn undo(&mut self) -> undo::Result {
    /// #       unsafe {
    /// #           let ref mut vec = *self.vec;
    /// #           vec.push(self.e.unwrap());
    /// #       }
    /// #       Ok(())
    /// #   }
    /// # }
    /// let mut vec = vec![1, 2, 3];
    /// let mut group = UndoGroup::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// let a = group.add_default();
    /// group.set_active(&a);
    ///
    /// group.push(cmd);
    /// group.push(cmd);
    /// group.push(cmd);
    ///
    /// assert!(vec.is_empty());
    ///
    /// group.undo();
    /// group.undo();
    /// group.undo();
    ///
    /// assert_eq!(vec, vec![1, 2, 3]);
    /// ```
    ///
    /// [`undo`]: struct.UndoStack.html#method.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result> {
        self.active
            .and_then(|active| self.group.get_mut(&active))
            .map(|stack| stack.undo())
    }
}

impl<'a> fmt::Debug for UndoGroup<'a> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("UndoGroup")
            .field("group", &self.group)
            .field("active", &self.active)
            .field("key", &self.key)
            .field("on_stack_change",
                   &if self.on_stack_change.is_some() {
                       "|_| { .. }"
                   } else {
                       "None"
                   })
            .finish()
    }
}

/// Builder for `UndoGroup`.
///
/// # Examples
/// ```
/// # #![allow(unused_variables)]
/// # use undo::UndoGroupBuilder;
/// let group = UndoGroupBuilder::new()
///     .capacity(10)
///     .on_stack_change(|is_clean| {
///         match is_clean {
///             Some(true) => { /* The new active stack is clean */ },
///             Some(false) => { /* The new active stack is dirty */ },
///             None => { /* No active stack */ },
///         }
///     })
///     .build();
/// ```
#[derive(Default)]
pub struct UndoGroupBuilder<'a> {
    capacity: usize,
    on_stack_change: Option<Box<FnMut(Option<bool>) + 'a>>,
}

impl<'a> UndoGroupBuilder<'a> {
    /// Creates a new builder.
    ///
    /// # Examples
    /// ```
    /// # #![allow(unused_variables)]
    /// # use undo::UndoGroupBuilder;
    /// let builder = UndoGroupBuilder::new();
    /// ```
    #[inline]
    pub fn new() -> UndoGroupBuilder<'a> {
        Default::default()
    }

    /// Sets the specified [capacity] for the group.
    ///
    /// # Examples
    /// ```
    /// # use undo::UndoGroupBuilder;
    /// let group = UndoGroupBuilder::new()
    ///     .capacity(10)
    ///     .build();
    /// assert!(group.capacity() >= 10);
    /// ```
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> UndoGroupBuilder<'a> {
        self.capacity = capacity;
        self
    }

    /// Sets what should happen when the active stack changes.
    /// By default the `UndoGroup` does nothing when the active stack changes.
    ///
    /// # Examples
    /// ```
    /// # #![allow(unused_variables)]
    /// # use undo::UndoGroupBuilder;
    /// let mut group = UndoGroupBuilder::new()
    ///     .on_stack_change(|is_clean| {
    ///         match is_clean {
    ///             Some(true) => { /* The new active stack is clean */ },
    ///             Some(false) => { /* The new active stack is dirty */ },
    ///             None => { /* No active stack */ },
    ///         }
    ///     })
    ///     .build();
    /// ```
    #[inline]
    pub fn on_stack_change<F>(mut self, f: F) -> UndoGroupBuilder<'a>
        where F: FnMut(Option<bool>) + 'a
    {
        self.on_stack_change = Some(Box::new(f));
        self
    }

    /// Builds the `UndoGroup`.
    ///
    /// # Examples
    /// ```
    /// # #![allow(unused_variables)]
    /// # use undo::UndoGroupBuilder;
    /// let group = UndoGroupBuilder::new()
    ///     .capacity(10)
    ///     .build();
    /// ```
    #[inline]
    pub fn build(self) -> UndoGroup<'a> {
        let UndoGroupBuilder {
            capacity,
            on_stack_change,
        } = self;
        UndoGroup {
            group: FnvHashMap::with_capacity_and_hasher(capacity, Default::default()),
            on_stack_change,
            ..Default::default()
        }
    }
}

impl<'a> fmt::Debug for UndoGroupBuilder<'a> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("UndoStackBuilder")
            .field("capacity", &self.capacity)
            .field("on_stack_change",
                   &if self.on_stack_change.is_some() {
                       "|_| { .. }"
                   } else {
                       "None"
                   })
            .finish()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Debug)]
    struct PopCmd {
        vec: *mut Vec<i32>,
        e: Option<i32>,
    }

    impl UndoCmd for PopCmd {
        fn redo(&mut self) -> Result {
            self.e = unsafe {
                let ref mut vec = *self.vec;
                vec.pop()
            };
            Ok(())
        }

        fn undo(&mut self) -> Result {
            unsafe {
                let ref mut vec = *self.vec;
                vec.push(self.e.unwrap());
            }
            Ok(())
        }
    }

    #[test]
    fn active() {
        let mut vec1 = vec![1, 2, 3];
        let mut vec2 = vec![1, 2, 3];

        let mut group = UndoGroup::new();

        let a = group.add(UndoStack::new());
        let b = group.add(UndoStack::new());

        group.set_active(&a);
        assert!(group
                    .push(PopCmd {
                              vec: &mut vec1,
                              e: None,
                          })
                    .unwrap()
                    .is_ok());
        assert_eq!(vec1.len(), 2);

        group.set_active(&b);
        assert!(group
                    .push(PopCmd {
                              vec: &mut vec2,
                              e: None,
                          })
                    .unwrap()
                    .is_ok());
        assert_eq!(vec2.len(), 2);

        group.set_active(&a);
        assert!(group.undo().unwrap().is_ok());
        assert_eq!(vec1.len(), 3);

        group.set_active(&b);
        assert!(group.undo().unwrap().is_ok());
        assert_eq!(vec2.len(), 3);

        assert!(group.remove(b).is_some());
        assert_eq!(group.group.len(), 1);

        assert!(group.redo().is_none());
        assert_eq!(vec2.len(), 3);
    }
}
