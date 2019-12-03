use crate::{Command, Merge};
#[cfg(feature = "display")]
use std::fmt;

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
/// let chain = Chain::new(Add('a'), Add('b')).join(Add('c'));
/// record.apply(chain)?;
/// assert_eq!(record.target(), "abc");
/// record.undo().unwrap()?;
/// assert_eq!(record.target(), "");
/// record.redo().unwrap()?;
/// assert_eq!(record.target(), "abc");
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Chain<A, B> {
    join: Join<A, B>,
    merge: Merge,
    #[cfg(feature = "display")]
    text: Option<String>,
}

impl<A, B> Chain<A, B> {
    /// Creates a chain from `a` and `b`.
    #[inline]
    pub const fn new(a: A, b: B) -> Chain<A, B> {
        Chain::with_merge(a, b, Merge::No)
    }

    /// Creates a chain from `a` and `b` with the specified merge behavior.
    ///
    /// By default the chain never merges.
    #[inline]
    pub const fn with_merge(a: A, b: B, merge: Merge) -> Chain<A, B> {
        Chain {
            join: Join(a, b),
            merge,
            #[cfg(feature = "display")]
            text: None,
        }
    }

    /// Creates a chain from `a` and `b` with the specified display text.
    ///
    /// By default the display text will be the display text for every command in the chain.
    ///
    /// Requires the `display` feature to be enabled.
    #[inline]
    #[cfg(feature = "display")]
    pub fn with_text(a: A, b: B, text: impl Into<String>) -> Chain<A, B> {
        Chain {
            join: Join(a, b),
            merge: Merge::No,
            #[cfg(feature = "display")]
            text: Some(text.into()),
        }
    }

    /// Joins the command with the chain and returns the chain.
    #[inline]
    pub fn join<C>(self, c: C) -> Chain<A, Join<B, C>> {
        Chain {
            join: Join(self.join.0, Join(self.join.1, c)),
            merge: self.merge,
            #[cfg(feature = "display")]
            text: self.text,
        }
    }

    /// Sets the merge behavior of the chain.
    #[inline]
    pub fn set_merge(&mut self, merge: Merge) {
        self.merge = merge;
    }

    /// Sets the display text for the chain.
    ///
    /// Requires the `display` feature to be enabled.
    #[inline]
    #[cfg(feature = "display")]
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = Some(text.into());
    }
}

impl<T, A: Command<T>, B: Command<T>> Command<T> for Chain<A, B> {
    #[inline]
    fn apply(&mut self, target: &mut T) -> crate::Result {
        self.join.apply(target)
    }

    #[inline]
    fn undo(&mut self, target: &mut T) -> crate::Result {
        self.join.undo(target)
    }

    #[inline]
    fn redo(&mut self, target: &mut T) -> crate::Result {
        self.join.redo(target)
    }

    #[inline]
    fn merge(&self) -> Merge {
        self.merge
    }
}

#[cfg(feature = "display")]
impl<A: fmt::Display, B: fmt::Display> fmt::Display for Chain<A, B> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.text {
            Some(text) => f.write_str(text),
            None => (&self.join as &dyn fmt::Display).fmt(f),
        }
    }
}

#[derive(Debug)]
pub struct Join<A, B>(pub A, pub B);

impl<T, A: Command<T>, B: Command<T>> Command<T> for Join<A, B> {
    #[inline]
    fn apply(&mut self, target: &mut T) -> crate::Result {
        self.0.apply(target)?;
        self.1.apply(target)
    }

    #[inline]
    fn undo(&mut self, target: &mut T) -> crate::Result {
        self.1.undo(target)?;
        self.0.undo(target)
    }

    #[inline]
    fn redo(&mut self, target: &mut T) -> crate::Result {
        self.0.redo(target)?;
        self.1.redo(target)
    }
}

#[cfg(feature = "display")]
impl<A: fmt::Display, B: fmt::Display> fmt::Display for Join<A, B> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{} + {}", self.0, self.1)
    }
}
