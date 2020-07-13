use crate::{Command, Merge};
use std::fmt::{self, Debug, Formatter};

/// Creates a command from the provided function.
///
/// The undo functionality is provided by cloning the original data before editing it.
///
/// # Examples
/// ```
/// # use undo::*;
/// # fn main() -> undo::Result {
/// let mut record = Record::default();
/// record.apply(undo::from_fn(|s: &mut String| s.push('a')))?;
/// record.apply(undo::from_fn(|s: &mut String| s.push('b')))?;
/// record.apply(undo::from_fn(|s: &mut String| s.push('c')))?;
/// assert_eq!(record.target(), "abc");
/// record.undo()?;
/// record.undo()?;
/// record.undo()?;
/// assert_eq!(record.target(), "");
/// record.redo()?;
/// record.redo()?;
/// record.redo()?;
/// assert_eq!(record.target(), "abc");
/// # Ok(())
/// # }
/// ```
pub fn from_fn<T, F>(f: F) -> FromFn<T, F> {
    FromFn { f, target: None }
}

/// A command wrapper created from a function.
///
/// Created by the [`from_fn`](fn.from_fn.html) function.
pub struct FromFn<T: 'static, F: 'static> {
    f: F,
    target: Option<T>,
}

impl<T, F> FromFn<T, F> {
    /// Returns a new command with the provided text.
    pub fn with_text(self, text: impl Into<String>) -> WithText<FromFn<T, F>> {
        with_text(self, text)
    }

    /// Returns a new command with the provided merge behavior.
    pub fn with_merge(self, merge: Merge) -> WithMerge<FromFn<T, F>> {
        with_merge(self, merge)
    }
}

impl<T: Debug + Clone, F: FnMut(&mut T)> Command<T> for FromFn<T, F> {
    fn apply(&mut self, target: &mut T) -> crate::Result {
        self.target = Some(target.clone());
        (self.f)(target);
        Ok(())
    }

    fn undo(&mut self, target: &mut T) -> crate::Result {
        *target = self.target.take().unwrap();
        Ok(())
    }
}

impl<T: Debug, F> Debug for FromFn<T, F> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Fn").field("target", &self.target).finish()
    }
}

/// Joins the `a` and `b` command.
///
/// The commands are executed in the order they were merged in.
pub fn join<A, B>(a: A, b: B) -> Join<A, B> {
    Join(a, b)
}

/// A command wrapper used for joining commands.
///
/// Created by the [`join`](fn.join.html) function.
#[derive(Debug)]
pub struct Join<A, B>(A, B);

impl<A, B> Join<A, B> {
    /// Joins the two commands.
    pub fn join<C>(self, command: C) -> Join<Join<A, B>, C> {
        Join(self, command)
    }

    /// Returns a new command with the provided text.
    pub fn with_text(self, text: impl Into<String>) -> WithText<Join<A, B>> {
        with_text(self, text)
    }

    /// Returns a new command with the provided merge behavior.
    pub fn with_merge(self, merge: Merge) -> WithMerge<Join<A, B>> {
        with_merge(self, merge)
    }
}

impl<T, A: Command<T>, B: Command<T>> Command<T> for Join<A, B> {
    fn apply(&mut self, target: &mut T) -> crate::Result {
        self.0.apply(target)?;
        self.1.apply(target)
    }

    fn undo(&mut self, target: &mut T) -> crate::Result {
        self.1.undo(target)?;
        self.0.undo(target)
    }

    fn redo(&mut self, target: &mut T) -> crate::Result {
        self.0.redo(target)?;
        self.1.redo(target)
    }

    fn merge(&self) -> Merge {
        Merge::No
    }

    fn text(&self) -> String {
        format!("{} & {}", self.0.text(), self.1.text())
    }
}

/// Creates a command wrapper with the specified text.
pub fn with_text<A>(command: A, text: impl Into<String>) -> WithText<A> {
    WithText {
        command,
        text: text.into(),
    }
}

/// A command wrapper with a specified text.
///
/// Created by the [`with_text`](fn.with_text.html) function.
#[derive(Debug)]
pub struct WithText<A> {
    command: A,
    text: String,
}

impl<A> WithText<A> {
    /// Returns a new command with the provided merge behavior.
    pub fn with_merge(self, merge: Merge) -> WithMerge<WithText<A>> {
        with_merge(self, merge)
    }
}

impl<T, C: Command<T>> Command<T> for WithText<C> {
    fn apply(&mut self, target: &mut T) -> crate::Result {
        self.command.apply(target)
    }

    fn undo(&mut self, target: &mut T) -> crate::Result {
        self.command.undo(target)
    }

    fn redo(&mut self, target: &mut T) -> crate::Result {
        self.command.redo(target)
    }

    fn merge(&self) -> Merge {
        self.command.merge()
    }

    fn text(&self) -> String {
        self.text.clone()
    }
}

/// Creates a command wrapper with the specified merge behavior.
pub fn with_merge<A>(command: A, merge: Merge) -> WithMerge<A> {
    WithMerge { command, merge }
}

/// A command wrapper with a specified merge behavior.
///
/// Created by the [`with_merge`](fn.with_merge.html) function.
#[derive(Debug)]
pub struct WithMerge<A> {
    command: A,
    merge: Merge,
}

impl<A> WithMerge<A> {
    /// Returns a new command with the provided text.
    pub fn with_text(self, text: impl Into<String>) -> WithText<WithMerge<A>> {
        with_text(self, text)
    }
}

impl<T, C: Command<T>> Command<T> for WithMerge<C> {
    fn apply(&mut self, target: &mut T) -> crate::Result {
        self.command.apply(target)
    }

    fn undo(&mut self, target: &mut T) -> crate::Result {
        self.command.undo(target)
    }

    fn redo(&mut self, target: &mut T) -> crate::Result {
        self.command.redo(target)
    }

    fn merge(&self) -> Merge {
        self.merge
    }

    fn text(&self) -> String {
        self.command.text()
    }
}
