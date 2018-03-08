use std::collections::hash_map::{HashMap, RandomState};
use std::error;
use std::fmt::{self, Debug, Formatter};
#[cfg(feature = "display")]
use std::fmt::Display;
use std::hash::{BuildHasher, Hash};
use std::marker::PhantomData;
use {Command, Commands, Error, Record, Stack};

/// A group of either stacks or records.
pub struct Group<'a, K: Hash + Eq, V, S = RandomState> where S: BuildHasher {
    map: HashMap<K, V, S>,
    active: Option<K>,
    signals: Option<Box<FnMut(Option<&K>) + Send + Sync + 'a>>,
}

impl<'a, K: Hash + Eq, V> Group<'a, K, V, RandomState> {
    /// Returns a new group.
    #[inline]
    pub fn new() -> Group<'a, K, V, RandomState> {
        Default::default()
    }
}

impl<'a, K: Hash + Eq, V, S: BuildHasher> Group<'a, K, V, S> {
    /// Returns a builder for a group.
    #[inline]
    pub fn builder() -> GroupBuilder<'a, K, V, S> {
        GroupBuilder {
            map: PhantomData,
            capacity: 0,
            signals: None,
        }
    }

    /// Returns the capacity of the group.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.map.capacity()
    }

    /// Returns the number of items in the group.
    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns `true` if the group is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Inserts an item into the group.
    #[inline]
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.map.insert(k, v)
    }

    /// Removes an item from the group.
    #[inline]
    pub fn remove(&mut self, k: &K) -> Option<V> {
        self.map.remove(k)
    }

    /// Gets a reference to the current active item in the group.
    #[inline]
    pub fn get(&self) -> Option<&V> {
        self.active.as_ref().and_then(|active| self.map.get(active))
    }

    /// Gets a mutable reference to the current active item in the group.
    #[inline]
    pub fn get_mut(&mut self) -> Option<&mut V> {
        let map = &mut self.map;
        self.active.as_mut().and_then(move |active| map.get_mut(active))
    }

    /// Sets the current active item in the group.
    ///
    /// Returns `None` if the item was successfully set, otherwise `k` is returned.
    #[inline]
    pub fn set(&mut self, k: K) -> Option<K> {
        if self.map.contains_key(&k) {
            if self.active.as_ref().map_or(false, |a| a != &k) {
                if let Some(ref mut f) = self.signals {
                    f(Some(&k))
                }
            }
            self.active = Some(k);
            None
        } else {
            Some(k)
        }
    }

    /// Unsets the current active item in the group.
    #[inline]
    pub fn unset(&mut self) {
        if self.active.is_some() {
            self.active = None;
            if let Some(ref mut f) = self.signals {
                f(None);
            }
        }
    }
}

impl<'a, K: Hash + Eq, R, S: BuildHasher> Group<'a, K, Stack<R>, S> {
    /// Calls the [`push`] method on the active stack.
    ///
    /// [`push`]: stack/struct.Stack.html#method.push
    #[inline]
    pub fn push<C>(&mut self, cmd: C) -> Option<Result<(), Error<R>>>
        where
            C: Command<R> + 'static,
            R: 'static,
    {
        self.get_mut().map(move |stack| stack.push(cmd))
    }

    /// Calls the [`pop`] method on the active stack.
    ///
    /// [`pop`]: stack/struct.Stack.html#method.pop
    #[inline]
    pub fn pop(&mut self) -> Option<Result<Box<Command<R>>, Error<R>>> {
        self.get_mut().and_then(|stack| stack.pop())
    }
}

