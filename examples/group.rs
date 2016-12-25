extern crate undo;

use std::rc::Rc;
use std::cell::RefCell;
use undo::{UndoCmd, UndoStack, UndoGroup};

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
    // We need to use Rc<RefCell> since all commands are going to mutate the vec.
    let vec1 = Rc::new(RefCell::new(vec![1, 2, 3]));
    let vec2 = Rc::new(RefCell::new(vec![1, 2, 3]));

    let mut group = UndoGroup::new();

    let a = group.add(UndoStack::new());
    let b = group.add(UndoStack::new());

    group.set_active(&a);
    group.push(PopCmd { vec: vec1.clone(), e: None });
    assert_eq!(vec1.borrow().len(), 2);

    group.set_active(&b);
    group.push(PopCmd { vec: vec2.clone(), e: None });
    assert_eq!(vec2.borrow().len(), 2);

    group.set_active(&a);
    group.undo();
    assert_eq!(vec1.borrow().len(), 3);

    group.set_active(&b);
    group.undo();
    assert_eq!(vec2.borrow().len(), 3);
}
