use fnv::{FnvHashMap, FnvHashSet};
use std::collections::VecDeque;
#[cfg(feature = "display")]
use std::fmt::{self, Display, Formatter};
use {Command, Error, Record, RecordBuilder, Signal};

/// A history of commands.
///
/// A history works mostly like a record but also provides branching, like [Vim]s undo-tree.
///
/// # Examples
/// ```
/// # use std::error::Error;
/// # use undo::*;
/// #[derive(Debug)]
/// struct Add(char);
///
/// impl Command<String> for Add {
///     fn apply(&mut self, s: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
///         s.push(self.0);
///         Ok(())
///     }
///
///     fn undo(&mut self, s: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
///         self.0 = s.pop().ok_or("`s` is empty")?;
///         Ok(())
///     }
/// }
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     let mut history = History::default();
///     history.apply(Add('a'))?;
///     history.apply(Add('b'))?;
///     history.apply(Add('c'))?;
///     assert_eq!(history.as_receiver(), "abc");
///     history.go_to(0, 1).unwrap()?;
///     let abc = history.apply(Add('f'))?.unwrap();
///     history.apply(Add('g'))?;
///     assert_eq!(history.as_receiver(), "afg");
///     history.go_to(abc, 3).unwrap()?;
///     assert_eq!(history.as_receiver(), "abc");
///     Ok(())
/// }
/// ```
///
/// [Vim]: https://www.vim.org/
#[derive(Debug)]
pub struct History<R> {
    root: usize,
    next: usize,
    saved: Option<At>,
    record: Record<R>,
    branches: FnvHashMap<usize, Branch<R>>,
}

impl<R> History<R> {
    /// Returns a new history.
    #[inline]
    pub fn new(receiver: impl Into<R>) -> History<R> {
        History {
            root: 0,
            next: 1,
            saved: None,
            record: Record::new(receiver),
            branches: FnvHashMap::default(),
        }
    }

