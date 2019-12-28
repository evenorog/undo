use crate::{
    At, Checkpoint, Command, Entry, Queue, Record, RecordBuilder, Result, Signal, Timeline,
};
use std::collections::{BTreeMap, VecDeque};
#[cfg(feature = "display")]
use {crate::Display, std::fmt};

/// A history of commands.
///
/// Unlike [Record] which maintains a linear undo history, History maintains an undo tree
/// containing every edit made to the target. By switching between different branches in the
/// tree, the user can get to any previous state of the target.
///
/// # Examples
/// ```
/// # use undo::*;
/// # struct Add(char);
/// # impl Command<String> for Add {
/// #     fn apply(&mut self, s: &mut String) -> undo::Result {
/// #         s.push(self.0);
/// #         Ok(())
/// #     }
/// #     fn undo(&mut self, s: &mut String) -> undo::Result {
/// #         self.0 = s.pop().ok_or("`s` is empty")?;
/// #         Ok(())
/// #     }
/// # }
/// # fn main() -> undo::Result {
/// let mut history = History::default();
/// history.apply(Add('a'))?;
/// history.apply(Add('b'))?;
/// assert_eq!(history.target(), "ab");
/// history.undo().unwrap()?;
/// history.apply(Add('c'))?;
/// assert_eq!(history.target(), "ac");
/// history.undo().unwrap()?;
/// history.undo().unwrap()?;
/// assert_eq!(history.target(), "ab");
/// # Ok(())
/// # }
/// ```
///
/// [Record]: struct.Record.html
#[cfg_attr(feature = "display", derive(Debug))]
pub struct History<T: 'static> {
    root: usize,
    next: usize,
    ctrl: Ctrl,
    pub(crate) saved: Option<At>,
    pub(crate) record: Record<T>,
    pub(crate) branches: BTreeMap<usize, Branch<T>>,
}

impl<T> History<T> {
    /// Returns a new history.
    #[inline]
    pub fn new(target: T) -> History<T> {
        History::from(Record::new(target))
    }

