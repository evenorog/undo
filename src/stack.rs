use std::fmt::{self, Display, Formatter};
use {Command, Error, Merger};

/// The command stack.
///
/// The stack is the simplest data structure and works by pushing and
/// popping off commands that modifies the `receiver`.
///
/// # Examples
/// ```
/// use std::error::Error;
/// use std::fmt::{self, Display, Formatter};
/// use undo::{Command, Stack};
///
/// #[derive(Debug)]
/// struct Add(char);
///
/// impl Command<String> for Add {
///     fn exec(&mut self, s: &mut String) -> Result<(), Box<Error>> {
///         s.push(self.0);
///         Ok(())
///     }
///
///     fn undo(&mut self, s: &mut String) -> Result<(), Box<Error>> {
///         self.0 = s.pop().ok_or("`String` is unexpectedly empty")?;
///         Ok(())
///     }
/// }
///
/// impl Display for Add {
///     fn fmt(&self, f: &mut Formatter) -> fmt::Result {
///         write!(f, "Add `{}`", self.0)
///     }
/// }
///
/// fn foo() -> Result<(), Box<Error>> {
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
    /// Creates a new stack.
    #[inline]
    pub fn new<T: Into<R>>(receiver: T) -> Stack<R> {
        Stack {
            commands: Vec::new(),
            receiver: receiver.into(),
        }
    }

    /// Returns the number of commands in the stack.
    #[inline]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns `true` if the stack is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Removes all commands from the stack without undoing them.
    ///
    /// This resets the stack back to its initial state while leaving the receiver unmodified.
    #[inline]
    pub fn clear(&mut self) {
        self.commands.clear();
    }

    /// Pushes the command on the stack and executes its [`exec`] method.
    /// The command is merged with the previous top command if their [`id`] is equal.
    ///
    /// # Errors
    /// If an error occur when executing [`exec`], the error is returned together with the command.
    ///
    /// [`exec`]: trait.Command.html#tymethod.exec
    /// [`id`]: trait.Command.html#method.id
    #[inline]
    pub fn push<C>(&mut self, mut cmd: C) -> Result<(), Error<R>>
    where
        C: Command<R> + 'static,
        R: 'static,
    {
        match cmd.exec(&mut self.receiver) {
            Ok(_) => {
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
            Err(e) => Err(Error(Box::new(cmd), e)),
        }
    }

    /// Calls the top commands [`undo`] method and pops it off the stack.
    /// Returns `None` if the stack is empty.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned together with the command.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    #[inline]
    pub fn pop(&mut self) -> Option<Result<Box<Command<R>>, Error<R>>> {
        self.commands
            .pop()
            .map(|mut cmd| match cmd.undo(&mut self.receiver) {
                Ok(_) => Ok(cmd),
                Err(e) => Err(Error(cmd, e)),
            })
    }

    /// Returns a reference to the `receiver`.
    #[inline]
    pub fn as_receiver(&self) -> &R {
        &self.receiver
    }

    /// Consumes the stack, returning the `receiver`.
    #[inline]
    pub fn into_receiver(self) -> R {
        self.receiver
    }
}

impl<R> AsRef<R> for Stack<R> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.as_receiver()
    }
}

impl<R> From<R> for Stack<R> {
    #[inline]
    fn from(receiver: R) -> Self {
        Stack::new(receiver)
    }
}

impl<R> Display for Stack<R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        if let Some(cmd) = self.commands.last() {
            writeln!(f, "* {}", cmd)?;
            for cmd in self.commands.iter().rev().skip(1) {
                writeln!(f, "  {}", cmd)?;
            }
        }
        Ok(())
    }
}
