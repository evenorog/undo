use std::fmt::{self, Debug, Formatter};
use std::{error, marker, u32};
use Command;

/// A command wrapper which always merges with itself.
///
/// This wrapper has an [`id`] of `u32::MAX`.
///
/// [`id`]: trait.Command.html#method.id
pub struct Merger<R, C: Command<R> + 'static> {
    cmd: C,
    _marker: marker::PhantomData<Box<dyn Command<R> + 'static>>,
}

impl<R, C: Command<R> + 'static> Merger<R, C> {
    /// Returns a new merger command.
    #[inline]
    pub fn new(cmd: C) -> Merger<R, C> {
        Merger {
            cmd,
            _marker: marker::PhantomData,
        }
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
    fn apply(&mut self, receiver: &mut R) -> Result<(), Box<dyn error::Error>> {
        self.cmd.apply(receiver)
    }

    #[inline]
    fn undo(&mut self, receiver: &mut R) -> Result<(), Box<dyn error::Error>> {
        self.cmd.undo(receiver)
    }

    #[inline]
    fn redo(&mut self, receiver: &mut R) -> Result<(), Box<dyn error::Error>> {
        self.cmd.redo(receiver)
    }

    #[inline]
    fn id(&self) -> Option<u32> {
        Some(u32::MAX)
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

/// The result of merging two commands.
pub struct Merged<R, C1: Command<R> + 'static, C2: Command<R> + 'static> {
    cmd1: C1,
    cmd2: C2,
    _marker: marker::PhantomData<Box<dyn Command<R> + 'static>>,
}

impl<R, C1: Command<R> + 'static, C2: Command<R> + 'static> Merged<R, C1, C2> {
    /// Merges `cmd1` and `cmd2` into a single command.
    #[inline]
    pub fn new(cmd1: C1, cmd2: C2) -> Merged<R, C1, C2> {
        Merged {
            cmd1,
            cmd2,
            _marker: marker::PhantomData,
        }
    }
}

impl<R, C1: Command<R> + 'static, C2: Command<R> + 'static> Command<R> for Merged<R, C1, C2> {
    #[inline]
    fn apply(&mut self, receiver: &mut R) -> Result<(), Box<dyn error::Error>> {
        self.cmd1.apply(receiver)?;
        self.cmd2.apply(receiver)
    }

    #[inline]
    fn undo(&mut self, receiver: &mut R) -> Result<(), Box<dyn error::Error>> {
        self.cmd2.undo(receiver)?;
        self.cmd1.undo(receiver)
    }

    #[inline]
    fn redo(&mut self, receiver: &mut R) -> Result<(), Box<dyn error::Error>> {
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
