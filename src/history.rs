//! A history of commands.

use crate::{format::Format, At, Command, Entry, Record, Result, Signal};
use alloc::{
    boxed::Box,
    collections::{BTreeMap, VecDeque},
    string::{String, ToString},
    vec,
    vec::Vec,
};
#[cfg(feature = "chrono")]
use chrono::{DateTime, TimeZone};
use core::fmt::{self, Write};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A history of commands.
///
/// Unlike [Record](struct.Record.html) which maintains a linear undo history, History maintains an undo tree
/// containing every edit made to the target.
///
/// # Examples
/// ```
/// # use undo::{Command, History};
/// # struct Add(char);
/// # impl Command for Add {
/// #     type Target = String;
/// #     type Error = &'static str;
/// #     fn apply(&mut self, s: &mut String) -> undo::Result<Add> {
/// #         s.push(self.0);
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self, s: &mut String) -> undo::Result<Add> {
/// #         self.0 = s.pop().ok_or("s is empty")?;
/// #         Ok(())
/// #     }
/// # }
/// # fn main() -> undo::Result<Add> {
/// let mut target = String::new();
/// let mut history = History::new();
/// history.apply(&mut target, Add('a'))?;
/// history.apply(&mut target, Add('b'))?;
/// history.apply(&mut target, Add('c'))?;
/// let abc = history.branch();
/// history.go_to(&mut target, abc, 1).unwrap()?;
/// history.apply(&mut target, Add('f'))?;
/// history.apply(&mut target, Add('g'))?;
/// assert_eq!(target, "afg");
/// history.go_to(&mut target, abc, 3).unwrap()?;
/// assert_eq!(target, "abc");
/// # Ok(())
/// # }
/// ```
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(bound(serialize = "C: Serialize", deserialize = "C: Deserialize<'de>"))
)]
#[derive(Clone)]
pub struct History<C, F = Box<dyn FnMut(Signal)>> {
    root: usize,
    next: usize,
    pub(crate) saved: Option<At>,
    pub(crate) record: Record<C, F>,
    pub(crate) branches: BTreeMap<usize, Branch<C>>,
}

impl<C> History<C> {
    /// Returns a new history.
    pub fn new() -> History<C> {
        History::from(Record::new())
    }
}