    /// Reserves capacity for at least `additional` more commands.
    ///
    /// # Panics
    /// Panics if the new capacity overflows usize.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.record.reserve(additional);
    }

    /// Returns the capacity of the history.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.record.capacity()
    }

    /// Shrinks the capacity of the history as much as possible.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.record.shrink_to_fit();
    }

    /// Returns the number of commands in the current branch of the history.
    #[inline]
    pub fn len(&self) -> usize {
        self.record.len()
    }

    /// Returns `true` if the current branch of the history is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.record.is_empty()
    }

    /// Returns the limit of the history.
    #[inline]
    pub fn limit(&self) -> usize {
        self.record.limit()
    }

    /// Sets how the signal should be handled when the state changes.
    ///
    /// The previous slot is returned if it exists.
    #[inline]
    pub fn connect(
        &mut self,
        slot: impl FnMut(Signal) + 'static,
    ) -> Option<impl FnMut(Signal) + 'static> {
        self.record.connect(slot)
    }

    /// Removes and returns the slot.
    #[inline]
    pub fn disconnect(&mut self) -> Option<impl FnMut(Signal) + 'static> {
        self.record.disconnect()
    }

    /// Returns `true` if the history can undo.
    #[inline]
    pub fn can_undo(&self) -> bool {
        self.record.can_undo()
    }

    /// Returns `true` if the history can redo.
    #[inline]
    pub fn can_redo(&self) -> bool {
        self.record.can_redo()
    }

    /// Returns `true` if the target is in a saved state, `false` otherwise.
    #[inline]
    pub fn is_saved(&self) -> bool {
        self.record.is_saved()
    }

    /// Marks the target as currently being in a saved or unsaved state.
    #[inline]
    pub fn set_saved(&mut self, saved: bool) {
        self.saved = None;
        self.record.set_saved(saved);
    }

    /// Returns the current branch.
    #[inline]
    pub fn branch(&self) -> usize {
        self.root
    }

    /// Returns the position of the current command.
    #[inline]
    pub fn current(&self) -> usize {
        self.record.current()
    }

    /// Removes all commands from the history without undoing them.
    #[inline]
    pub fn clear(&mut self) {
        self.root = 0;
        self.next = 1;
        self.saved = None;
        self.ctrl.clear();
        self.record.clear();
        self.branches.clear();
    }

    /// Pushes the command to the top of the history and executes its [`apply`] method.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    #[inline]
    pub fn apply(&mut self, command: impl Command<T>) -> Result {
        let at = self.at();
        let saved = self.record.saved.filter(|&saved| saved > at.current);
        let (merged, tail) = self.record.__apply(Entry::new(command))?;
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
        self.ctrl.apply(self.branch());
        Ok(())
    }

    /// Calls the [`undo`] method for the active command
    /// and sets the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result> {
        let root = self.ctrl.undo()?;
        if root == self.branch() {
            self.record.undo()
        } else {
            self.jump_to(root);
            self.record.redo()
        }
    }

    /// Calls the [`redo`] method for the active command
    /// and sets the next one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the error is returned.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result> {
        let root = self.ctrl.redo()?;
        if root == self.branch() {
            self.record.redo()
        } else {
            let ok = self.record.undo();
            if let Some(Ok(_)) = ok {
                self.jump_to(root);
            }
            ok
        }
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

    /// Applies each command in the iterator.
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned
    /// and the remaining commands in the iterator are discarded.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    #[inline]
    pub fn extend<C: Command<T>>(&mut self, commands: impl IntoIterator<Item = C>) -> Result {
        for command in commands {
            self.apply(command)?;
        }
        Ok(())
    }

    /// Returns a queue.
    #[inline]
    pub fn queue(&mut self) -> Queue<History<T>> {
        Queue::from(self)
    }

    /// Returns a checkpoint.
    #[inline]
    pub fn checkpoint(&mut self) -> Checkpoint<History<T>> {
        Checkpoint::from(self)
    }

    /// Returns the string of the command which will be undone in the next call to [`undo`].
    ///
    /// Requires the `display` feature to be enabled.
    ///
    /// [`undo`]: struct.History.html#method.undo
    #[inline]
    #[cfg(feature = "display")]
    pub fn to_undo_string(&self) -> Option<String> {
        self.record.to_undo_string()
    }

    /// Returns the string of the command which will be redone in the next call to [`redo`].
    ///
    /// Requires the `display` feature to be enabled.
    ///
    /// [`redo`]: struct.History.html#method.redo
    #[inline]
    #[cfg(feature = "display")]
    pub fn to_redo_string(&self) -> Option<String> {
        self.record.to_redo_string()
    }

    /// Returns a structure for configurable formatting of the history.
    ///
    /// Requires the `display` feature to be enabled.
    #[inline]
    #[cfg(feature = "display")]
    pub fn display(&self) -> Display<Self> {
        Display::from(self)
    }

    /// Returns a reference to the `target`.
    #[inline]
    pub fn target(&self) -> &T {
        self.record.target()
    }

    /// Returns a mutable reference to the `target`.
    ///
    /// This method should **only** be used when doing changes that should not be able to be undone.
    #[inline]
    pub fn target_mut(&mut self) -> &mut T {
        self.record.target_mut()
    }

    /// Consumes the history, returning the `target`.
    #[inline]
    pub fn into_target(self) -> T {
        self.record.into_target()
    }

    fn at(&self) -> At {
        At::new(self.branch(), self.current())
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
            if let Some(ref mut slot) = self.record.slot {
                slot(Signal::Saved(true));
            }
        } else if let Some(saved) = self.record.saved {
            self.saved = Some(At::new(old, saved));
            self.record.saved = None;
            if let Some(ref mut slot) = self.record.slot {
                slot(Signal::Saved(false));
            }
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
}

impl<T> Timeline for History<T> {
    type Target = T;

    #[inline]
    fn apply(&mut self, command: impl Command<T>) -> Result {
        self.apply(command)
    }

    #[inline]
    fn undo(&mut self) -> Option<Result> {
        self.undo()
    }

    #[inline]
    fn redo(&mut self) -> Option<Result> {
        self.redo()
    }
}

impl<T: Default> Default for History<T> {
    #[inline]
    fn default() -> History<T> {
        History::new(T::default())
    }
}

impl<T> From<T> for History<T> {
    #[inline]
    fn from(target: T) -> History<T> {
        History::new(target)
    }
}

impl<T> From<Record<T>> for History<T> {
    #[inline]
    fn from(record: Record<T>) -> History<T> {
        History {
            root: 0,
            next: 1,
            ctrl: Ctrl::default(),
            saved: None,
            record,
            branches: BTreeMap::default(),
        }
    }
}

#[cfg(feature = "display")]
impl<T> fmt::Display for History<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.display() as &dyn fmt::Display).fmt(f)
    }
}

/// A branch in the history.
#[cfg_attr(feature = "display", derive(Debug))]
pub(crate) struct Branch<T> {
    pub(crate) parent: At,
    pub(crate) entries: VecDeque<Entry<T>>,
}

