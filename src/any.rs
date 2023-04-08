use crate::Edit;
use alloc::boxed::Box;
use alloc::string::String;
use core::fmt::{self, Debug, Display, Formatter};

/// Any edit type.
///
/// This allows you to use multiple types of edits at the same time
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
/// record.edit(&mut target, Any::new(Push('a')));
/// record.edit(&mut target, Any::new(FromFn::new(|s: &mut String| s.push('b'))));
/// assert_eq!(target, "ab");
/// # }
/// ```
pub struct Any<T, O> {
    edit: Box<dyn Edit<Target = T, Output = O>>,
    string: String,
}

impl<T, O> Any<T, O> {
    /// Creates an `Any` from the provided edit.
    pub fn new<A>(edit: A) -> Any<T, O>
    where
        A: Edit<Target = T, Output = O>,
        A: 'static,
    {
        Any {
            edit: Box::new(edit),
            string: String::new(),
        }
    }

    /// Sets the display message of this edit.
    pub fn set_string(&mut self, str: impl Into<String>) {
        self.string = str.into();
    }
}

impl<T, O> Edit for Any<T, O> {
    type Target = T;
    type Output = O;

    fn edit(&mut self, target: &mut Self::Target) -> Self::Output {
        self.edit.edit(target)
    }

    fn undo(&mut self, target: &mut Self::Target) -> Self::Output {
        self.edit.undo(target)
    }

    fn redo(&mut self, target: &mut Self::Target) -> Self::Output {
        self.edit.redo(target)
    }
}

impl<T, O> Debug for Any<T, O> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Any")
            .field("string", &self.string)
            .finish_non_exhaustive()
    }
}

impl<T, O> Display for Any<T, O> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(&self.string, f)
    }
}
