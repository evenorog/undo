use crate::Action;
use core::fmt::{self, Display, Formatter};

/// Two actions joined together.
///
/// Can be used to build more complex actions from simpler ones.
///
/// # Examples
/// ```
/// # include!("doctest.rs");
/// # fn main() {
/// # use undo::{Record, Join};
/// let mut target = String::new();
/// let mut record = Record::new();
///
/// let abc = Join::new(Push('a'), Push('b')).join(Push('c'));
/// record.apply(&mut target, abc);
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
    /// Creates a new `Join` from `a` and `b`.
    pub const fn new(a: A, b: B) -> Self {
        Join { a, b }
    }

    /// Joins `self` with `c`.
    pub const fn join<C>(self, c: C) -> Join<Self, C> {
        Join::new(self, c)
    }
}

impl<A, B> Action for Join<A, B>
where
    A: Action,
    B: Action<Target = A::Target, Output = A::Output>,
{
    type Target = A::Target;
    type Output = A::Output;

    /// The output of a will be discarded.
    fn apply(&mut self, target: &mut A::Target) -> Self::Output {
        self.a.apply(target);
        self.b.apply(target)
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

/// Joins two fallible actions together.
///
/// Same as [`Join`] but for actions that outputs [`Result`].
#[derive(Clone, Debug)]
pub struct TryJoin<A, B> {
    a: A,
    b: B,
}

impl<A, B> TryJoin<A, B> {
    /// Creates a new `TryJoin` from `a` and `b`.
    pub const fn new(a: A, b: B) -> Self {
        TryJoin { a, b }
    }

    /// Joins `self` with `c`.
    pub const fn join<C>(self, c: C) -> TryJoin<Self, C> {
        TryJoin::new(self, c)
    }
}

impl<A, B, T, E> Action for TryJoin<A, B>
where
    A: Action<Output = Result<T, E>>,
    B: Action<Target = A::Target, Output = A::Output>,
{
    type Target = A::Target;
    type Output = A::Output;

    /// The output of a will be discarded if success.
    fn apply(&mut self, target: &mut A::Target) -> Self::Output {
        self.a.apply(target)?;
        self.b.apply(target)
    }

    /// The output of b will be discarded if success.
    fn undo(&mut self, target: &mut A::Target) -> Self::Output {
        self.b.undo(target)?;
        self.a.undo(target)
    }

    /// The output of a will be discarded if success.
    fn redo(&mut self, target: &mut A::Target) -> Self::Output {
        self.a.redo(target)?;
        self.b.redo(target)
    }
}

impl<A, B> Display for TryJoin<A, B>
where
    A: Display,
    B: Display,
{
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{} & {}", self.a, self.b)
    }
}