impl<C, F> History<C, F> {
    /// Reserves capacity for at least `additional` more commands.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    pub fn reserve(&mut self, additional: usize) {
        self.record.reserve(additional);
    }

    /// Returns the capacity of the history.
    pub fn capacity(&self) -> usize {
        self.record.capacity()
    }

    /// Shrinks the capacity of the history as much as possible.
    pub fn shrink_to_fit(&mut self) {
        self.record.shrink_to_fit();
    }

    /// Returns the number of commands in the current branch of the history.
    pub fn len(&self) -> usize {
        self.record.len()
    }

    /// Returns `true` if the current branch of the history is empty.
    pub fn is_empty(&self) -> bool {
        self.record.is_empty()
    }

    /// Returns the limit of the history.
    pub fn limit(&self) -> usize {
        self.record.limit()
    }

    /// Sets how the signal should be handled when the state changes.
    ///
    /// The previous slot is returned if it exists.
    pub fn connect(&mut self, slot: F) -> Option<F> {
        self.record.connect(slot)
    }

    /// Removes and returns the slot if it exists.
    pub fn disconnect(&mut self) -> Option<F> {
        self.record.disconnect()
    }

    /// Returns `true` if the target is in a saved state, `false` otherwise.
    pub fn is_saved(&self) -> bool {
        self.record.is_saved()
    }

    /// Returns `true` if the history can undo.
    pub fn can_undo(&self) -> bool {
        self.record.can_undo()
    }

    /// Returns `true` if the history can redo.
    pub fn can_redo(&self) -> bool {
        self.record.can_redo()
    }

    /// Returns the current branch.
    pub fn branch(&self) -> usize {
        self.root
    }

    /// Returns the position of the current command.
    pub fn current(&self) -> usize {
        self.record.current()
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<C, F> {
        Queue::from(self)
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<C, F> {
        Checkpoint::from(self)
    }

    /// Returns a structure for configurable formatting of the history.
    pub fn display(&self) -> Display<C, F> {
        Display::from(self)
    }

    fn at(&self) -> At {
        At::new(self.branch(), self.current())
    }
}

impl<C: Command, F: FnMut(Signal)> History<C, F> {
    /// Pushes the command to the top of the history and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    pub fn apply(&mut self, target: &mut C::Target, command: C) -> Result<C> {
        let at = self.at();
        let saved = self.record.saved.filter(|&saved| saved > at.current);
        let (merged, tail) = self.record.__apply(target, command)?;
        // Check if the limit has been reached.
        if !merged && at.current == self.current() {
            let root = self.branch();
            self.rm_child(root, 0);
            self.branches
                .values_mut()
                .filter(|branch| branch.parent.branch == root)
                .for_each(|branch| branch.parent.current -= 1);
        }
        // Handle new branch.
        if !tail.is_empty() {
            let new = self.next;
            self.next += 1;
            self.branches
                .insert(at.branch, Branch::new(new, at.current, tail));
            self.set_root(new, at.current, saved);
        }
        Ok(())
    }

    /// Calls the [`undo`] method for the active command
    /// and sets the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    pub fn undo(&mut self, target: &mut C::Target) -> Result<C> {
        self.record.undo(target)
    }

    /// Calls the [`redo`] method for the active command
    /// and sets the next one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the error is returned.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    pub fn redo(&mut self, target: &mut C::Target) -> Result<C> {
        self.record.redo(target)
    }

    /// Repeatedly calls [`undo`] or [`redo`] until the command in `branch` at `current` is reached.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    /// [`redo`]: trait.Command.html#method.redo
    pub fn go_to(
        &mut self,
        target: &mut C::Target,
        branch: usize,
        current: usize,
    ) -> Option<Result<C>> {
        let root = self.root;
        if root == branch {
            return self.record.go_to(target, current);
        }
        // Walk the path from `root` to `branch`.
        for (new, branch) in self.mk_path(branch)? {
            // Walk to `branch.current` either by undoing or redoing.
            if let Err(err) = self.record.go_to(target, branch.parent.current).unwrap() {
                return Some(Err(err));
            }
            // Apply the commands in the branch and move older commands into their own branch.
            for entry in branch.entries {
                let current = self.current();
                let saved = self.record.saved.filter(|&saved| saved > current);
                let entries = match self.record.__apply(target, entry.command) {
                    Ok((_, entries)) => entries,
                    Err(err) => return Some(Err(err)),
                };
                if !entries.is_empty() {
                    self.branches
                        .insert(self.root, Branch::new(new, current, entries));
                    self.set_root(new, current, saved);
                }
            }
        }
        self.record.go_to(target, current)
    }

    /// Go back or forward in the history to the command that was made closest to the datetime provided.
    ///
    /// This method does not jump across branches.
    #[cfg(feature = "chrono")]
    pub fn time_travel(
        &mut self,
        target: &mut C::Target,
        to: &DateTime<impl TimeZone>,
    ) -> Option<Result<C>> {
        self.record.time_travel(target, to)
    }

    /// Marks the target as currently being in a saved or unsaved state.
    pub fn set_saved(&mut self, saved: bool) {
        self.saved = None;
        self.record.set_saved(saved);
    }

    /// Removes all commands from the history without undoing them.
    pub fn clear(&mut self) {
        self.root = 0;
        self.next = 1;
        self.saved = None;
        self.record.clear();
        self.branches.clear();
    }

    pub(crate) fn jump_to(&mut self, root: usize) {
        let mut branch = self.branches.remove(&root).unwrap();
        debug_assert_eq!(branch.parent, self.at());
        let current = self.current();
        let saved = self.record.saved.filter(|&saved| saved > current);
        let tail = self.record.entries.split_off(current);
        self.record.entries.append(&mut branch.entries);
        self.branches
            .insert(self.root, Branch::new(root, current, tail));
        self.set_root(root, current, saved);
    }

    fn set_root(&mut self, root: usize, current: usize, saved: Option<usize>) {
        let old = self.branch();
        self.root = root;
        debug_assert_ne!(old, root);
        // Handle the child branches.
        self.branches
            .values_mut()
            .filter(|branch| branch.parent.branch == old && branch.parent.current <= current)
            .for_each(|branch| branch.parent.branch = root);
        match (self.record.saved, saved, self.saved) {
            (Some(_), None, None) | (None, None, Some(_)) => self.swap_saved(root, old, current),
            (None, Some(_), None) => {
                self.record.saved = saved;
                self.swap_saved(old, root, current);
            }
            (None, None, None) => (),
            _ => unreachable!(),
        }
    }

    fn swap_saved(&mut self, old: usize, new: usize, current: usize) {
        debug_assert_ne!(old, new);
        if let Some(At { current: saved, .. }) = self
            .saved
            .filter(|at| at.branch == new && at.current <= current)
        {
            self.saved = None;
            self.record.saved = Some(saved);
            self.record.slot.emit(Signal::Saved(true));
        } else if let Some(saved) = self.record.saved {
            self.saved = Some(At::new(old, saved));
            self.record.saved = None;
            self.record.slot.emit(Signal::Saved(false));
        }
    }

    fn rm_child(&mut self, branch: usize, current: usize) {
        // We need to check if any of the branches had the removed node as root.
        let mut dead: Vec<_> = self
            .branches
            .iter()
            .filter(|&(_, child)| child.parent == At::new(branch, current))
            .map(|(&id, _)| id)
            .collect();
        while let Some(parent) = dead.pop() {
            // Remove the dead branch.
            self.branches.remove(&parent).unwrap();
            self.saved = self.saved.filter(|saved| saved.branch != parent);
            // Add the children of the dead branch so they are removed too.
            dead.extend(
                self.branches
                    .iter()
                    .filter(|&(_, child)| child.parent.branch == parent)
                    .map(|(&id, _)| id),
            )
        }
    }

    fn mk_path(&mut self, mut to: usize) -> Option<impl Iterator<Item = (usize, Branch<C>)>> {
        debug_assert_ne!(self.branch(), to);
        let mut dest = self.branches.remove(&to)?;
        let mut i = dest.parent.branch;
        let mut path = vec![(to, dest)];
        while i != self.branch() {
            dest = self.branches.remove(&i).unwrap();
            to = i;
            i = dest.parent.branch;
            path.push((to, dest));
        }
        Some(path.into_iter().rev())
    }
}

