use std::collections::VecDeque;
use std::fmt;
use {Result, UndoCmd};

/// Maintains a stack of `UndoCmd`s.
///
/// `UndoStack` uses dynamic dispatch so it can hold multiple types of commands at a given time.
///
/// It will notice when it's state changes to either dirty or clean, and call the user
/// defined methods set in [on_clean] and [on_dirty]. This is useful if you want to trigger some
/// event when the state changes, eg. enabling and disabling buttons in an ui.
///
/// [on_clean]: struct.UndoStack.html#method.on_clean
/// [on_dirty]: struct.UndoStack.html#method.on_dirty
#[derive(Default)]
pub struct UndoStack<'a> {
    // All commands on the stack.
    stack: VecDeque<Box<UndoCmd + 'a>>,
    // Current position in the stack.
    idx: usize,
    // Max amount of commands allowed on the stack.
    limit: Option<usize>,
    // Called when the state changes.
    on_state_change: Option<Box<FnMut(bool) + 'a>>,
}

impl<'a> UndoStack<'a> {
    /// Creates a new `UndoStack`.
    ///
    /// # Examples
    /// ```
    /// # #![allow(unused_variables)]
    /// # use undo::UndoStack;
    /// let stack = UndoStack::new();
    /// ```
    #[inline]
    pub fn new() -> UndoStack<'a> {
        Default::default()
    }

    /// Creates a new `UndoStack` with a limit on how many `UndoCmd`s can be stored in the stack.
    /// If this limit is reached it will start popping of commands at the bottom of the stack when
    /// pushing new commands on to the stack. No limit is set by default which means it may grow
    /// indefinitely.
    ///
    /// The stack may remove multiple commands at a time to increase performance.
    ///
    /// # Examples
    /// ```
    /// # use undo::{self, UndoCmd, UndoStack};
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
    /// # fn foo() -> undo::Result {
    /// let mut vec = vec![1, 2, 3];
    /// let mut stack = UndoStack::with_limit(2);
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    /// stack.push(cmd)?; // Pops off the first cmd.
    ///
    /// assert!(vec.is_empty());
    ///
    /// stack.undo()?;
    /// stack.undo()?;
    /// stack.undo()?; // Does nothing.
    ///
    /// assert_eq!(vec, vec![1, 2]);
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn with_limit(limit: usize) -> UndoStack<'a> {
        UndoStack {
            limit: if limit == 0 { None } else { Some(limit) },
            ..Default::default()
        }
    }

    /// Creates a new `UndoStack` with the specified [capacity].
    ///
    ///
    /// # Examples
    /// ```
    /// # use undo::UndoStack;
    /// let stack = UndoStack::with_capacity(10);
    /// assert!(stack.capacity() >= 10);
    /// ```
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn with_capacity(capacity: usize) -> UndoStack<'a> {
        UndoStack {
            stack: VecDeque::with_capacity(capacity),
            ..Default::default()
        }
    }

    /// Returns the limit of the `UndoStack`, or `None` if it has no limit.
    ///
    /// # Examples
    /// ```
    /// # use undo::UndoStack;
    /// let stack = UndoStack::with_limit(10);
    /// assert_eq!(stack.limit(), Some(10));
    ///
    /// let stack = UndoStack::new();
    /// assert_eq!(stack.limit(), None);
    /// ```
    #[inline]
    pub fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// Returns the number of commands the stack can hold without reallocating.
    ///
    /// # Examples
    /// ```
    /// # use undo::UndoStack;
    /// let stack = UndoStack::with_capacity(10);
    /// assert!(stack.capacity() >= 10);
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        self.stack.capacity()
    }

    /// Reserves capacity for at least `additional` more commands to be inserted in the given stack.
    /// The stack may reserve more space to avoid frequent reallocations.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    ///
    /// # Examples
    /// ```
    /// # use undo::{self, UndoCmd, UndoStack};
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
    /// # fn foo() -> undo::Result {
    /// let mut vec = vec![1, 2, 3];
    /// let mut stack = UndoStack::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// stack.push(cmd)?;
    /// stack.reserve(10);
    /// assert!(stack.capacity() >= 11);
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.stack.reserve(additional);
    }

    /// Shrinks the capacity of the `UndoStack` as much as possible.
    ///
    /// # Examples
    /// ```
    /// # use undo::{self, UndoCmd, UndoStack};
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
    /// # fn foo() -> undo::Result {
    /// let mut vec = vec![1, 2, 3];
    /// let mut stack = UndoStack::with_capacity(10);
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    ///
    /// assert!(stack.capacity() >= 10);
    /// stack.shrink_to_fit();
    /// assert!(stack.capacity() >= 3);
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.stack.shrink_to_fit();
    }

    /// Returns `true` if the state of the stack is clean, `false` otherwise.
    ///
    /// # Examples
    /// ```
    /// # use undo::{self, UndoCmd, UndoStack};
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
    /// # fn foo() -> undo::Result {
    /// let mut vec = vec![1, 2, 3];
    /// let mut stack = UndoStack::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// assert!(stack.is_clean()); // An empty stack is always clean.
    /// stack.push(cmd)?;
    /// assert!(stack.is_clean());
    /// stack.undo()?;
    /// assert!(!stack.is_clean());
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn is_clean(&self) -> bool {
        self.idx == self.stack.len()
    }

    /// Returns `true` if the state of the stack is dirty, `false` otherwise.
    ///
    /// # Examples
    /// ```
    /// # use undo::{self, UndoCmd, UndoStack};
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
    /// # fn foo() -> undo::Result {
    /// let mut vec = vec![1, 2, 3];
    /// let mut stack = UndoStack::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// assert!(!stack.is_dirty()); // An empty stack is always clean.
    /// stack.push(cmd)?;
    /// assert!(!stack.is_dirty());
    /// stack.undo()?;
    /// assert!(stack.is_dirty());
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn is_dirty(&self) -> bool {
        !self.is_clean()
    }

    /// Pushes `cmd` to the top of the stack and executes its [`redo`] method.
    /// This pops off all other commands above the active command from the stack.
    ///
    /// If `cmd`s id is equal to the top command on the stack, the two commands are merged.
    ///
    /// # Examples
    /// ```
    /// # use undo::{self, UndoCmd, UndoStack};
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
    /// # fn foo() -> undo::Result {
    /// let mut vec = vec![1, 2, 3];
    /// let mut stack = UndoStack::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    ///
    /// assert!(vec.is_empty());
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    ///
    /// [`redo`]: trait.UndoCmd.html#tymethod.redo
    pub fn push<T>(&mut self, mut cmd: T) -> Result
        where T: UndoCmd + 'a
    {
        let is_dirty = self.is_dirty();
        let len = self.idx;
        // Pop off all elements after len from stack.
        self.stack.truncate(len);
        cmd.redo()?;

        match (cmd.id(), self.stack.back().and_then(|cmd| cmd.id())) {
            (Some(id1), Some(id2)) if id1 == id2 => {
                // Merge the command with the one on the top of the stack.
                let cmd = MergeCmd {
                    cmd1: self.stack.pop_back().unwrap(),
                    cmd2: Box::new(cmd),
                };
                self.stack.push_back(Box::new(cmd));
            }
            _ => {
                match self.limit {
                    Some(limit) if len == limit => {
                        let _ = self.stack.pop_front();
                    }
                    _ => self.idx += 1,
                }
                self.stack.push_back(Box::new(cmd));
            }
        }

        debug_assert_eq!(self.idx, self.stack.len());
        // State is always clean after a push, check if it was dirty before.
        if is_dirty {
            if let Some(ref mut f) = self.on_state_change {
                f(true);
            }
        }
        Ok(())
    }

    /// Calls the [`redo`] method for the active `UndoCmd` and sets the next `UndoCmd` as the new
    /// active one.
    ///
    /// # Examples
    /// ```
    /// # use undo::{self, UndoCmd, UndoStack};
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
    /// # fn foo() -> undo::Result {
    /// let mut vec = vec![1, 2, 3];
    /// let mut stack = UndoStack::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    ///
    /// assert!(vec.is_empty());
    ///
    /// stack.undo()?;
    /// stack.undo()?;
    /// stack.undo()?;
    ///
    /// assert_eq!(vec, vec![1, 2, 3]);
    ///
    /// stack.redo()?;
    /// stack.redo()?;
    /// stack.redo()?;
    ///
    /// assert!(vec.is_empty());
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    ///
    /// [`redo`]: trait.UndoCmd.html#tymethod.redo
    #[inline]
    pub fn redo(&mut self) -> Result {
        if self.idx < self.stack.len() {
            let is_dirty = self.is_dirty();
            self.stack[self.idx].redo()?;
            self.idx += 1;
            // Check if stack went from dirty to clean.
            if is_dirty && self.is_clean() {
                if let Some(ref mut f) = self.on_state_change {
                    f(true);
                }
            }
        }
        Ok(())
    }

    /// Calls the [`undo`] method for the active `UndoCmd` and sets the previous `UndoCmd` as the
    /// new active one.
    ///
    /// # Examples
    /// ```
    /// # use undo::{self, UndoCmd, UndoStack};
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
    /// # fn foo() -> undo::Result {
    /// let mut vec = vec![1, 2, 3];
    /// let mut stack = UndoStack::new();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    ///
    /// assert!(vec.is_empty());
    ///
    /// stack.undo()?;
    /// stack.undo()?;
    /// stack.undo()?;
    ///
    /// assert_eq!(vec, vec![1, 2, 3]);
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    ///
    /// [`undo`]: trait.UndoCmd.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Result {
        if self.idx > 0 {
            let is_clean = self.is_clean();
            self.idx -= 1;
            self.stack[self.idx].undo()?;
            // Check if stack went from clean to dirty.
            if is_clean && self.is_dirty() {
                if let Some(ref mut f) = self.on_state_change {
                    f(false);
                }
            }
        }
        Ok(())
    }
}