    /// Returns a builder for a history.
    #[inline]
    pub fn builder() -> HistoryBuilder<R> {
        HistoryBuilder {
            inner: Record::builder(),
        }
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

    /// Sets the limit of the history and returns the new limit.
    ///
    /// If `limit < len` the first commands will be removed until `len == limit`.
    /// However, if the current active command is going to be removed, the limit is instead
    /// adjusted to `len - active` so the active command is not removed.
    ///
    /// # Panics
    /// Panics if `limit` is `0`.
    #[inline]
    pub fn set_limit(&mut self, limit: usize) -> usize {
        let len = self.len();
        let limit = self.record.set_limit(limit);
        let diff = len - self.len();
        let root = self.root();
        for cursor in 0..diff {
            self.remove_children(At {
                branch: root,
                cursor,
            });
        }
        for branch in self
            .branches
            .values_mut()
            .filter(|branch| branch.parent.branch == root)
        {
            branch.parent.cursor -= diff;
        }
        limit
    }

    /// Sets how the signal should be handled when the state changes.
    #[inline]
    pub fn set_signal(&mut self, f: impl FnMut(Signal) + Send + Sync + 'static) {
        self.record.set_signal(f);
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

    /// Marks the receiver as currently being in a saved or unsaved state.
    #[inline]
    pub fn set_saved(&mut self, saved: bool) {
        self.record.set_saved(saved);
        self.saved = None;
    }

    /// Returns `true` if the receiver is in a saved state, `false` otherwise.
    #[inline]
    pub fn is_saved(&self) -> bool {
        self.record.is_saved()
    }

    /// Returns the current branch.
    #[inline]
    pub fn root(&self) -> usize {
        self.root
    }

    /// Returns the position of the current command.
    #[inline]
    pub fn cursor(&self) -> usize {
        self.record.cursor()
    }

    /// Removes all commands from the history without undoing them.
    #[inline]
    pub fn clear(&mut self) {
        let old = self.root;
        self.root = 0;
        self.next = 1;
        self.saved = None;
        self.record.clear();
        self.branches.clear();

        if let Some(ref mut f) = self.record.signal {
            f(Signal::Branch { old, new: 0 })
        }
    }

    /// Pushes the command to the top of the history and executes its [`apply`] method.
    /// The command is merged with the previous top command if they have the same [`id`].
    ///
    /// # Errors
    /// If an error occur when executing [`apply`] the error is returned together with the command.
    ///
    /// [`apply`]: trait.Command.html#tymethod.apply
    /// [`id`]: trait.Command.html#method.id
    #[inline]
    pub fn apply(&mut self, cmd: impl Command<R> + 'static) -> Result<Option<usize>, Error<R>>
    where
        R: 'static,
    {
        let old = self.cursor();
        let (merged, commands) = self.record.__apply(cmd)?;
        // Check if the limit has been reached.
        if !merged && old == self.cursor() {
            let root = self.root;
            self.remove_children(At {
                branch: root,
                cursor: 0,
            });
            for branch in self
                .branches
                .values_mut()
                .filter(|branch| branch.parent.branch == root)
            {
                branch.parent.cursor -= 1;
            }
        }
        // Handle new branch.
        if !commands.is_empty() {
            let root = self.root;
            let next = self.next;
            self.next += 1;
            self.set_root(next, old);
            self.branches.insert(
                root,
                Branch {
                    parent: At {
                        branch: self.root,
                        cursor: old,
                    },
                    commands,
                },
            );

            if let Some(ref mut f) = self.record.signal {
                f(Signal::Branch {
                    old: root,
                    new: self.root,
                })
            }

            Ok(Some(root))
        } else {
            Ok(None)
        }
    }

    /// Calls the [`undo`] method for the active command and sets the previous one as the new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] the error is returned together with the command.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    #[inline]
    #[must_use]
    pub fn undo(&mut self) -> Option<Result<(), Error<R>>> {
        self.record.undo()
    }

    /// Calls the [`redo`] method for the active command and sets the next one as the
    /// new active one.
    ///
    /// # Errors
    /// If an error occur when executing [`redo`] the error is returned together with the command.
    ///
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    #[must_use]
    pub fn redo(&mut self) -> Option<Result<(), Error<R>>> {
        self.record.redo()
    }

    /// Repeatedly calls [`undo`] or [`redo`] until the command in `branch` at `cursor` is reached.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned together with the command.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    /// [`redo`]: trait.Command.html#method.redo
    #[inline]
    #[must_use]
    pub fn go_to(&mut self, branch: usize, cursor: usize) -> Option<Result<usize, Error<R>>>
    where
        R: 'static,
    {
        if self.root == branch {
            return self.record.go_to(cursor).map(|r| r.map(|_| branch));
        }

        // Walk the path from `start` to `dest`.
        let old = self.root;
        for (id, branch) in self.create_path(branch)? {
            // Walk to `branch.cursor` either by undoing or redoing.
            if let Err(err) = self.record.go_to(branch.parent.cursor).unwrap() {
                return Some(Err(err));
            }
            // Apply the commands in the branch and move older commands into their own branch.
            for cmd in branch.commands {
                let old = self.cursor();
                let commands = match self.record.__apply(cmd) {
                    Ok((_, commands)) => commands,
                    Err(err) => return Some(Err(err)),
                };
                // Handle new branch.
                if !commands.is_empty() {
                    self.branches.insert(
                        self.root,
                        Branch {
                            parent: At {
                                branch: id,
                                cursor: old,
                            },
                            commands,
                        },
                    );
                    self.set_root(id, old);
                }
            }
        }

        if let Err(err) = self.record.go_to(cursor)? {
            return Some(Err(err));
        }

        if let Some(ref mut f) = self.record.signal {
            f(Signal::Branch {
                old,
                new: self.root,
            });
        }
        Some(Ok(old))
    }

    /// Jump directly to the command in `branch` at `cursor` and executes its [`undo`] or [`redo`] method.
    ///
    /// This method can be used if the commands store the whole state of the receiver,
    /// and does not require the commands in between to be called to get the same result.
    /// Use [`go_to`] otherwise.
    ///
    /// # Errors
    /// If an error occur when executing [`undo`] or [`redo`] the error is returned together with the command.
    ///
    /// [`undo`]: trait.Command.html#tymethod.undo
    /// [`redo`]: trait.Command.html#method.redo
    /// [`go_to`]: struct.History.html#method.go_to
    #[inline]
    #[must_use]
    pub fn jump_to(&mut self, branch: usize, cursor: usize) -> Option<Result<usize, Error<R>>>
    where
        R: 'static,
    {
        if self.root == branch {
            return self.record.jump_to(cursor).map(|r| r.map(|_| branch));
        }

        // Jump the path from `start` to `dest`.
        let old = self.root;
        for (id, mut branch) in self.create_path(branch)? {
            // Jump to `branch.cursor` either by undoing or redoing.
            if let Err(err) = self.record.jump_to(branch.parent.cursor).unwrap() {
                return Some(Err(err));
            }

            let old = self.cursor();
            let mut commands = self.record.commands.split_off(old);
            self.record.commands.append(&mut branch.commands);
            // Handle new branch.
            if !commands.is_empty() {
                self.branches.insert(
                    self.root,
                    Branch {
                        parent: At {
                            branch: id,
                            cursor: old,
                        },
                        commands,
                    },
                );
                self.set_root(id, old);
            }
        }

        if let Err(err) = self.record.jump_to(cursor)? {
            return Some(Err(err));
        }

        if let Some(ref mut f) = self.record.signal {
            f(Signal::Branch {
                old,
                new: self.root,
            });
        }
        Some(Ok(old))
    }

    /// Returns the string of the command which will be undone in the next call to [`undo`].
    ///
    /// [`undo`]: struct.History.html#method.undo
    #[inline]
    #[must_use]
    #[cfg(feature = "display")]
    pub fn to_undo_string(&self) -> Option<String> {
        self.record.to_undo_string()
    }

    /// Returns the string of the command which will be redone in the next call to [`redo`].
    ///
    /// [`redo`]: struct.History.html#method.redo
    #[inline]
    #[must_use]
    #[cfg(feature = "display")]
    pub fn to_redo_string(&self) -> Option<String> {
        self.record.to_redo_string()
    }

    /// Returns a reference to the `receiver`.
    #[inline]
    pub fn as_receiver(&self) -> &R {
        self.record.as_receiver()
    }

    /// Returns a mutable reference to the `receiver`.
    ///
    /// This method should **only** be used when doing changes that should not be able to be undone.
    #[inline]
    pub fn as_mut_receiver(&mut self) -> &mut R {
        self.record.as_mut_receiver()
    }

    /// Consumes the history, returning the `receiver`.
    #[inline]
    pub fn into_receiver(self) -> R {
        self.record.into_receiver()
    }

    /// Sets the `root`.
    #[inline]
    fn set_root(&mut self, root: usize, cursor: usize) {
        let old = self.root;
        // Handle the child branches.
        for branch in self
            .branches
            .values_mut()
            .filter(|branch| branch.parent.branch == old && branch.parent.cursor <= cursor)
        {
            branch.parent.branch = root;
        }
        // Handle the saved state.
        if let Some(At { cursor: saved, .. }) = self
            .saved
            .filter(|at| at.branch == root && at.cursor <= cursor)
        {
            self.record.saved = Some(saved);
            self.saved = None;
            if let Some(ref mut f) = self.record.signal {
                f(Signal::Saved(true));
            }
        } else if let Some(saved) = self.record.saved {
            self.saved = Some(At {
                branch: self.root,
                cursor: saved,
            });
            self.record.saved = None;
            if let Some(ref mut f) = self.record.signal {
                f(Signal::Saved(false));
            }
        }
        self.root = root;
    }

    /// Remove all children of `branch` at `cursor`.
    #[inline]
    fn remove_children(&mut self, at: At) {
        let mut dead = FnvHashSet::default();
        // We need to check if any of the branches had the removed node as root.
        let mut children = self
            .branches
            .iter()
            .filter(|&(&id, child)| child.parent == at && dead.insert(id))
            .map(|(&id, _)| id)
            .collect::<Vec<_>>();
        // Add all the children of dead branches so they are removed too.
        while let Some(parent) = children.pop() {
            for (&id, _) in self
                .branches
                .iter()
                .filter(|&(&id, child)| child.parent.branch == parent && dead.insert(id))
            {
                children.push(id);
            }
        }
        // Remove all dead branches.
        for id in dead {
            self.branches.remove(&id);
            self.saved = self.saved.filter(|saved| saved.branch != id);
        }
    }

    /// Create a path between the current branch and the `to` branch.
    #[inline]
    #[must_use]
    fn create_path(&mut self, mut to: usize) -> Option<Vec<(usize, Branch<R>)>> {
        // Find the path from `dest` to `root`.
        let root = self.root;
        let visited = {
            let mut visited = Vec::with_capacity(self.capacity());
            let mut dest = self.branches.get(&to)?;
            while dest.parent.branch != root {
                visited.push(dest.parent.branch);
                dest = &self.branches[&dest.parent.branch];
            }
            visited
        };
        // Find the path from `start` to the lowest common ancestor of `dest`.
        let mut path = Vec::with_capacity(visited.len() + self.record.len());
        for id in visited {
            let branch = self.branches.remove(&id).unwrap();
            path.push((id, branch));
        }

        // Find the path from `dest` to the lowest common ancestor of `start`.
        let mut dest = self.branches.remove(&to)?;
        let mut id = to;
        to = dest.parent.branch;
        let len = path.len();
        path.push((id, dest));
        let last = path
            .last()
            .map_or(root, |&(_, ref last)| last.parent.branch);
        while to != last {
            dest = self.branches.remove(&to).unwrap();
            id = to;
            to = dest.parent.branch;
            path.push((id, dest));
        }
        path[len..].reverse();
        Some(path)
    }
}

impl<R: Default> Default for History<R> {
    #[inline]
    fn default() -> History<R> {
        History::new(R::default())
    }
}

impl<R> AsRef<R> for History<R> {
    #[inline]
    fn as_ref(&self) -> &R {
        self.as_receiver()
    }
}

impl<R> AsMut<R> for History<R> {
    #[inline]
    fn as_mut(&mut self) -> &mut R {
        self.as_mut_receiver()
    }
}

impl<R> From<R> for History<R> {
    #[inline]
    fn from(receiver: R) -> Self {
        History::new(receiver)
    }
}

#[cfg(feature = "display")]
impl<R> Display for History<R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        (&self.record as &dyn Display).fmt(f)
    }
}

/// A branch in the history.
#[derive(Debug)]
struct Branch<R> {
    parent: At,
    commands: VecDeque<Box<dyn Command<R> + 'static>>,
}

/// The position in the tree.
#[derive(Copy, Clone, Debug, Default, Hash, Ord, PartialOrd, Eq, PartialEq)]
struct At {
    branch: usize,
    cursor: usize,
}

/// Builder for a history.
#[derive(Debug)]
pub struct HistoryBuilder<R> {
    inner: RecordBuilder<R>,
}

impl<R> HistoryBuilder<R> {
    /// Sets the specified [capacity] for the history.
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> HistoryBuilder<R> {
        self.inner = self.inner.capacity(capacity);
        self
    }