impl<C: ToString, F> History<C, F> {
    /// Returns the string of the command which will be undone in the next call to [`undo`].
    ///
    /// [`undo`]: struct.History.html#method.undo
    pub fn undo_text(&self) -> Option<String> {
        self.record.undo_text()
    }

    /// Returns the string of the command which will be redone in the next call to [`redo`].
    ///
    /// [`redo`]: struct.History.html#method.redo
    pub fn redo_text(&self) -> Option<String> {
        self.record.redo_text()
    }
}

impl<C: Command> Default for History<C>
where
    C::Target: Default,
{
    fn default() -> History<C> {
        History::new()
    }
}

impl<C, F> From<Record<C, F>> for History<C, F> {
    fn from(record: Record<C, F>) -> Self {
        History {
            root: 0,
            next: 1,
            saved: None,
            record,
            branches: BTreeMap::new(),
        }
    }
}

impl<C: fmt::Debug, F> fmt::Debug for History<C, F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("History")
            .field("root", &self.root)
            .field("next", &self.next)
            .field("saved", &self.saved)
            .field("record", &self.record)
            .field("branches", &self.branches)
            .finish()
    }
}

/// A branch in the history.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub(crate) struct Branch<C> {
    pub(crate) parent: At,
    pub(crate) entries: VecDeque<Entry<C>>,
}

impl<C> Branch<C> {
    fn new(branch: usize, current: usize, entries: VecDeque<Entry<C>>) -> Branch<C> {
        Branch {
            parent: At::new(branch, current),
            entries,
        }
    }
}

/// Builder for a History.
///
/// # Examples
/// ```
/// # use undo::{Command, history::Builder, Record};
/// # struct Add(char);
/// # impl Command for Add {
/// #     type Target = String;
/// #     type Error = &'static str;
/// #     fn apply(&mut self, s: &mut String) -> undo::Result<Add> {
/// #         s.push(self.0);
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self, s: &mut String) -> undo::Result<Add> {
/// #         self.0 = s.pop().ok_or("s is empty")?;
/// #         Ok(())
/// #     }
/// # }
/// let _ = Builder::new()
///     .limit(100)
///     .capacity(100)
///     .connect(|s| { dbg!(s); })
///     .build::<Add>();
/// ```
pub struct Builder<F = Box<dyn FnMut(Signal)>>(crate::record::Builder<F>);

impl<F> Builder<F> {
    /// Returns a builder for a history.
    pub fn new() -> Builder<F> {
        Builder(crate::record::Builder::new())
    }

    /// Sets the capacity for the history.
    pub fn capacity(self, capacity: usize) -> Builder<F> {
        Builder(self.0.capacity(capacity))
    }

    /// Sets the `limit` for the history.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    pub fn limit(self, limit: usize) -> Builder<F> {
        Builder(self.0.limit(limit))
    }

    /// Sets if the target is initially in a saved state.
    /// By default the target is in a saved state.
    pub fn saved(self, saved: bool) -> Builder<F> {
        Builder(self.0.saved(saved))
    }

    /// Builds the history.
    pub fn build<C>(self) -> History<C, F> {
        History::from(self.0.build())
    }
}

