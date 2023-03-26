use crate::Action;
use core::fmt::{self, Debug, Display, Formatter};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Wrapper around an action that contains additional metadata.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Entry<A> {
    pub action: A,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

impl<A: Action> Entry<A> {
    pub fn undo(&mut self, target: &mut A::Target) -> A::Output {
        self.updated_at = SystemTime::now();
        self.action.undo(target)
    }

    pub fn redo(&mut self, target: &mut A::Target) -> A::Output {
        self.updated_at = SystemTime::now();
        self.action.redo(target)
    }
}

impl<A> From<A> for Entry<A> {
    fn from(action: A) -> Self {
        let at = SystemTime::now();
        Entry {
            action,
            created_at: at,
            updated_at: at,
        }
    }
}

impl<A: Display> Display for Entry<A> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(&self.action, f)
    }
}
