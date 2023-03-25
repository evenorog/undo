use crate::Action;
use std::mem;

/// Action made from a function.
#[derive(Clone, Debug)]
pub struct FromFn<F, T> {
    f: F,
    target: Option<T>,
}

impl<F, T> FromFn<F, T> {
    /// Creates a new `FromFn` from `f`.
    pub fn new(f: F) -> Self {
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

/// Action made from a fallible function.
#[derive(Clone, Debug)]
pub struct TryFromFn<F, T> {
    f: F,
    target: Option<T>,
}

impl<F, T> TryFromFn<F, T> {
    /// Creates a new `TryFromFn` from `f`.
    pub fn new(f: F) -> Self {
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
