#![allow(dead_code)]

use crate::Action;
use alloc::boxed::Box;
use core::fmt::{self, Debug, Formatter};

/// Any action type.
///
/// This allows you to use multiple different actions in a record or history
/// as long as they all share the same target, output, and error type.
pub struct AnyAction<T, O, E> {
    id: u64,
    action: Box<dyn Action<Target = T, Output = O, Error = E>>,
}

impl<T, O, E> AnyAction<T, O, E> {
    /// Creates an `AnyAction` from the provided action.
    pub fn new<A>(action: A) -> AnyAction<T, O, E>
    where
        A: Action<Target = T, Output = O, Error = E>,
        A: 'static,
    {
        AnyAction {
            id: 0,
            action: Box::new(action),
        }
    }
}

impl<T, E> AnyAction<T, (), E>
where
    Self: 'static,
{
    /// Creates a new any action from `self` and `action`.
    ///
    /// `self` will be called first in `apply`.
    pub fn join<A>(self, action: A) -> AnyAction<T, (), E>
    where
        A: Action<Target = T, Output = (), Error = E>,
        A: 'static,
    {
        AnyAction::new(Join { a: self, b: action })
    }
}

impl<T, O, E> Action for AnyAction<T, O, E> {
    type Target = T;
    type Output = O;
    type Error = E;

    fn apply(&mut self, target: &mut Self::Target) -> crate::Result<Self> {
        self.action.apply(target)
    }

    fn undo(&mut self, target: &mut Self::Target) -> crate::Result<Self> {
        self.action.undo(target)
    }

    fn redo(&mut self, target: &mut Self::Target) -> crate::Result<Self> {
        self.action.redo(target)
    }
}

impl<T, O, E> Debug for AnyAction<T, O, E> {
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

impl<A, B, T, E> Action for Join<A, B>
where
    A: Action<Target = T, Output = (), Error = E>,
    B: Action<Target = T, Output = (), Error = E>,
{
    type Target = T;
    type Output = ();
    type Error = E;

    fn apply(&mut self, target: &mut T) -> crate::Result<Self> {
        self.a.apply(target)?;
        self.b.apply(target)
    }

    fn undo(&mut self, target: &mut T) -> crate::Result<Self> {
        self.b.undo(target)?;
        self.a.undo(target)
    }

    fn redo(&mut self, target: &mut T) -> crate::Result<Self> {
        self.a.redo(target)?;
        self.b.redo(target)
    }
}

#[cfg(test)]
mod tests {
    use super::AnyAction;
    use crate::{Action, Record, Result};
    use alloc::string::String;

    struct Push(char);

    impl Action for Push {
        type Target = String;
        type Output = ();
        type Error = &'static str;

        fn apply(&mut self, s: &mut String) -> Result<Push> {
            s.push(self.0);
            Ok(())
        }

        fn undo(&mut self, s: &mut String) -> Result<Push> {
            self.0 = s.pop().ok_or("s is empty")?;
            Ok(())
        }
    }

    #[test]
    fn any() {
        let mut target = String::new();
        let mut record = Record::new();
        record
            .apply(&mut target, AnyAction::new(Push('a')))
            .unwrap();
        assert_eq!(target, "a");
        record.undo(&mut target).unwrap().unwrap();
        assert_eq!(target, "");
        record.redo(&mut target).unwrap().unwrap();
        assert_eq!(target, "a");
    }
}
