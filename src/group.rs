use fnv::FnvHashMap;
use {UndoCmd, UndoStack};

#[derive(Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Id(u32);

pub struct UndoGroup<'a, T: UndoCmd + 'a> {
    group: FnvHashMap<Id, UndoStack<'a, T>>,
    active: Option<&'a mut UndoStack<'a, T>>,
}

impl<'a, T: UndoCmd> UndoGroup<'a, T> {
    pub fn new() -> Self {
        UndoGroup {
            group: FnvHashMap::default(),
            active: None,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        UndoGroup {
            group: FnvHashMap::with_capacity_and_hasher(capacity, Default::default()),
            active: None,
        }
    }

    pub fn add_undo_stack(&mut self, stack: UndoStack<'a, T>) -> Id {
        impl Clone for Id {
            fn clone(&self) -> Self {
                Id(self.0)
            }
        }

        let id = match self.group.keys().max() {
            Some(&Id(max)) => Id(max + 1),
            None => Id(0),
        };
        self.group.insert(id.clone(), stack);
        id
    }

    pub fn remove_undo_stack(&mut self, id: Id) -> UndoStack<'a, T> {
        self.group.remove(&id).unwrap()
    }

    pub fn set_active_undo_stack(&'a mut self, id: &Id) {
        self.active = self.group.get_mut(id);
    }

    pub fn is_clean(&self) -> Option<bool> {
        self.active.as_ref().map(|t| t.is_clean())
    }

    pub fn is_dirty(&self) -> Option<bool> {
        self.is_clean().map(|t| !t)
    }

    pub fn push(&mut self, cmd: T) {
        if let Some(ref mut stack) = self.active {
            stack.push(cmd);
        }
    }

    pub fn redo(&mut self) {
        if let Some(ref mut stack) = self.active {
            stack.redo();
        }
    }

    pub fn undo(&mut self) {
        if let Some(ref mut stack) = self.active {
            stack.undo();
        }
    }
}
