use std::error::Error;
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use Command;

/// Macro for merging commands.
///
/// # Examples
/// ```
/// # use std::error::Error;
/// # use undo::*;
/// #[derive(Debug)]
/// struct Add(char);
///
/// impl Command<String> for Add {
///     fn apply(&mut self, s: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
///         s.push(self.0);
///         Ok(())
///     }
///
///     fn undo(&mut self, s: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
///         self.0 = s.pop().ok_or("`s` is empty")?;
///         Ok(())
///     }
/// }
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     let mut record = Record::default();
///
///     let cmd = merge![Add('a'), Add('b'), Add('c')];
///     record.apply(cmd)?;
///     assert_eq!(record.as_receiver(), "abc");
///     record.undo().unwrap()?;
///     assert_eq!(record.as_receiver(), "");
///     record.redo().unwrap()?;
///     assert_eq!(record.as_receiver(), "abc");
///
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! merge {
    ($cmd1:expr, $cmd2:expr) => (
        Merged::new($cmd1, $cmd2)
    );
    ($cmd1:expr, $cmd2:expr, $($tail:expr),+) => (
        merge![Merged::new($cmd1, $cmd2), $($tail),*]
    );
}

/// The result of merging two commands.
///
/// The [`merge!`](macro.merge.html) macro can be used for convenience when merging commands.
pub struct Merged<R, C1: Command<R> + 'static, C2: Command<R> + 'static> {
    cmd1: C1,
    cmd2: C2,
    _marker: PhantomData<Box<dyn Command<R> + 'static>>,
}

impl<R, C1: Command<R> + 'static, C2: Command<R> + 'static> Merged<R, C1, C2> {
    /// Merges `cmd1` and `cmd2` into a single command.
    ///
    /// The [`id`] of the command will be the `cmd1`s [`id`].
    ///
    /// [`id`]: trait.Command.html#method.id
    #[inline]
    pub fn new(cmd1: C1, cmd2: C2) -> Merged<R, C1, C2> {
        Merged {
            cmd1,
            cmd2,
            _marker: PhantomData,
        }
    }

    /// Returns the two merged commands.
    #[inline]
    pub fn into_commands(self) -> (C1, C2) {
        (self.cmd1, self.cmd2)
    }
}

impl<R, C1: Command<R> + 'static, C2: Command<R> + 'static> Command<R> for Merged<R, C1, C2> {
    #[inline]
    fn apply(&mut self, receiver: &mut R) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.cmd1.apply(receiver)?;
        self.cmd2.apply(receiver)
    }

    #[inline]
    fn undo(&mut self, receiver: &mut R) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.cmd2.undo(receiver)?;
        self.cmd1.undo(receiver)
    }

    #[inline]
    fn redo(&mut self, receiver: &mut R) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.cmd1.redo(receiver)?;
        self.cmd2.redo(receiver)
    }

    #[inline]
    fn id(&self) -> Option<u32> {
        self.cmd1.id()
    }
}

impl<R, C1: Command<R> + 'static, C2: Command<R> + 'static> Debug for Merged<R, C1, C2> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Merged")
            .field("cmd1", &self.cmd1)
            .field("cmd2", &self.cmd2)
            .finish()
    }
}

#[cfg(feature = "display")]
impl<R, C1: Command<R> + 'static, C2: Command<R> + 'static> fmt::Display for Merged<R, C1, C2> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{cmd1} + {cmd2}", cmd1 = self.cmd1, cmd2 = self.cmd2)
    }
}

/// A command wrapper which always merges with itself.
///
/// This wrapper has an [`id`] of [`u32::max_value`].
///
/// [`id`]: trait.Command.html#method.id
/// [`u32::max_value`]: https://doc.rust-lang.org/std/primitive.u32.html#method.max_value
pub struct Merger<R, C: Command<R> + 'static> {
    cmd: C,
    _marker: PhantomData<Box<dyn Command<R> + 'static>>,
}

impl<R, C: Command<R> + 'static> Merger<R, C> {
    /// Returns a new merger command.
    #[inline]
    pub fn new(cmd: C) -> Merger<R, C> {
        Merger {
            cmd,
            _marker: PhantomData,
        }
    }

    /// Returns the inner command.
    #[inline]
    pub fn into_command(self) -> C {
        self.cmd
    }
}

impl<R, C: Command<R> + 'static> From<C> for Merger<R, C> {
    #[inline]
    fn from(cmd: C) -> Self {
        Merger::new(cmd)
    }
}

impl<R, C: Command<R> + 'static> Command<R> for Merger<R, C> {
    #[inline]
    fn apply(&mut self, receiver: &mut R) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.cmd.apply(receiver)
    }

    #[inline]
    fn undo(&mut self, receiver: &mut R) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.cmd.undo(receiver)
    }

    #[inline]
    fn redo(&mut self, receiver: &mut R) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.cmd.redo(receiver)
    }

    #[inline]
    fn id(&self) -> Option<u32> {
        Some(u32::max_value())
    }
}

impl<R, C: Command<R> + 'static> Debug for Merger<R, C> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Merger").field("cmd", &self.cmd).finish()
    }
}

#[cfg(feature = "display")]
impl<R, C: Command<R> + 'static> fmt::Display for Merger<R, C> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        (&self.cmd as &fmt::Display).fmt(f)
    }
}