    /// Sets the `limit` for the history.
    ///
    /// If this limit is reached it will start popping of commands at the beginning
    /// of the history when pushing new commands on to the stack. No limit is set by
    /// default which means it may grow indefinitely.
    #[inline]
    pub fn limit(mut self, limit: usize) -> HistoryBuilder<R> {
        self.inner = self.inner.limit(limit);
        self
    }

    /// Sets if the receiver is initially in a saved state.
    #[inline]
    pub fn saved(mut self, saved: bool) -> HistoryBuilder<R> {
        self.inner = self.inner.saved(saved);
        self
    }

    /// Decides how the signal should be handled when the state changes.
    /// By default the history does nothing.
    #[inline]
    pub fn signal(mut self, f: impl FnMut(Signal) + Send + Sync + 'static) -> HistoryBuilder<R> {
        self.inner = self.inner.signal(f);
        self
    }

    /// Creates the history.
    #[inline]
    pub fn build(self, receiver: impl Into<R>) -> History<R> {
        History {
            root: 0,
            next: 1,
            saved: None,
            record: self.inner.build(receiver),
            branches: FnvHashMap::default(),
        }
    }
}

impl<R: Default> HistoryBuilder<R> {
    /// Creates the history with a default `receiver`.
    #[inline]
    pub fn default(self) -> History<R> {
        self.build(R::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[derive(Debug)]
    struct Add(char);

    impl Command<String> for Add {
        fn apply(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
            receiver.push(self.0);
            Ok(())
        }

        fn undo(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
            self.0 = receiver.pop().ok_or("`receiver` is empty")?;
            Ok(())
        }
    }

    #[derive(Debug)]
    struct JumpAdd(char, String);

    impl From<char> for JumpAdd {
        fn from(c: char) -> JumpAdd {
            JumpAdd(c, Default::default())
        }
    }

    impl Command<String> for JumpAdd {
        fn apply(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
            self.1 = receiver.clone();
            receiver.push(self.0);
            Ok(())
        }

        fn undo(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
            *receiver = self.1.clone();
            Ok(())
        }

        fn redo(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
            *receiver = self.1.clone();
            receiver.push(self.0);
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
        let mut history = History::default();
        assert!(history.apply(Add('a')).unwrap().is_none());
        assert!(history.apply(Add('b')).unwrap().is_none());
        assert!(history.apply(Add('c')).unwrap().is_none());
        assert!(history.apply(Add('d')).unwrap().is_none());
        assert!(history.apply(Add('e')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abcde");
        history.undo().unwrap().unwrap();
        history.undo().unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abc");
        let abcde = history.apply(Add('f')).unwrap().unwrap();
        assert!(history.apply(Add('g')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abcfg");
        history.undo().unwrap().unwrap();
        let abcfg = history.apply(Add('h')).unwrap().unwrap();
        assert!(history.apply(Add('i')).unwrap().is_none());
        assert!(history.apply(Add('j')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abcfhij");
        history.undo().unwrap().unwrap();
        let abcfhij = history.apply(Add('k')).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abcfhik");
        history.undo().unwrap().unwrap();
        let abcfhik = history.apply(Add('l')).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abcfhil");
        assert!(history.apply(Add('m')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abcfhilm");
        let abcfhilm = history.go_to(abcde, 2).unwrap().unwrap();
        history.apply(Add('n')).unwrap().unwrap();
        assert!(history.apply(Add('o')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abno");
        history.undo().unwrap().unwrap();
        let abno = history.apply(Add('p')).unwrap().unwrap();
        assert!(history.apply(Add('q')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abnpq");

        let abnpq = history.go_to(abcde, 5).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abcde");
        assert_eq!(history.go_to(abcfg, 5).unwrap().unwrap(), abcde);
        assert_eq!(history.as_receiver(), "abcfg");
        assert_eq!(history.go_to(abcfhij, 7).unwrap().unwrap(), abcfg);
        assert_eq!(history.as_receiver(), "abcfhij");
        assert_eq!(history.go_to(abcfhik, 7).unwrap().unwrap(), abcfhij);
        assert_eq!(history.as_receiver(), "abcfhik");
        assert_eq!(history.go_to(abcfhilm, 8).unwrap().unwrap(), abcfhik);
        assert_eq!(history.as_receiver(), "abcfhilm");
        assert_eq!(history.go_to(abno, 4).unwrap().unwrap(), abcfhilm);
        assert_eq!(history.as_receiver(), "abno");
        assert_eq!(history.go_to(abnpq, 5).unwrap().unwrap(), abno);
        assert_eq!(history.as_receiver(), "abnpq");
    }

    #[test]
    fn jump_to() {
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
        let mut history = History::default();
        assert!(history.apply(JumpAdd::from('a')).unwrap().is_none());
        assert!(history.apply(JumpAdd::from('b')).unwrap().is_none());
        assert!(history.apply(JumpAdd::from('c')).unwrap().is_none());
        assert!(history.apply(JumpAdd::from('d')).unwrap().is_none());
        assert!(history.apply(JumpAdd::from('e')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abcde");
        history.undo().unwrap().unwrap();
        history.undo().unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abc");
        let abcde = history.apply(JumpAdd::from('f')).unwrap().unwrap();
        assert!(history.apply(JumpAdd::from('g')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abcfg");
        history.undo().unwrap().unwrap();
        let abcfg = history.apply(JumpAdd::from('h')).unwrap().unwrap();
        assert!(history.apply(JumpAdd::from('i')).unwrap().is_none());
        assert!(history.apply(JumpAdd::from('j')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abcfhij");
        history.undo().unwrap().unwrap();
        let abcfhij = history.apply(JumpAdd::from('k')).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abcfhik");
        history.undo().unwrap().unwrap();
        let abcfhik = history.apply(JumpAdd::from('l')).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abcfhil");
        assert!(history.apply(JumpAdd::from('m')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abcfhilm");
        let abcfhilm = history.go_to(abcde, 2).unwrap().unwrap();
        history.apply(JumpAdd::from('n')).unwrap().unwrap();
        assert!(history.apply(JumpAdd::from('o')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abno");
        history.undo().unwrap().unwrap();
        let abno = history.apply(JumpAdd::from('p')).unwrap().unwrap();
        assert!(history.apply(JumpAdd::from('q')).unwrap().is_none());
        assert_eq!(history.as_receiver(), "abnpq");

        let abnpq = history.jump_to(abcde, 5).unwrap().unwrap();
        assert_eq!(history.as_receiver(), "abcde");
        assert_eq!(history.jump_to(abcfg, 5).unwrap().unwrap(), abcde);
        assert_eq!(history.as_receiver(), "abcfg");
        assert_eq!(history.jump_to(abcfhij, 7).unwrap().unwrap(), abcfg);
        assert_eq!(history.as_receiver(), "abcfhij");
        assert_eq!(history.jump_to(abcfhik, 7).unwrap().unwrap(), abcfhij);
        assert_eq!(history.as_receiver(), "abcfhik");
        assert_eq!(history.jump_to(abcfhilm, 8).unwrap().unwrap(), abcfhik);
        assert_eq!(history.as_receiver(), "abcfhilm");
        assert_eq!(history.jump_to(abno, 4).unwrap().unwrap(), abcfhilm);
        assert_eq!(history.as_receiver(), "abno");
        assert_eq!(history.jump_to(abnpq, 5).unwrap().unwrap(), abno);
        assert_eq!(history.as_receiver(), "abnpq");
    }
}
