#![allow(dead_code)]

use crate::Action;
use alloc::boxed::Box;
use core::fmt::{self, Debug, Formatter};

/// Any action type.
///
/// This allows you to use multiple different actions in a record or history
/// as long as they all share the same target and output type.
pub struct AnyAction<T, O> {
    action: Box<dyn Action<Target = T, Output = O>>,
}

impl<T, O> AnyAction<T, O> {
    /// Creates an `AnyAction` from the provided action.
    pub fn new<A>(action: A) -> AnyAction<T, O>
    where
        A: Action<Target = T, Output = O>,
        A: 'static,
    {
        AnyAction {
            action: Box::new(action),
        }
    }
}

impl<T> AnyAction<T, ()>
where
    Self: 'static,
{
    /// Creates a new any action from `self` and `action`.
    ///
    /// `self` will be called first in `apply`.
    pub fn join<A>(self, action: A) -> AnyAction<T, ()>
    where
        A: Action<Target = T, Output = ()>,
        A: 'static,
    {
        AnyAction::new(Join { a: self, b: action })
    }
}

impl<T, E> AnyAction<T, Result<(), E>>
where
    Self: 'static,
{
    /// Creates a new any action from `self` and `action`.
    ///
    /// `self` will be called first in `apply`.
    pub fn join<A>(self, action: A) -> AnyAction<T, Result<(), E>>
    where
        A: Action<Target = T, Output = Result<(), E>>,
        A: 'static,
    {
        AnyAction::new(TryJoin { a: self, b: action })
    }
}

impl<T, O> Action for AnyAction<T, O> {
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

impl<T, O> Debug for AnyAction<T, O> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("AnyAction").finish_non_exhaustive()
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
