#![allow(dead_code)]

use crate::Action;
use alloc::boxed::Box;
use core::fmt::{self, Debug, Formatter};

/// Any action type.
///
/// This allows you to use multiple different actions in a record or history
/// as long as they all share the same target and output type.
pub struct AnyAction<T, O> {
    id: u64,
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
            id: 0,
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
        f.debug_struct("AnyAction")
            .field("id", &self.id)
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

#[cfg(test)]
mod tests {
    use super::AnyAction;
    use crate::{Action, Record};
    use alloc::string::String;

    struct Push(char);

    impl Action for Push {
        type Target = String;
        type Output = ();

        fn apply(&mut self, s: &mut String) {
            s.push(self.0);
        }

        fn undo(&mut self, s: &mut String) {
            self.0 = s.pop().unwrap();
        }
    }

    #[test]
    fn any() {
        let mut target = String::new();
        let mut record = Record::new();
        record.apply(&mut target, AnyAction::new(Push('a')));
        assert_eq!(target, "a");
        record.undo(&mut target).unwrap();
        assert_eq!(target, "");
        record.redo(&mut target).unwrap();
        assert_eq!(target, "a");
    }
}
