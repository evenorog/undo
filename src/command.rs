use crate::{Command, Merge};

/// A command wrapper with a specified text.
///
/// Requires the `display` feature to be enabled.
#[derive(Clone, Debug)]
pub struct Text<C> {
    command: C,
    text: String,
}

impl<C> Text<C> {
    /// Creates a command with the specified text.
    pub fn new(command: C, text: impl Into<String>) -> Text<C> {
        Text {
            command,
            text: text.into(),
        }
    }
}

impl<T, C: Command<T>> Command<T> for Text<C> {
    fn apply(&mut self, target: &mut T) -> crate::Result {
        self.command.apply(target)
    }

    fn undo(&mut self, target: &mut T) -> crate::Result {
        self.command.undo(target)
    }

    fn redo(&mut self, target: &mut T) -> crate::Result {
        self.command.redo(target)
    }

    fn merge(&self) -> Merge {
        self.command.merge()
    }

    fn text(&self) -> String {
        self.text.clone()
    }
}

/// A command wrapper with a specified merge behavior.
#[derive(Clone, Debug)]
pub struct Merger<C> {
    command: C,
    merge: Merge,
}

impl<C> Merger<C> {
    /// Creates a command with the specified merge behavior.
    pub fn new(command: C, merge: Merge) -> Merger<C> {
        Merger { command, merge }
    }
}

impl<T, C: Command<T>> Command<T> for Merger<C> {
    fn apply(&mut self, target: &mut T) -> crate::Result {
        self.command.apply(target)
    }

    fn undo(&mut self, target: &mut T) -> crate::Result {
        self.command.undo(target)
    }

    fn redo(&mut self, target: &mut T) -> crate::Result {
        self.command.redo(target)
    }

    fn merge(&self) -> Merge {
        self.merge
    }

    fn text(&self) -> String {
        self.command.text()
    }
}

/// A command wrapper used for joining commands.
///
/// The commands are executed in the order they were merged in.
#[derive(Clone, Debug)]
pub struct Join<A, B>(A, B);

impl<A, B> Join<A, B> {
    /// Joins the `a` and `b` command.
    pub fn new(a: A, b: B) -> Join<A, B> {
        Join(a, b)
    }
}

impl<T, A: Command<T>, B: Command<T>> Command<T> for Join<A, B> {
    fn apply(&mut self, target: &mut T) -> crate::Result {
        self.0.apply(target)?;
        self.1.apply(target)
    }

    fn undo(&mut self, target: &mut T) -> crate::Result {
        self.1.undo(target)?;
        self.0.undo(target)
    }

    fn redo(&mut self, target: &mut T) -> crate::Result {
        self.0.redo(target)?;
        self.1.redo(target)
    }

    fn text(&self) -> String {
        format!("{} & {}", self.0.text(), self.1.text())
    }
}
