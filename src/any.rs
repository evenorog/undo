use crate::Action;
use std::fmt::{self, Debug, Display, Formatter};

/// Any action type.
///
/// This allows you to use multiple types of actions at the same time
/// as long as they all share the same target and output type.
///
/// # Examples
/// ```
/// # include!("doctest.rs");
/// # fn main() {
/// # use undo::{Any, Record, FromFn};
/// let mut target = String::new();
/// let mut record = Record::new();
///
/// record.apply(&mut target, Any::new(Push('a')));
/// record.apply(&mut target, Any::new(FromFn::new(|s: &mut String| s.push('b'))));
/// assert_eq!(target, "ab");
/// # }
/// ```
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

impl<T, O> Display for Any<T, O> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(&self.message, f)
    }
}
