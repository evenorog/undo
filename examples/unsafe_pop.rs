extern crate undo;

use undo::{UndoCmd, UndoStack};

/// Pops an element from a vector.
#[derive(Clone)]
struct PopCmd {
    vec: *mut Vec<i32>,
    e: Option<i32>,
}

impl UndoCmd for PopCmd {
    fn redo(&mut self) {
        self.e = unsafe { (*self.vec).pop() };
    }

    fn undo(&mut self) {
        unsafe { (*self.vec).push(self.e.unwrap()) };
        self.e = None;
    }
}

fn main() {
    let mut vec = vec![1, 2, 3];
    let mut stack = UndoStack::new();

    let cmd = PopCmd { vec: &mut vec as *mut _, e: None };
    stack.push(cmd.clone());
    stack.push(cmd.clone());
    stack.push(cmd.clone());

    assert!(vec.is_empty());

    stack.undo();
    stack.undo();
    stack.undo();

    assert_eq!(vec.len(), 3);
}
