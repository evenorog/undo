use crate::{Command, Merge};

/// A command wrapper used for joining commands.
///
/// The commands are executed in the order they were merged in.
#[derive(Debug)]
pub struct Join<A, B>(A, B);

impl<A, B> Join<A, B> {
    /// Joins the `a` and `b` command.
    pub fn new(a: A, b: B) -> Join<A, B> {
        Join(a, b)
    }

    /// Joins the two commands.
    pub fn join<C>(self, command: C) -> Join<Join<A, B>, C> {
        Join(self, command)
    }

    /// Returns a new command with the provided text.
    pub fn with_text(self, text: impl Into<String>) -> Text<Join<A, B>> {
        Text::new(self, text)
    }

    /// Returns a new command with the provided merge behavior.
    pub fn with_merge(self, merge: Merge) -> Merger<Join<A, B>> {
        Merger::new(self, merge)
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

    fn merge(&self) -> Merge {
        Merge::No
    }

    fn text(&self) -> String {
        format!("{} & {}", self.0.text(), self.1.text())
    }
}

/// A command wrapper with a specified text.
#[derive(Debug)]
pub struct Text<A> {
    command: A,
    text: String,
}

impl<A> Text<A> {
    /// Creates a command with the specified text.
    pub fn new(command: A, text: impl Into<String>) -> Text<A> {
        Text {
            command,
            text: text.into(),
        }
    }

    /// Returns a new command with the provided merge behavior.
    pub fn with_merge(self, merge: Merge) -> Merger<Text<A>> {
        Merger::new(self, merge)
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
#[derive(Debug)]
pub struct Merger<A> {
    command: A,
    merge: Merge,
}

impl<A> Merger<A> {
    /// Creates a command with the specified merge behavior.
    pub fn new(command: A, merge: Merge) -> Merger<A> {
        Merger { command, merge }
    }

    /// Returns a new command with the provided text.
    pub fn with_text(self, text: impl Into<String>) -> Text<Merger<A>> {
        Text::new(self, text)
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
