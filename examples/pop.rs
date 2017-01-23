extern crate undo;

use std::rc::Rc;
use std::cell::RefCell;
use undo::{UndoCmd, UndoStack};

/// Pops an element from a vector.
#[derive(Clone)]
struct PopCmd {
    vec: Rc<RefCell<Vec<i32>>>,
    e: Option<i32>,
}

impl UndoCmd for PopCmd {
    fn redo(&mut self) {
        self.e = self.vec.borrow_mut().pop();
    }

    fn undo(&mut self) {
        self.vec.borrow_mut().push(self.e.unwrap());
        self.e = None;
    }
}

fn main() {
    // We need to use Rc<RefCell> in safe code since all commands are going to mutate the vec.
    // unsafe_pop.rs shows how to use raw pointers instead, if performance is important.
    let vec = Rc::new(RefCell::new(vec![1, 2, 3]));
    let mut stack = UndoStack::new()
        .on_clean(|| println!("This is called when the stack changes from dirty to clean!"))
        .on_dirty(|| println!("This is called when the stack changes from clean to dirty!"));

    let cmd = PopCmd { vec: vec.clone(), e: None };
    stack.push(cmd.clone());
    stack.push(cmd.clone());
    stack.push(cmd.clone());

    assert!(vec.borrow().is_empty());

    stack.undo(); // on_dirty is going to be called here.
    stack.undo();
    stack.undo();

    assert_eq!(vec.borrow().len(), 3);
}
