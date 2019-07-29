use crate::{Command, Merge};
#[cfg(feature = "display")]
use std::fmt;
use std::{
    iter::{FromIterator, IntoIterator},
    vec::IntoIter,
};

/// A chain of merged commands.
///
/// Commands in a chain are all executed in the order they was merged in.
///
/// # Examples
///
/// ```
/// # use undo::*;
/// # struct Add(char);
/// # impl Command<String> for Add {
/// #     fn apply(&mut self, s: &mut String) -> undo::Result {
/// #         s.push(self.0);
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self, s: &mut String) -> undo::Result {
/// #         self.0 = s.pop().ok_or("`s` is empty")?;
/// #         Ok(())
/// #     }
/// # }
/// # fn main() -> undo::Result {
/// let mut record = Record::default();
/// let chain = Chain::with_capacity(2)
///     .join(Add('a'))
///     .join(Add('b'));
/// record.apply(chain)?;
/// assert_eq!(record.as_receiver(), "ab");
/// record.undo().unwrap()?;
/// assert_eq!(record.as_receiver(), "");
/// record.redo().unwrap()?;
/// assert_eq!(record.as_receiver(), "ab");
/// # Ok(())
/// # }
/// ```
pub struct Chain<R> {
    commands: Vec<Box<dyn Command<R>>>,
    merge: Option<Merge>,
    #[cfg(feature = "display")]
    text: Option<String>,
}

impl<R> Chain<R> {
    /// Returns an empty command.
    #[inline]
    pub fn new() -> Chain<R> {
        Chain {
            commands: vec![],
            merge: None,
            #[cfg(feature = "display")]
            text: None,
        }
    }

    /// Returns an empty command with the specified display text.
    #[inline]
    #[cfg(feature = "display")]
    pub fn with_text(text: impl Into<String>) -> Chain<R> {
        Chain {
            commands: vec![],
            merge: None,
            #[cfg(feature = "display")]
            text: Some(text.into()),
        }
    }

    /// Returns an empty command with the specified capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Chain<R> {
        Chain {
            commands: Vec::with_capacity(capacity),
            merge: None,
            #[cfg(feature = "display")]
            text: None,
        }
    }

    /// Reserves capacity for at least `additional` more commands in the chain.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.commands.reserve(additional);
    }

    /// Returns the capacity of the chain.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.commands.capacity()
    }

    /// Returns the amount of commands in the chain.
    #[inline]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns `true` if no commands have been merged.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Merges the command with the chain.
    #[inline]
    pub fn push(&mut self, command: impl Command<R> + 'static) {
        self.commands.push(Box::new(command));
    }

    /// Merges the command with the chain and returns the chain.
    #[inline]
    pub fn join(mut self, command: impl Command<R> + 'static) -> Chain<R> {
        self.push(command);
        self
    }

    /// Sets the merge behavior of the chain.
    ///
    /// By default the merge behavior of the first command in the chain is used,
    /// and it merges if the chain is empty.
    #[inline]
    pub fn set_merge(&mut self, merge: Merge) {
        self.merge = Some(merge);
    }

    /// Sets the display text for the chain.
    ///
    /// By default the display text will be the display text for every command in the chain.
    #[inline]
    #[cfg(feature = "display")]
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = Some(text.into());
    }
}

impl<R> Command<R> for Chain<R> {
    #[inline]
    fn apply(&mut self, receiver: &mut R) -> crate::Result {
        for command in &mut self.commands {
            command.apply(receiver)?;
        }
        Ok(())
    }

    #[inline]
    fn undo(&mut self, receiver: &mut R) -> crate::Result {
        for command in self.commands.iter_mut().rev() {
            command.undo(receiver)?;
        }
        Ok(())
    }

    #[inline]
    fn redo(&mut self, receiver: &mut R) -> crate::Result {
        for command in &mut self.commands {
            command.redo(receiver)?;
        }
        Ok(())
    }

    #[inline]
    fn merge(&self) -> Merge {
        self.merge
            .or_else(|| self.commands.first().map(Command::merge))
            .unwrap_or(Merge::Yes)
    }

    #[inline]
    fn is_dead(&self) -> bool {
        self.commands.iter().any(Command::is_dead)
    }
}

impl<R> Default for Chain<R> {
    #[inline]
    fn default() -> Self {
        Chain::new()
    }
}

impl<R, C: Command<R> + 'static> FromIterator<C> for Chain<R> {
    #[inline]
    fn from_iter<T: IntoIterator<Item = C>>(commands: T) -> Self {
        Chain {
            commands: commands.into_iter().map(|c| Box::new(c) as _).collect(),
            merge: None,
            #[cfg(feature = "display")]
            text: None,
        }
    }
}

impl<R> IntoIterator for Chain<R> {
    type Item = Box<dyn Command<R>>;
    type IntoIter = IntoIter<Self::Item>;

    #[inline]
    fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
        self.commands.into_iter()
    }
}

impl<R, C: Command<R> + 'static> Extend<C> for Chain<R> {
    #[inline]
    fn extend<T: IntoIterator<Item = C>>(&mut self, iter: T) {
        self.commands
            .extend(iter.into_iter().map(|c| Box::new(c) as _));
    }
}

#[cfg(feature = "display")]
impl<R> fmt::Debug for Chain<R> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Chain")
            .field("commands", &self.commands)
            .field("merge", &self.merge)
            .field("text", &self.text)
            .finish()
    }
}

#[cfg(feature = "display")]
impl<R> fmt::Display for Chain<R> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.text {
            Some(text) => f.write_str(text),
            None => {
                for command in &self.commands {
                    writeln!(f, "- {}", command)?;
                }
                Ok(())
            }
        }
    }
}
