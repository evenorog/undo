# Undo
An undo/redo library.

## About
It uses the [Command Pattern](https://en.wikipedia.org/wiki/Command_pattern) where the user
implements the `UndoCmd` trait for each command.

The `UndoStack` has two states, clean and dirty. The stack is clean when no more commands can
be redone, otherwise it is dirty. The stack will notice when it's state changes to either dirty
or clean, and call the user defined methods set in `on_clean` and `on_dirty`. This is useful if
you want to trigger some event when the state changes, eg. enabling and disabling buttons in an ui.

It also supports automatic merging of commands that has the same id.

[![Build Status](https://travis-ci.org/evenorog/undo.svg?branch=master)](https://travis-ci.org/evenorog/undo)
[![Crates.io](https://img.shields.io/crates/v/undo.svg)](https://crates.io/crates/undo)
[![Docs](https://docs.rs/undo/badge.svg)](https://docs.rs/undo)

```toml
[dependencies]
undo = "0.5.0"
```

## Examples
```rust
use undo::{self, UndoCmd, UndoStack};

#[derive(Clone, Copy)]
struct PopCmd {
    vec: *mut Vec<i32>,
    e: Option<i32>,
}

impl UndoCmd for PopCmd {
    type Err = ();

    fn redo(&mut self) -> undo::Result<()> {
        self.e = unsafe {
            let ref mut vec = *self.vec;
            vec.pop()
        };
        Ok(())
    }

    fn undo(&mut self) -> undo::Result<()> {
        unsafe {
            let ref mut vec = *self.vec;
            let e = self.e.ok_or(())?;
            vec.push(e);
        }
        Ok(())
    }
}

fn foo() -> undo::Result<()> {
    let mut vec = vec![1, 2, 3];
    let mut stack = UndoStack::new();
    let cmd = PopCmd { vec: &mut vec, e: None };

    stack.push(cmd)?;
    stack.push(cmd)?;
    stack.push(cmd)?;

    assert!(vec.is_empty());

    stack.undo()?;
    stack.undo()?;
    stack.undo()?;

    assert_eq!(vec.len(), 3);
    Ok(())
}
```
