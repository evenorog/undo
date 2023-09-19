use crate::{Edit, Merged};
use core::fmt::{self, Debug, Display, Formatter};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use std::time::SystemTime;

/// Wrapper around an [`Edit`] command that contains additional metadata.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub(crate) struct Entry<E> {
    pub edit: E,
    #[cfg(feature = "std")]
    pub created_at: SystemTime,
    #[cfg(feature = "std")]
    pub updated_at: SystemTime,
}

impl<E: Edit> Entry<E> {
    pub fn edit(&mut self, target: &mut E::Target) -> E::Output {
        self.edit.edit(target)
    }

    pub fn undo(&mut self, target: &mut E::Target) -> E::Output {
        #[cfg(feature = "std")]
        {
            self.updated_at = SystemTime::now();
        }
        self.edit.undo(target)
    }

    pub fn redo(&mut self, target: &mut E::Target) -> E::Output {
        #[cfg(feature = "std")]
        {
            self.updated_at = SystemTime::now();
        }
        self.edit.redo(target)
    }

    pub fn merge(&mut self, other: Self) -> Merged<Self>
    where
        Self: Sized,
    {
        match self.edit.merge(other.edit) {
            Merged::Yes => {
                #[cfg(feature = "std")]
                {
                    self.updated_at = other.updated_at;
                }
                Merged::Yes
            }
            Merged::No(edit) => Merged::No(Self { edit, ..other }),
            Merged::Annul => Merged::Annul,
        }
    }
}

impl<E> From<E> for Entry<E> {
    #[cfg(feature = "std")]
    fn from(edit: E) -> Self {
        let at = SystemTime::now();
        Entry {
            edit,
            created_at: at,
            updated_at: at,
        }
    }

    #[cfg(not(feature = "std"))]
    fn from(edit: E) -> Self {
        Entry { edit }
    }
}

impl<E: Display> Display for Entry<E> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(&self.edit, f)
    }
}
