use crate::Action;
use std::fmt::{self, Debug, Formatter};
use std::mem;

/// Any action type.
///
/// This allows you to use multiple different actions in a record or history
/// as long as they all share the same target and output type.
pub struct Any<T, O> {
    action: Box<dyn Action<Target = T, Output = O>>,
    message: String,
}

impl<T, O> Any<T, O> {
    /// Creates an `Any` from the provided action.
    pub fn new<A>(action: A) -> Any<T, O>
    where
        A: Action<Target = T, Output = O>,
        A: 'static,
    {
        Any {
            action: Box::new(action),
            message: String::new(),
        }
    }
}

impl<T> Any<T, ()>
where
    Self: 'static,
{
    /// Creates a new `Any` from a function.
    pub fn from_fn<F>(f: F) -> Any<T, ()>
    where
        F: FnMut(&mut T),
        F: 'static,
        T: Clone,
    {
        Any::new(FromFn { f, target: None })
    }

    /// Creates a new `Any` from `self` and `action`.
    ///
    /// `self` will be called first in `apply`.
    pub fn join<A>(self, action: A) -> Any<T, ()>
    where
        A: Action<Target = T, Output = ()>,
        A: 'static,
    {
        Any::new(Join { a: self, b: action })
    }
}

impl<T, E> Any<T, Result<(), E>>
where
    Self: 'static,
{
    /// Creates a new `Any` from a function.
    pub fn from_fn<F>(f: F) -> Any<T, Result<(), E>>
    where
        F: FnMut(&mut T) -> Result<(), E>,
        F: 'static,
        T: Clone,
    {
        Any::new(TryFromFn { f, target: None })
    }

    /// Creates a new `Any` from `self` and `action`.
    ///
    /// `self` will be called first in `apply`.
    pub fn join<A>(self, action: A) -> Any<T, Result<(), E>>
    where
        A: Action<Target = T, Output = Result<(), E>>,
        A: 'static,
    {
        Any::new(TryJoin { a: self, b: action })
    }
}

impl<T, O> Action for Any<T, O> {
    type Target = T;
    type Output = O;

    fn apply(&mut self, target: &mut Self::Target) -> Self::Output {
        self.action.apply(target)
    }

    fn undo(&mut self, target: &mut Self::Target) -> Self::Output {
        self.action.undo(target)
    }

    fn redo(&mut self, target: &mut Self::Target) -> Self::Output {
        self.action.redo(target)
    }
}

impl<T, O> Debug for Any<T, O> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Any")
            .field("message", &self.message)
            .finish_non_exhaustive()
    }
}

struct Join<A, B> {
    a: A,
    b: B,
}

impl<A, B, T> Action for Join<A, B>
where
    A: Action<Target = T, Output = ()>,
    B: Action<Target = T, Output = ()>,
{
    type Target = T;
    type Output = ();

    fn apply(&mut self, target: &mut T) {
        self.a.apply(target);
        self.b.apply(target);
    }

    fn undo(&mut self, target: &mut T) {
        self.b.undo(target);
        self.a.undo(target);
    }

    fn redo(&mut self, target: &mut T) {
        self.a.redo(target);
        self.b.redo(target);
    }
}

struct TryJoin<A, B> {
    a: A,
    b: B,
}

impl<A, B, T, E> Action for TryJoin<A, B>
where
    A: Action<Target = T, Output = Result<(), E>>,
    B: Action<Target = T, Output = Result<(), E>>,
{
    type Target = T;
    type Output = Result<(), E>;

    fn apply(&mut self, target: &mut T) -> Self::Output {
        self.a.apply(target)?;
        self.b.apply(target)
    }

    fn undo(&mut self, target: &mut T) -> Self::Output {
        self.b.undo(target)?;
        self.a.undo(target)
    }

    fn redo(&mut self, target: &mut T) -> Self::Output {
        self.a.redo(target)?;
        self.b.redo(target)
    }
}

struct FromFn<F, T> {
    f: F,
    target: Option<T>,
}

impl<F, T> Action for FromFn<F, T>
where
    F: FnMut(&mut T),
    T: Clone,
{
    type Target = T;
    type Output = ();

    fn apply(&mut self, target: &mut Self::Target) {
        self.target = Some(target.clone());
        (self.f)(target)
    }

    fn undo(&mut self, target: &mut Self::Target) {
        let old = self.target.as_mut().unwrap();
        mem::swap(old, target);
    }

    fn redo(&mut self, target: &mut Self::Target) {
        let new = self.target.as_mut().unwrap();
        mem::swap(new, target);
    }
}

struct TryFromFn<F, T> {
    f: F,
    target: Option<T>,
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
        let old = self.target.as_mut().unwrap();
        mem::swap(old, target);
        Ok(())
    }

    fn redo(&mut self, target: &mut Self::Target) -> Self::Output {
        let new = self.target.as_mut().unwrap();
        mem::swap(new, target);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{Any, Record};

    #[test]
    fn join() {
        let mut target = String::new();
        let mut record = Record::new();
        let a = Any::<String, ()>::from_fn(|s| s.push('a'));
        let b = Any::<String, ()>::from_fn(|s| s.push('b'));
        let c = Any::<String, ()>::from_fn(|s| s.push('c'));
        let joined = a.join(b).join(c);
        record.apply(&mut target, joined);
        assert_eq!(target, "abc");
        record.undo(&mut target).unwrap();
        assert_eq!(target, "");
        record.redo(&mut target).unwrap();
        assert_eq!(target, "abc");
    }

    #[test]
    fn from_fn() {
        let mut target = String::new();
        let mut record = Record::new();
        record.apply(&mut target, Any::<String, ()>::from_fn(|s| s.push('a')));
        record.apply(&mut target, Any::<String, ()>::from_fn(|s| s.push('b')));
        record.apply(&mut target, Any::<String, ()>::from_fn(|s| s.push('c')));
        assert_eq!(target, "abc");
        record.undo(&mut target).unwrap();
        record.undo(&mut target).unwrap();
        record.undo(&mut target).unwrap();
        assert_eq!(target, "");
        record.redo(&mut target).unwrap();
        record.redo(&mut target).unwrap();
        record.redo(&mut target).unwrap();
        assert_eq!(target, "abc");
    }
}
