use crate::Edit;
use core::fmt::{self, Display, Formatter};

/// Two [`Edit`] commands joined together.
///
/// This is a convenient way to build more complex edit commands from simpler ones,
/// but for more complex edits it is probably better to create a custom edit command.
///
/// # Examples
/// ```
/// # include!("doctest.rs");
/// # fn main() {
/// # use undo::{Record, Join};
/// let mut target = String::new();
/// let mut record = Record::new();
///
/// let abc = Join::new(Add('a'), Add('b')).join(Add('c'));
/// record.edit(&mut target, abc);
/// assert_eq!(target, "abc");
/// record.undo(&mut target);
/// assert_eq!(target, "");
/// record.redo(&mut target);
/// assert_eq!(target, "abc");
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct Join<A, B> {
    a: A,
    b: B,
}

impl<A, B> Join<A, B> {
    /// Creates a new [`Join`] from `a` and `b`.
    pub const fn new(a: A, b: B) -> Self {
        Join { a, b }
    }

    /// Joins `self` with `c`.
    pub const fn join<C>(self, c: C) -> Join<Self, C> {
        Join::new(self, c)
    }
}

impl<A, B> Edit for Join<A, B>
where
    A: Edit,
    B: Edit<Target = A::Target, Output = A::Output>,
{
    type Target = A::Target;
    type Output = A::Output;

    /// The output of a will be discarded.
    fn edit(&mut self, target: &mut A::Target) -> Self::Output {
        self.a.edit(target);
        self.b.edit(target)
    }

    /// The output of b will be discarded.
    fn undo(&mut self, target: &mut A::Target) -> Self::Output {
        self.b.undo(target);
        self.a.undo(target)
    }

    /// The output of a will be discarded.
    fn redo(&mut self, target: &mut A::Target) -> Self::Output {
        self.a.redo(target);
        self.b.redo(target)
    }
}

impl<A, B> Display for Join<A, B>
where
    A: Display,
    B: Display,
{
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{} & {}", self.a, self.b)
    }
}
