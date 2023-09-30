use crate::{Edit, Merged};
use core::fmt::{self, Debug, Display, Formatter};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use std::time::SystemTime;

/// Wrapper around an [`Edit`] command that contains additional metadata.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct Entry<E> {
    edit: E,
    #[cfg(feature = "std")]
    st_edit: SystemTime,
    #[cfg(feature = "std")]
    st_undo: SystemTime,
    #[cfg(feature = "std")]
    st_redo: SystemTime,
}

impl<E> Entry<E> {
    pub(crate) const fn new(edit: E) -> Self {
        Entry {
            edit,
            #[cfg(feature = "std")]
            st_edit: SystemTime::UNIX_EPOCH,
            #[cfg(feature = "std")]
            st_undo: SystemTime::UNIX_EPOCH,
            #[cfg(feature = "std")]
            st_redo: SystemTime::UNIX_EPOCH,
        }
    }

    /// Returns the edit command.
    pub fn get(&self) -> &E {
        &self.edit
    }

    /// Returns the time the edit method was called.
    #[cfg(feature = "std")]
    pub fn st_of_edit(&self) -> SystemTime {
        self.st_edit
    }

    /// Returns the last time the undo method was called.
    ///
    /// Returns [`UNIX_EPOCH`](SystemTime::UNIX_EPOCH) if it has never been called.
    #[cfg(feature = "std")]
    pub fn st_of_undo(&self) -> SystemTime {
        self.st_undo
    }

    /// Returns the last time the redo method was called.
    ///
    /// Returns [`UNIX_EPOCH`](SystemTime::UNIX_EPOCH) if it has never been called.
    #[cfg(feature = "std")]
    pub fn st_of_redo(&self) -> SystemTime {
        self.st_redo
    }

    /// Returns the largest of the edit, undo, and redo times.
    #[cfg(feature = "std")]
    pub fn st_of_latest(&self) -> SystemTime {
        self.st_edit.max(self.st_undo).max(self.st_redo)
    }
}

impl<E: Edit> Entry<E> {
    pub(crate) fn edit(&mut self, target: &mut E::Target) -> E::Output {
        #[cfg(feature = "std")]
        {
            self.st_edit = SystemTime::now();
        }
        self.edit.edit(target)
    }

    pub(crate) fn undo(&mut self, target: &mut E::Target) -> E::Output {
        #[cfg(feature = "std")]
        {
            self.st_undo = SystemTime::now();
        }
        self.edit.undo(target)
    }

    pub(crate) fn redo(&mut self, target: &mut E::Target) -> E::Output {
        #[cfg(feature = "std")]
        {
            self.st_redo = SystemTime::now();
        }
        self.edit.redo(target)
    }

    pub(crate) fn merge(&mut self, other: Self) -> Merged<Self>
    where
        Self: Sized,
    {
        match self.edit.merge(other.edit) {
            Merged::Yes => {
                #[cfg(feature = "std")]
                {
                    self.st_edit = other.st_edit;
                }
                Merged::Yes
            }
            Merged::No(edit) => Merged::No(Self { edit, ..other }),
            Merged::Annul => Merged::Annul,
        }
    }
}

impl<E: Display> Display for Entry<E> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(&self.edit, f)
    }
}
