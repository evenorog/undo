use crate::{Command, Merge};
use std::error::Error;
use std::fmt;
use std::iter::{FromIterator, IntoIterator};
use std::vec::IntoIter;

/// Macro for merging commands.
///
/// # Examples
/// ```
/// # use std::error::Error;
/// # use undo::{merge, Command, Record};
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
/// fn main() -> undo::Result<String> {
///     let mut record = Record::default();
///     record.apply(merge![Add('a'), Add('b'), Add('c')])?;
///     assert_eq!(record.as_receiver(), "abc");
///     record.undo().unwrap()?;
///     assert_eq!(record.as_receiver(), "");
///     record.redo().unwrap()?;
///     assert_eq!(record.as_receiver(), "abc");
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! merge {
    ($cmd1:expr, $cmd2:expr, $($commands:expr),*) => {{
        let mut merged = $crate::Merged::new($cmd1, $cmd2);
        $(merged.push($commands);)*
        merged
    }};
}

/// The result of merging commands.
///
/// The [`merge!`](macro.merge.html) macro can be used for convenience when merging commands.
pub struct Merged<R> {
    commands: Vec<Box<dyn Command<R> + 'static>>,
    #[cfg(feature = "display")]
    summary: Option<String>,
}

impl<R> Merged<R> {
    /// Merges `cmd1` and `cmd2` into a single command.
    #[inline]
    pub fn new(cmd1: impl Command<R> + 'static, cmd2: impl Command<R> + 'static) -> Merged<R> {
        Merged {
            commands: vec![Box::new(cmd1), Box::new(cmd2)],
            #[cfg(feature = "display")]
            summary: None,
        }
    }

    /// Merges `self` with `command`.
    #[inline]
    pub fn push(&mut self, command: impl Command<R> + 'static) {
        self.commands.push(Box::new(command));
    }

    /// Merges `self` with `command` and returns the merged command.
    #[inline]
    pub fn join(mut self, command: impl Command<R> + 'static) -> Merged<R> {
        self.push(command);
        self
    }

    /// Sets a summary for the two merged commands. This overrides the default display text.
    #[inline]
    #[cfg(feature = "display")]
    pub fn set_summary(&mut self, summary: impl Into<String>) {
        self.summary = Some(summary.into());
    }
}

impl<R> Command<R> for Merged<R> {
    #[inline]
    fn apply(&mut self, receiver: &mut R) -> Result<(), Box<dyn Error + Send + Sync>> {
        for command in &mut self.commands {
            command.apply(receiver)?;
        }
        Ok(())
    }

    #[inline]
    fn undo(&mut self, receiver: &mut R) -> Result<(), Box<dyn Error + Send + Sync>> {
        for command in self.commands.iter_mut().rev() {
            command.undo(receiver)?;
        }
        Ok(())
    }

    #[inline]
    fn redo(&mut self, receiver: &mut R) -> Result<(), Box<dyn Error + Send + Sync>> {
        for command in &mut self.commands {
            command.redo(receiver)?;
        }
        Ok(())
    }

    #[inline]
    fn merge(&self) -> Merge {
        self.commands.first().map_or(Merge::Always, |c| c.merge())
    }
}

impl<R> Default for Merged<R> {
    #[inline]
    fn default() -> Self {
        Merged {
            commands: Vec::default(),
            #[cfg(feature = "display")]
            summary: None,
        }
    }
}

impl<R, C: Command<R> + 'static> FromIterator<C> for Merged<R> {
    #[inline]
    fn from_iter<T: IntoIterator<Item = C>>(commands: T) -> Self {
        Merged {
            commands: commands.into_iter().map(|c| Box::new(c) as _).collect(),
            ..Default::default()
        }
    }
}

impl<R> IntoIterator for Merged<R> {
    type Item = Box<dyn Command<R> + 'static>;
    type IntoIter = IntoIter<Self::Item>;

    #[inline]
    fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
        self.commands.into_iter()
    }
}

impl<R, C: Command<R> + 'static> Extend<C> for Merged<R> {
    #[inline]
    fn extend<T: IntoIterator<Item = C>>(&mut self, iter: T) {
        self.commands
            .extend(iter.into_iter().map(|c| Box::new(c) as _));
    }
}

impl<R> fmt::Debug for Merged<R> {
    #[inline]
    #[cfg(not(feature = "display"))]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Merged")
            .field("commands", &self.commands)
            .finish()
    }

    #[inline]
    #[cfg(feature = "display")]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Merged")
            .field("commands", &self.commands)
            .field("summary", &self.summary)
            .finish()
    }
}

#[cfg(feature = "display")]
impl<R> fmt::Display for Merged<R> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.summary {
            Some(summary) => f.write_str(summary),
            None => {
                if let Some((first, commands)) = self.commands.split_first() {
                    (first as &dyn fmt::Display).fmt(f)?;
                    for command in commands {
                        write!(f, "\n\n{}", command)?;
                    }
                }
                Ok(())
            }
        }
    }
}
