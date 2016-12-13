use fnv::FnvHashMap;
use {UndoCmd, UndoStack};

pub struct UndoGroup<'a, T: UndoCmd + 'a> {
    group: FnvHashMap<usize, UndoStack<'a, T>>,
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

    pub fn add_undo_stack(&mut self, stack: UndoStack<'a, T>) -> usize {
        let id = match self.group.keys().max() {
            Some(max) => max + 1,
            None => 0,
        };
        self.group.insert(id, stack);
        id
    }

    pub fn remove_undo_stack(&mut self, id: usize) -> Option<UndoStack<'a, T>> {
        self.group.remove(&id)
    }

    pub fn set_active_undo_stack(&'a mut self, id: usize) {
        self.active = self.group.get_mut(&id);
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