impl<'a> fmt::Debug for UndoStack<'a> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("UndoStack")
            .field("stack", &self.stack)
            .field("idx", &self.idx)
            .field("limit", &self.limit)
            .field("on_state_change",
                   &if self.on_state_change.is_some() {
                       "|_| { .. }"
                   } else {
                       "None"
                   })
            .finish()
    }
}

#[derive(Debug)]
struct MergeCmd<'a> {
    cmd1: Box<UndoCmd + 'a>,
    cmd2: Box<UndoCmd + 'a>,
}

impl<'a> UndoCmd for MergeCmd<'a> {
    #[inline]
    fn redo(&mut self) -> Result {
        self.cmd1.redo()?;
        self.cmd2.redo()
    }

    #[inline]
    fn undo(&mut self) -> Result {
        self.cmd2.undo()?;
        self.cmd1.undo()
    }

    #[inline]
    fn id(&self) -> Option<u64> {
        self.cmd1.id()
    }
}

/// Builder for `UndoStack`.
///
/// # Examples
/// ```
/// # #![allow(unused_variables)]
/// # use undo::UndoStackBuilder;
/// let stack = UndoStackBuilder::new()
///     .capacity(10)
///     .limit(10)
///     .on_state_change(|is_clean| {
///         println!("{}", is_clean);
///     })
///     .build();
/// ```
#[derive(Default)]
pub struct UndoStackBuilder<'a> {
    capacity: usize,
    limit: Option<usize>,
    on_state_change: Option<Box<FnMut(bool) + 'a>>,
}

