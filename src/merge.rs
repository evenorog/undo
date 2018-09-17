use std::error::Error;
use std::fmt;
use std::marker::PhantomData;
use {Command, Merge};

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
///     let cmd = merge![Add('a'), Add('b'), Add('c')];
///     record.apply(cmd)?;
///     assert_eq!(record.as_receiver(), "abc");
///
///     record.undo().unwrap()?;
///     assert_eq!(record.as_receiver(), "");
///
///     record.redo().unwrap()?;
///     assert_eq!(record.as_receiver(), "abc");
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! merge {
    ($cmd1:expr, $cmd2:expr) => (
        $crate::Merged::new($cmd1, $cmd2)
    );
    ($cmd1:expr, $cmd2:expr, $($tail:expr),+) => (
        merge![$crate::Merged::new($cmd1, $cmd2), $($tail),*]
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
    fn merge(&self) -> Merge {
        self.cmd1.merge()
    }
}

impl<R, C1: Command<R> + 'static, C2: Command<R> + 'static> fmt::Debug for Merged<R, C1, C2> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Merged")
            .field("cmd1", &self.cmd1)
            .field("cmd2", &self.cmd2)
            .finish()
    }
}

#[cfg(feature = "display")]
impl<R, C1: Command<R> + 'static, C2: Command<R> + 'static> fmt::Display for Merged<R, C1, C2> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{cmd1} + {cmd2}", cmd1 = self.cmd1, cmd2 = self.cmd2)
    }
}