impl<F: FnMut(Signal)> Builder<F> {
    /// Connects the slot.
    pub fn connect(self, f: F) -> Builder<F> {
        Builder(self.0.connect(f))
    }
}

impl Default for Builder {
    fn default() -> Self {
        Builder::new()
    }
}

#[derive(Debug)]
enum QueueCommand<C> {
    Apply(C),
    Undo,
    Redo,
}

/// Wraps a record and gives it batch queue functionality.
///
/// # Examples
/// ```
/// # use undo::{Command, Record};
/// # struct Add(char);
/// # impl Command for Add {
/// #     type Target = String;
/// #     type Error = &'static str;
/// #     fn apply(&mut self, s: &mut String) -> undo::Result<Add> {
/// #         s.push(self.0);
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self, s: &mut String) -> undo::Result<Add> {
/// #         self.0 = s.pop().ok_or("s is empty")?;
/// #         Ok(())
/// #     }
/// # }
/// # fn main() -> undo::Result<Add> {
/// let mut string = String::new();
/// let mut record = Record::new();
/// let mut queue = record.queue();
/// queue.apply(Add('a'));
/// queue.apply(Add('b'));
/// queue.apply(Add('c'));
/// assert_eq!(string, "");
/// queue.commit(&mut string)?;
/// assert_eq!(string, "abc");
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Queue<'a, C, F> {
    history: &'a mut History<C, F>,
    commands: Vec<QueueCommand<C>>,
}

impl<C: Command, F: FnMut(Signal)> Queue<'_, C, F> {
    /// Queues an `apply` action.
    pub fn apply(&mut self, command: C) {
        self.commands.push(QueueCommand::Apply(command));
    }

    /// Queues an `undo` action.
    pub fn undo(&mut self) {
        self.commands.push(QueueCommand::Undo);
    }

    /// Queues a `redo` action.
    pub fn redo(&mut self) {
        self.commands.push(QueueCommand::Redo);
    }

    /// Applies the queued commands.
    ///
    /// # Errors
    /// If an error occurs, it stops applying the commands and returns the error.
    pub fn commit(self, target: &mut C::Target) -> Result<C> {
        for command in self.commands {
            match command {
                QueueCommand::Apply(command) => self.history.apply(target, command)?,
                QueueCommand::Undo => self.history.undo(target)?,
                QueueCommand::Redo => self.history.redo(target)?,
            }
        }
        Ok(())
    }

    /// Cancels the queued actions.
    pub fn cancel(self) {}

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<C, F> {
        self.history.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<C, F> {
        self.history.checkpoint()
    }
}

impl<'a, C, F> From<&'a mut History<C, F>> for Queue<'a, C, F> {
    fn from(history: &'a mut History<C, F>) -> Self {
        Queue {
            history,
            commands: Vec::new(),
        }
    }
}

#[derive(Debug)]
enum CheckpointCommand {
    Apply(usize),
    Undo,
    Redo,
}

/// Wraps a history and gives it checkpoint functionality.
#[derive(Debug)]
pub struct Checkpoint<'a, C, F> {
    history: &'a mut History<C, F>,
    commands: Vec<CheckpointCommand>,
}

