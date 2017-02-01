# Undo
An undo/redo library.

It uses the [Command Pattern](https://en.wikipedia.org/wiki/Command_pattern) where the user
implements the `UndoCmd` trait for each command and then the commands can be used with the
`UndoStack`.

The `UndoStack` has two different states, clean and dirty. The stack is in a clean state when
there are no more commands that can be redone, otherwise it's in a dirty state. The stack
can be configured to call a given method when this state changes, using the `on_clean` and
`on_dirty` methods.

The `UndoStack` also supports easy merging of commands by just implementing the `id` method
for a given command.

[![Build Status](https://travis-ci.org/evenorog/undo.svg?branch=master)](https://travis-ci.org/evenorog/undo)
[![Crates.io](https://img.shields.io/crates/v/undo.svg)](https://crates.io/crates/undo)
[![Docs](https://docs.rs/undo/badge.svg)](https://docs.rs/undo)

```toml
[dependencies]
undo = "0.4.0"
```

## Examples
```rust
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
    let vec = Rc::new(RefCell::new(vec![1, 2, 3]));
    let mut stack = UndoStack::new();
    let cmd = PopCmd { vec: vec.clone(), e: None };

    stack.push(cmd.clone());
    stack.push(cmd.clone());
    stack.push(cmd.clone());

    assert!(vec.borrow().is_empty());

    stack.undo();
    stack.undo();
    stack.undo();

    assert_eq!(vec.borrow().len(), 3);
}
```
