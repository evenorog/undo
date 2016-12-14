use fnv::FnvHashMap;
use {UndoCmd, UndoStack};

#[derive(Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Id(u64);

pub struct UndoGroup<'a, T: UndoCmd + 'a> {
    group: FnvHashMap<Id, UndoStack<'a, T>>,
    active: Option<&'a mut UndoStack<'a, T>>,
    id: u64,
}

impl<'a, T: UndoCmd> UndoGroup<'a, T> {
    pub fn new() -> Self {
        UndoGroup {
            group: FnvHashMap::default(),
            active: None,
            id: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        UndoGroup {
            group: FnvHashMap::with_capacity_and_hasher(capacity, Default::default()),
            active: None,
            id: 0,
        }
    }

    pub fn add_stack(&mut self, stack: UndoStack<'a, T>) -> Id {
        let id = Id(self.id);
        self.id += 1;
        self.group.insert(Id(id.0), stack);
        id
    }

    pub fn remove_stack(&mut self, id: Id) -> UndoStack<'a, T> {
        let stack = self.group.remove(&id).unwrap();
        // Check if it was the active stack that was removed.
        let is_active = match self.active {
            Some(ref active) => {
                *active as *const _ == &stack as *const _
            },
            None => return stack,
        };
        // If it was, we remove it from the active stack.
        if is_active {
            self.active = None;
        }
        stack
    }

    pub fn set_active_stack(&'a mut self, id: &Id) {
        self.active = self.group.get_mut(id);
    }

    pub fn clear_active_stack(&mut self) {
        self.active = None;
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
