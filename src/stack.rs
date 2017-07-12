use std::error::Error;
use {Command, Merger};

/// A stack of commands.
///
/// The `Stack` is the simplest data structure and works by pushing and
/// popping off `Command`s that modifies the `receiver`.
/// Unlike the `Record`, it does not have a special state that can be used for callbacks.
///
/// # Examples
/// ```
/// use std::error::Error;
/// use undo::{Command, Stack};
///
/// struct Add(char);
///
/// impl Command<String> for Add {
///     fn redo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
///         s.push(self.0);
///         Ok(())
///     }
///
///     fn undo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
///         self.0 = s.pop().expect("`String` is unexpectedly empty");
///         Ok(())
///     }
/// }
///
/// fn foo() -> Result<(), (Box<Command<String>>, Box<Error>)> {
///     let mut stack = Stack::default();
///
///     stack.push(Add('a'))?;
///     stack.push(Add('b'))?;
///     stack.push(Add('c'))?;
///
///     assert_eq!(stack.as_receiver(), "abc");
///
///     let c = stack.pop().unwrap()?;
///     let b = stack.pop().unwrap()?;
///     let a = stack.pop().unwrap()?;
///
///     assert_eq!(stack.as_receiver(), "");
///
///     stack.push(a)?;
///     stack.push(b)?;
///     stack.push(c)?;
///
///     assert_eq!(stack.into_receiver(), "abc");
///
///     Ok(())
/// }
/// # foo().unwrap();
/// ```
#[derive(Debug, Default)]
pub struct Stack<R> {
    commands: Vec<Box<Command<R>>>,
    receiver: R,
}

impl<R> Stack<R> {
    /// Creates a new `Stack`.
    #[inline]
    pub fn new<T: Into<R>>(receiver: T) -> Stack<R> {
        Stack {
            commands: Vec::new(),
            receiver: receiver.into(),
        }
    }

    /// Creates a new `Stack` with the given `capacity`.
    #[inline]
    pub fn with_capacity<T: Into<R>>(receiver: T, capacity: usize) -> Stack<R> {
        Stack {
            commands: Vec::with_capacity(capacity),
            receiver: receiver.into(),
        }
    }

    /// Returns the capacity of the `Stack`.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.commands.capacity()
    }

    /// Returns the number of `Command`s in the `Stack`.
    #[inline]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns `true` if the `Stack` is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Returns a reference to the `receiver`.
    #[inline]
    pub fn as_receiver(&self) -> &R {
        &self.receiver
    }

    /// Consumes the `Stack`, returning the `receiver`.
    #[inline]
    pub fn into_receiver(self) -> R {
        self.receiver
    }

    /// Pushes `cmd` on the stack and executes its [`redo`] method. The command is merged with
    /// the previous top `Command` if their `id` is equal.
    ///
    /// # Errors
    /// If an error occur when executing `redo`, the error is returned together with the `Command`.
    ///
    /// [`redo`]: ../trait.Command.html#tymethod.redo
    #[inline]
    pub fn push<C>(&mut self, mut cmd: C) -> Result<(), (Box<Command<R>>, Box<Error>)>
        where C: Command<R> + 'static,
              R: 'static,
    {
        if let Err(e) = cmd.redo(&mut self.receiver) {
            return Err((Box::new(cmd), e));
        }
        match (cmd.id(), self.commands.last().and_then(|last| last.id())) {
            (Some(id1), Some(id2)) if id1 == id2 => {
                // Merge the command with the one on the top of the stack.
                let cmd = Merger {
                    cmd1: self.commands.pop().unwrap(),
                    cmd2: Box::new(cmd),
                };
                self.commands.push(Box::new(cmd));
            }
            _ => self.commands.push(Box::new(cmd)),
        }
        Ok(())
    }

    /// Calls the top commands [`undo`] method and pops it off the stack.
    /// Returns `None` if the stack is empty.
    ///
    /// # Errors
    /// If an error occur when executing `undo` the error is returned together with the `Command`.
    ///
    /// [`undo`]: ../trait.Command.html#tymethod.undo
    #[inline]
    pub fn pop(&mut self) -> Option<Result<Box<Command<R>>, (Box<Command<R>>, Box<Error>)>> {
        let mut cmd = match self.commands.pop() {
            Some(cmd) => cmd,
            None => return None,
        };
        match cmd.undo(&mut self.receiver) {
            Ok(_) => Some(Ok(cmd)),
            Err(e) => Some(Err((cmd, e))),
        }
    }
}

impl<R> AsRef<R> for Stack<R> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.as_receiver()
    }
}