impl<'a, K: Hash + Eq, R, S: BuildHasher> Group<'a, K, Record<'a, R>, S> {
    /// Calls the [`set_saved`] method on the active record.
    ///
    /// [`set_saved`]: record/struct.Record.html#method.set_saved
    #[inline]
    pub fn set_saved(&mut self) -> Option<()> {
        self.get_mut().map(|stack| stack.set_saved())
    }

    /// Calls the [`set_unsaved`] method on the active record.
    ///
    /// [`set_unsaved`]: record/struct.Record.html#method.set_unsaved
    #[inline]
    pub fn set_unsaved(&mut self) -> Option<()> {
        self.get_mut().map(|stack| stack.set_unsaved())
    }

    /// Calls the [`is_saved`] method on the active record.
    ///
    /// [`is_saved`]: record/struct.Record.html#method.is_saved
    #[inline]
    pub fn is_saved(&self) -> Option<bool> {
        self.get().map(|stack| stack.is_saved())
    }

    /// Calls the [`exec`] method on the active record.
    ///
    /// [`exec`]: record/struct.Record.html#method.exec
    #[inline]
    pub fn exec<C>(&mut self, cmd: C) -> Option<Result<Commands<R>, Error<R>>>
        where
            C: Command<R> + 'static,
            R: 'static,
    {
        self.get_mut().map(move |record| record.exec(cmd))
    }

    /// Calls the [`undo`] method on the active record.
    ///
    /// [`undo`]: record/struct.Record.html#method.undo
    #[inline]
    pub fn undo(&mut self) -> Option<Result<(), Box<error::Error>>> {
        self.get_mut().and_then(|record| record.undo())
    }

    /// Calls the [`redo`] method on the active record.
    ///
    /// [`redo`]: record/struct.Record.html#method.redo
    #[inline]
    pub fn redo(&mut self) -> Option<Result<(), Box<error::Error>>> {
        self.get_mut().and_then(|record| record.redo())
    }

    /// Calls the [`to_undo_string`] method on the active record.
    ///
    /// [`to_undo_string`]: record/struct.Record.html#method.to_undo_string
    #[inline]
    #[cfg(feature = "display")]
    pub fn to_undo_string(&self) -> Option<String> {
        self.get().and_then(|record| record.to_undo_string())
    }

    /// Calls the [`to_redo_string`] method on the active record.
    ///
    /// [`to_redo_string`]: record/struct.Record.html#method.to_redo_string
    #[inline]
    #[cfg(feature = "display")]
    pub fn to_redo_string(&self) -> Option<String> {
        self.get().and_then(|record| record.to_redo_string())
    }
}

impl<'a, K: Hash + Eq + Debug, V: Debug, S: BuildHasher> Debug for Group<'a, K, V, S> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Group")
            .field("map", &self.map)
            .field("active", &self.active)
            .finish()
    }
}

#[cfg(feature = "display")]
impl<'a, K: Hash + Eq + Display, V: Display> Display for Group<'a, K, V> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        for (key, item) in &self.map {
            if self.active.as_ref().map_or(false, |a| a == key) {
                writeln!(f, "* {}:\n{}", key, item)?;
            } else {
                writeln!(f, "  {}:\n{}", key, item)?;
            }
        }
        Ok(())
    }
}

impl<'a, K: Hash + Eq, V> Default for Group<'a, K, V, RandomState> {
    #[inline]
    fn default() -> Group<'a, K, V, RandomState> {
        Group {
            map: HashMap::new(),
            active: None,
            signals: None,
        }
    }
}

/// Builder for a group.
pub struct GroupBuilder<'a, K: Hash + Eq, V, S = RandomState> where S: BuildHasher {
    map: PhantomData<(K, V, S)>,
    capacity: usize,
    signals: Option<Box<FnMut(Option<&K>) + Send + Sync + 'a>>,
}

impl<'a, K: Hash + Eq, V> GroupBuilder<'a, K, V, RandomState> {
    /// Creates the group.
    #[inline]
    pub fn build(self) -> Group<'a, K, V, RandomState> {
        self.build_with_hasher(Default::default())
    }
}

impl<'a, K: Hash + Eq, V, S: BuildHasher> GroupBuilder<'a, K, V, S> {
    /// Sets the specified [capacity] for the group.
    ///
    /// [capacity]: https://doc.rust-lang.org/std/vec/struct.Vec.html#capacity-and-reallocation
    #[inline]
    pub fn capacity(mut self, capacity: usize) -> GroupBuilder<'a, K, V, S> {
        self.capacity = capacity;
        self
    }

    /// Decides what should happen when the active stack changes.
    ///
    /// # Examples
    /// ```
    /// # use undo::*;
    /// # let _: Group<u8, u8> =
    /// Group::builder()
    ///     .signals(|k| match k {
    ///         Some(k) => println!("The new active stack is {}.", k),
    ///         None => println!("No active stack."),
    ///     })
    ///     .build();
    /// ```
    #[inline]
    pub fn signals<F>(mut self, f: F) -> GroupBuilder<'a, K, V, S>
    where
        F: FnMut(Option<&K>) + Send + Sync + 'a
    {
        self.signals = Some(Box::new(f));
        self
    }

    /// Creates the group with the given hasher.
    #[inline]
    pub fn build_with_hasher(self, hasher: S) -> Group<'a, K, V, S> {
        Group {
            map: HashMap::with_capacity_and_hasher(self.capacity, hasher),
            active: None,
            signals: self.signals,
        }
    }
}

impl<'a, K: Hash + Eq, V, S: BuildHasher> Debug for GroupBuilder<'a, K, V, S> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("GroupBuilder")
            .field("map", &self.map)
            .field("capacity", &self.capacity)
            .finish()
    }
}