impl<C: Command, F: FnMut(Signal)> Checkpoint<'_, C, F> {
    /// Calls the `apply` method.
    pub fn apply(&mut self, target: &mut C::Target, command: C) -> Result<C> {
        let branch = self.history.branch();
        self.history.apply(target, command)?;
        self.commands.push(CheckpointCommand::Apply(branch));
        Ok(())
    }

    /// Calls the `undo` method.
    pub fn undo(&mut self, target: &mut C::Target) -> Result<C> {
        if self.history.can_undo() {
            self.history.undo(target)?;
            self.commands.push(CheckpointCommand::Undo);
        }
        Ok(())
    }

    /// Calls the `redo` method.
    pub fn redo(&mut self, target: &mut C::Target) -> Result<C> {
        if self.history.can_redo() {
            self.history.redo(target)?;
            self.commands.push(CheckpointCommand::Redo);
        }
        Ok(())
    }

    /// Commits the changes and consumes the checkpoint.
    pub fn commit(self) {}

    /// Cancels the changes and consumes the checkpoint.
    ///
    /// # Errors
    /// If an error occur when canceling the changes, the error is returned
    /// and the remaining commands are not canceled.
    pub fn cancel(self, target: &mut C::Target) -> Result<C> {
        for command in self.commands.into_iter().rev() {
            match command {
                CheckpointCommand::Apply(branch) => {
                    let root = self.history.branch();
                    self.history.jump_to(branch);
                    if root == branch {
                        self.history.record.entries.pop_back();
                    } else {
                        self.history.branches.remove(&root).unwrap();
                    }
                }
                CheckpointCommand::Undo => self.history.redo(target)?,
                CheckpointCommand::Redo => self.history.undo(target)?,
            }
        }
        Ok(())
    }

    /// Returns a queue.
    pub fn queue(&mut self) -> Queue<C, F> {
        self.history.queue()
    }

    /// Returns a checkpoint.
    pub fn checkpoint(&mut self) -> Checkpoint<C, F> {
        self.history.checkpoint()
    }
}

impl<'a, C, F> From<&'a mut History<C, F>> for Checkpoint<'a, C, F> {
    fn from(history: &'a mut History<C, F>) -> Self {
        Checkpoint {
            history,
            commands: Vec::new(),
        }
    }
}

/// Configurable display formatting for history.
#[derive(Clone, Debug)]
pub struct Display<'a, C, F> {
    history: &'a History<C, F>,
    format: Format,
}

impl<C, F> Display<'_, C, F> {
    /// Show colored output (on by default).
    ///
    /// Requires the `colored` feature to be enabled.
    #[cfg(feature = "colored")]
    pub fn colored(&mut self, on: bool) -> &mut Self {
        self.format.colored = on;
        self
    }

    /// Show the current position in the output (on by default).
    pub fn current(&mut self, on: bool) -> &mut Self {
        self.format.current = on;
        self
    }

    /// Show detailed output (on by default).
    pub fn detailed(&mut self, on: bool) -> &mut Self {
        self.format.detailed = on;
        self
    }

    /// Show the position of the command (on by default).
    pub fn position(&mut self, on: bool) -> &mut Self {
        self.format.position = on;
        self
    }

    /// Show the saved command (on by default).
    pub fn saved(&mut self, on: bool) -> &mut Self {
        self.format.saved = on;
        self
    }
}

impl<C: fmt::Display, F> Display<'_, C, F> {
    fn fmt_list(
        &self,
        f: &mut fmt::Formatter,
        at: At,
        entry: Option<&Entry<C>>,
        level: usize,
    ) -> fmt::Result {
        self.format.mark(f, level)?;
        self.format.position(f, at, true)?;

        #[cfg(feature = "chrono")]
        if let Some(entry) = entry {
            if self.format.detailed {
                self.format.timestamp(f, &entry.timestamp)?;
            }
        }

        self.format.labels(
            f,
            at,
            At::new(self.history.branch(), self.history.current()),
            self.history
                .record
                .saved
                .map(|saved| At::new(self.history.branch(), saved))
                .or(self.history.saved),
        )?;
        if let Some(entry) = entry {
            if self.format.detailed {
                writeln!(f)?;
                self.format.message(f, entry, Some(level))?;
            } else {
                f.write_char(' ')?;
                self.format.message(f, entry, Some(level))?;
                writeln!(f)?;
            }
        }
        Ok(())
    }

    fn fmt_graph(
        &self,
        f: &mut fmt::Formatter,
        at: At,
        entry: Option<&Entry<C>>,
        level: usize,
    ) -> fmt::Result {
        for (&i, branch) in self
            .history
            .branches
            .iter()
            .filter(|(_, branch)| branch.parent == at)
        {
            for (j, entry) in branch.entries.iter().enumerate().rev() {
                let at = At::new(i, j + branch.parent.current + 1);
                self.fmt_graph(f, at, Some(entry), level + 1)?;
            }
            for j in 0..level {
                self.format.edge(f, j)?;
                f.write_char(' ')?;
            }
            self.format.split(f, level)?;
            writeln!(f)?;
        }
        for i in 0..level {
            self.format.edge(f, i)?;
            f.write_char(' ')?;
        }
        self.fmt_list(f, at, entry, level)
    }
}