impl<T> Branch<T> {
    fn new(branch: usize, current: usize, entries: VecDeque<Entry<T>>) -> Branch<T> {
        Branch {
            parent: At::new(branch, current),
            entries,
        }
    }
}

#[derive(Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
struct Ctrl {
    current: usize,
    buf: Vec<usize>,
}

impl Ctrl {
    fn apply(&mut self, root: usize) {
        self.buf.push(root);
        self.current = self.buf.len();
    }

    fn undo(&mut self) -> Option<usize> {
        let index = self.current.checked_sub(1)?;
        let root = *self.buf.get(index)?;
        self.buf.push(root);
        self.current -= 1;
        Some(root)
    }

    fn redo(&mut self) -> Option<usize> {
        let root = *self.buf.get(self.current + 1)?;
        self.buf.push(root);
        self.current += 1;
        Some(root)
    }

    fn clear(&mut self) {
        self.current = 0;
        self.buf.clear();
    }
}

/// Builder for a History.
///
/// # Examples
/// ```
/// # use undo::{History, HistoryBuilder};
/// # fn foo() -> History<String> {
/// HistoryBuilder::new()
///     .capacity(100)
///     .limit(100)
///     .saved(false)
///     .default()
/// # }
/// ```
#[derive(Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct HistoryBuilder {
    inner: RecordBuilder,
}

impl HistoryBuilder {
    /// Returns a builder for a history.
    #[inline]
    pub fn new() -> HistoryBuilder {
        HistoryBuilder {
            inner: RecordBuilder::new(),
        }
    }

    /// Sets the specified capacity for the history.
    #[inline]
    pub fn capacity(&mut self, capacity: usize) -> &mut HistoryBuilder {
        self.inner.capacity(capacity);
        self
    }

    /// Sets the `limit` for the history.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    #[inline]
    pub fn limit(&mut self, limit: usize) -> &mut HistoryBuilder {
        self.inner.limit(limit);
        self
    }

    /// Sets if the target is initially in a saved state.
    /// By default the target is in a saved state.
    #[inline]
    pub fn saved(&mut self, saved: bool) -> &mut HistoryBuilder {
        self.inner.saved(saved);
        self
    }

    /// Builds the history.
    #[inline]
    pub fn build<T>(&self, target: T) -> History<T> {
        History::from(self.inner.build(target))
    }

    /// Builds the history with the slot.
    #[inline]
    pub fn build_with<T>(&self, target: T, slot: impl FnMut(Signal) + 'static) -> History<T> {
        History::from(self.inner.build_with(target, slot))
    }

    /// Creates the history with a default `target`.
    #[inline]
    pub fn default<T: Default>(&self) -> History<T> {
        self.build(T::default())
    }

    /// Creates the history with a default `target` and with the slot.
    #[inline]
    pub fn default_with<T: Default>(&self, slot: impl FnMut(Signal) + 'static) -> History<T> {
        self.build_with(T::default(), slot)
    }
}

impl Default for HistoryBuilder {
    #[inline]
    fn default() -> Self {
        HistoryBuilder::new()
    }
}

#[cfg(all(test, not(feature = "display")))]
mod tests {
    use crate::*;

    struct Add(char);

    impl Command<String> for Add {
        fn apply(&mut self, target: &mut String) -> Result {
            target.push(self.0);
            Ok(())
        }

        fn undo(&mut self, target: &mut String) -> Result {
            self.0 = target.pop().ok_or("`target` is empty")?;
            Ok(())
        }
    }

    #[test]
    fn actions() {
        let mut history = History::default();
        history.apply(Add('a')).unwrap();
        history.apply(Add('b')).unwrap();
        history.undo().unwrap().unwrap();
        history.apply(Add('c')).unwrap();
        history.undo().unwrap().unwrap();
        history.undo().unwrap().unwrap();
        assert_eq!(history.target(), "ab");
        history.redo().unwrap().unwrap();
        history.redo().unwrap().unwrap();
        assert_eq!(history.target(), "ac");
        history.undo().unwrap().unwrap();
        history.undo().unwrap().unwrap();
        assert_eq!(history.target(), "ab");
        history.undo().unwrap().unwrap();
        history.undo().unwrap().unwrap();
        assert_eq!(history.target(), "");
        history.redo().unwrap().unwrap();
        history.redo().unwrap().unwrap();
        assert_eq!(history.target(), "ab");
        history.redo().unwrap().unwrap();
        history.redo().unwrap().unwrap();
        assert_eq!(history.target(), "ac");
    }
}