impl<'a> UndoStackBuilder<'a> {
    /// Creates a new builder.
    ///
    /// # Examples
    /// ```
    /// # #![allow(unused_variables)]
    /// # use undo::UndoStackBuilder;
    /// let builder = UndoStackBuilder::new();
    /// ```
    #[inline]
    pub fn new() -> UndoStackBuilder<'a> {
        Default::default()
    }

    /// Sets the specified [capacity] for the stack.
    ///
    /// # Examples
    /// ```
    /// # use undo::UndoStackBuilder;
    /// let stack = UndoStackBuilder::new()
    ///     .capacity(10)
    ///     .build();
    /// assert!(stack.capacity() >= 10);
    /// ```
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> UndoStackBuilder<'a> {
        self.capacity = capacity;
        self
    }

    /// Sets a limit on how many `UndoCmd`s can be stored in the stack.
    /// If this limit is reached it will start popping of commands at the bottom of the stack when
    /// pushing new commands on to the stack. No limit is set by default which means it may grow
    /// indefinitely.
    ///
    /// The stack may remove multiple commands at a time to increase performance.
    ///
    /// # Examples
    /// ```
    /// # use undo::{self, UndoCmd, UndoStackBuilder};
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
    /// # fn foo() -> undo::Result {
    /// let mut vec = vec![1, 2, 3];
    /// let mut stack = UndoStackBuilder::new()
    ///     .limit(2)
    ///     .build();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    ///
    /// stack.push(cmd)?;
    /// stack.push(cmd)?;
    /// stack.push(cmd)?; // Pops off the first cmd.
    ///
    /// assert!(vec.is_empty());
    ///
    /// stack.undo()?;
    /// stack.undo()?;
    /// stack.undo()?; // Does nothing.
    ///
    /// assert_eq!(vec, vec![1, 2]);
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn limit(mut self, limit: usize) -> UndoStackBuilder<'a> {
        self.limit = if limit == 0 { None } else { Some(limit) };
        self
    }

    /// Sets what should happen when the state changes.
    /// By default the `UndoStack` does nothing when the state changes.
    ///
    /// # Examples
    /// ```
    /// # use std::cell::Cell;
    /// # use undo::{self, UndoCmd, UndoStackBuilder};
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
    /// # fn foo() -> undo::Result {
    /// let mut vec = vec![1, 2, 3];
    /// let x = Cell::new(0);
    /// let mut stack = UndoStackBuilder::new()
    ///     .on_state_change(|is_clean| {
    ///         if is_clean {
    ///             x.set(0);
    ///         } else {
    ///             x.set(1);
    ///         }
    ///     })
    ///     .build();
    /// let cmd = PopCmd { vec: &mut vec, e: None };
    /// stack.push(cmd)?;
    /// stack.undo()?;
    /// assert_eq!(x.get(), 1);
    /// stack.redo()?;
    /// assert_eq!(x.get(), 0);
    /// # Ok(())
    /// # }
    /// # foo().unwrap();
    /// ```
    #[inline]
    pub fn on_state_change<F>(mut self, f: F) -> UndoStackBuilder<'a>
        where F: FnMut(bool) + 'a
    {
        self.on_state_change = Some(Box::new(f));
        self
    }

    /// Builds the `UndoStack`.
    ///
    /// # Examples
    /// ```
    /// # #![allow(unused_variables)]
    /// # use undo::UndoStackBuilder;
    /// let stack = UndoStackBuilder::new()
    ///     .capacity(10)
    ///     .limit(10)
    ///     .build();
    /// ```
    #[inline]
    pub fn build(self) -> UndoStack<'a> {
        let UndoStackBuilder {
            capacity,
            limit,
            on_state_change,
        } = self;
        UndoStack {
            stack: VecDeque::with_capacity(capacity),
            limit,
            on_state_change,
            ..Default::default()
        }
    }
}

