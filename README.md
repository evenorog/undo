# Undo
An undo/redo library with dynamic dispatch, state handling and automatic command merging.

[![Build Status](https://travis-ci.org/evenorog/undo.svg?branch=master)](https://travis-ci.org/evenorog/undo)
[![Crates.io](https://img.shields.io/crates/v/undo.svg)](https://crates.io/crates/undo)
[![Docs](https://docs.rs/undo/badge.svg)](https://docs.rs/undo)

## About
It uses the [Command Pattern] where the user implements the `UndoCmd` trait for each command.

The `UndoStack` has two states, clean and dirty. The stack is clean when no more commands can
be redone, otherwise it is dirty. When it's state changes to either dirty or clean, it calls
the user defined method set in [`on_state_change`]. This is useful if you want to trigger some
event when the state changes, eg. enabling and disabling undo and redo buttons.

It also supports [automatic merging][auto] of commands with the same id.

## Redo vs Undo
|                 | Redo             | Undo            |
|-----------------|------------------|-----------------|
| Dispatch        | [Static]         | [Dynamic]       |
| State Handling  | Yes              | Yes             |
| Command Merging | [Manual][manual] | [Auto][auto]    |

Both supports command merging but `undo` will automatically merge commands with the same id,
while in `redo` you need to implement the merge method yourself.

## Examples
```rust
use undo::{self, UndoCmd, UndoStack};

#[derive(Clone, Copy, Debug)]
struct PopCmd {
    vec: *mut Vec<i32>,
    e: Option<i32>,
}

impl UndoCmd for PopCmd {
    fn redo(&mut self) -> undo::Result {
        self.e = unsafe {
            let ref mut vec = *self.vec;
            vec.pop()
        };
        Ok(())
    }

    fn undo(&mut self) -> undo::Result {
        unsafe {
            let ref mut vec = *self.vec;
            vec.push(self.e.unwrap());
        }
        Ok(())
    }
}

fn foo() -> undo::Result {
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

[Command Pattern]: https://en.wikipedia.org/wiki/Command_pattern
[`on_state_change`]: struct.UndoStack.html#method.on_state_change
[auto]: trait.UndoCmd.html#method.id
[manual]: https://docs.rs/redo/0.4.0/redo/trait.RedoCmd.html#method.merge
[Static]: https://doc.rust-lang.org/stable/book/trait-objects.html#static-dispatch
[Dynamic]: https://doc.rust-lang.org/stable/book/trait-objects.html#dynamic-dispatch
[`redo`]: https://crates.io/crates/redo