impl<'a, C, F> From<&'a History<C, F>> for Display<'a, C, F> {
    fn from(history: &'a History<C, F>) -> Self {
        Display {
            history,
            format: Format::default(),
        }
    }
}

impl<C: fmt::Display, F> fmt::Display for Display<'_, C, F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let branch = self.history.branch();
        for (i, entry) in self.history.record.entries.iter().enumerate().rev() {
            let at = At::new(branch, i + 1);
            self.fmt_graph(f, at, Some(entry), 0)?;
        }
        self.fmt_graph(f, At::new(branch, 0), None, 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use alloc::string::String;

    struct Add(char);

    impl Command for Add {
        type Target = String;
        type Error = &'static str;

        fn apply(&mut self, s: &mut String) -> Result<Add> {
            s.push(self.0);
            Ok(())
        }

        fn undo(&mut self, s: &mut String) -> Result<Add> {
            self.0 = s.pop().ok_or("s is empty")?;
            Ok(())
        }
    }

    #[test]
    fn go_to() {
        //          m
        //          |
        //    j  k  l
        //     \ | /
        //       i
        //       |
        // e  g  h
        // |  | /
        // d  f  p - q *
        // | /  /
        // c  n - o
        // | /
        // b
        // |
        // a
        let mut target = String::new();
        let mut history = History::new();
        history.apply(&mut target, Add('a')).unwrap();
        history.apply(&mut target, Add('b')).unwrap();
        history.apply(&mut target, Add('c')).unwrap();
        history.apply(&mut target, Add('d')).unwrap();
        history.apply(&mut target, Add('e')).unwrap();
        assert_eq!(target, "abcde");
        history.undo(&mut target).unwrap();
        history.undo(&mut target).unwrap();
        assert_eq!(target, "abc");
        let abcde = history.branch();
        history.apply(&mut target, Add('f')).unwrap();
        history.apply(&mut target, Add('g')).unwrap();
        assert_eq!(target, "abcfg");
        history.undo(&mut target).unwrap();
        let abcfg = history.branch();
        history.apply(&mut target, Add('h')).unwrap();
        history.apply(&mut target, Add('i')).unwrap();
        history.apply(&mut target, Add('j')).unwrap();
        assert_eq!(target, "abcfhij");
        history.undo(&mut target).unwrap();
        let abcfhij = history.branch();
        history.apply(&mut target, Add('k')).unwrap();
        assert_eq!(target, "abcfhik");
        history.undo(&mut target).unwrap();
        let abcfhik = history.branch();
        history.apply(&mut target, Add('l')).unwrap();
        assert_eq!(target, "abcfhil");
        history.apply(&mut target, Add('m')).unwrap();
        assert_eq!(target, "abcfhilm");
        let abcfhilm = history.branch();
        history.go_to(&mut target, abcde, 2).unwrap().unwrap();
        history.apply(&mut target, Add('n')).unwrap();
        history.apply(&mut target, Add('o')).unwrap();
        assert_eq!(target, "abno");
        history.undo(&mut target).unwrap();
        let abno = history.branch();
        history.apply(&mut target, Add('p')).unwrap();
        history.apply(&mut target, Add('q')).unwrap();
        assert_eq!(target, "abnpq");

        let abnpq = history.branch();
        history.go_to(&mut target, abcde, 5).unwrap().unwrap();
        assert_eq!(target, "abcde");
        history.go_to(&mut target, abcfg, 5).unwrap().unwrap();
        assert_eq!(target, "abcfg");
        history.go_to(&mut target, abcfhij, 7).unwrap().unwrap();
        assert_eq!(target, "abcfhij");
        history.go_to(&mut target, abcfhik, 7).unwrap().unwrap();
        assert_eq!(target, "abcfhik");
        history.go_to(&mut target, abcfhilm, 8).unwrap().unwrap();
        assert_eq!(target, "abcfhilm");
        history.go_to(&mut target, abno, 4).unwrap().unwrap();
        assert_eq!(target, "abno");
        history.go_to(&mut target, abnpq, 5).unwrap().unwrap();
        assert_eq!(target, "abnpq");
    }
}