impl<'a> fmt::Debug for UndoStackBuilder<'a> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("UndoStackBuilder")
            .field("capacity", &self.capacity)
            .field("limit", &self.limit)
            .field("on_state_change",
                   &if self.on_state_change.is_some() {
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

    #[derive(Clone, Copy, Debug)]
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
    fn state() {
        use std::cell::Cell;

        let x = Cell::new(0);
        let mut vec = vec![1, 2, 3];
        let mut stack = UndoStackBuilder::new()
            .on_state_change(|is_clean| if is_clean {
                                 x.set(0);
                             } else {
                                 x.set(1);
                             })
            .build();

        let cmd = PopCmd {
            vec: &mut vec,
            e: None,
        };
        for _ in 0..3 {
            stack.push(cmd).unwrap();
        }
        assert_eq!(x.get(), 0);
        assert!(vec.is_empty());

        for _ in 0..3 {
            stack.undo().unwrap();
        }
        assert_eq!(x.get(), 1);
        assert_eq!(vec, vec![1, 2, 3]);

        stack.push(cmd).unwrap();
        assert_eq!(x.get(), 0);
        assert_eq!(vec, vec![1, 2]);

        stack.undo().unwrap();
        assert_eq!(x.get(), 1);
        assert_eq!(vec, vec![1, 2, 3]);

        stack.redo().unwrap();
        assert_eq!(x.get(), 0);
        assert_eq!(vec, vec![1, 2]);
    }
}
