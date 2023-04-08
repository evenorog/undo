use crate::Edit;
use core::fmt::{self, Debug, Display, Formatter};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use std::time::SystemTime;

/// Wrapper around an edit that contains additional metadata.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Entry<E> {
    pub edit: E,
    #[cfg(feature = "std")]
    pub created_at: SystemTime,
    #[cfg(feature = "std")]
    pub updated_at: SystemTime,
}

impl<E: Edit> Entry<E> {
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
