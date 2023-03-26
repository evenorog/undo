use crate::Action;
use std::fmt::{self, Debug, Formatter};
use std::mem;

/// Action made from a function.
///
/// # Examples
/// ```
/// # include!("doctest.rs");
/// # fn main() {
/// # use undo::{Any, Record, FromFn};
/// let mut target = String::new();
/// let mut record = Record::new();
///
/// let a: fn(&mut String) = |s| s.push('a');
/// let b: fn(&mut String) = |s| s.push('b');
/// record.apply(&mut target, FromFn::new(a));
/// record.apply(&mut target, FromFn::new(b));
/// assert_eq!(target, "ab");
///
/// record.undo(&mut target);
/// record.undo(&mut target);
/// assert_eq!(target, "");
///
/// record.redo(&mut target);
/// record.redo(&mut target);
/// assert_eq!(target, "ab");
/// # }
/// ```
#[derive(Clone)]
pub struct FromFn<F, T> {
    f: F,
    target: Option<T>,
}

impl<F, T> FromFn<F, T> {
    /// Creates a new `FromFn` from `f`.
    pub const fn new(f: F) -> Self {
        FromFn { f, target: None }
    }
}

impl<F, T> Action for FromFn<F, T>
where
    F: FnMut(&mut T),
    T: Clone,
{
    type Target = T;
    type Output = ();

    fn apply(&mut self, target: &mut Self::Target) -> Self::Output {
        self.target = Some(target.clone());
        (self.f)(target)
    }

    fn undo(&mut self, target: &mut Self::Target) -> Self::Output {
        if let Some(old) = self.target.as_mut() {
            mem::swap(old, target);
        }
    }

    fn redo(&mut self, target: &mut Self::Target) -> Self::Output {
        if let Some(new) = self.target.as_mut() {
            mem::swap(new, target);
        }
    }
}

impl<F, T> Debug for FromFn<F, T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("FromFn").finish_non_exhaustive()
    }
}

/// Action made from a fallible function.
///
/// Same as [`FromFn`] but for functions that outputs [`Result`].
#[derive(Clone, Debug)]
pub struct TryFromFn<F, T> {
    f: F,
    target: Option<T>,
}

impl<F, T> TryFromFn<F, T> {
    /// Creates a new `TryFromFn` from `f`.
    pub const fn new(f: F) -> Self {
        TryFromFn { f, target: None }
    }
}

impl<F, T, E> Action for TryFromFn<F, T>
where
    F: FnMut(&mut T) -> Result<(), E>,
    T: Clone,
{
    type Target = T;
    type Output = Result<(), E>;

    fn apply(&mut self, target: &mut Self::Target) -> Self::Output {
        self.target = Some(target.clone());
        (self.f)(target)
    }

    fn undo(&mut self, target: &mut Self::Target) -> Self::Output {
        if let Some(old) = self.target.as_mut() {
            mem::swap(old, target);
        }
        Ok(())
    }

    fn redo(&mut self, target: &mut Self::Target) -> Self::Output {
        if let Some(new) = self.target.as_mut() {
            mem::swap(new, target);
        }
        Ok(())
    }
}
