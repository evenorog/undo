# Undo
An undo/redo library with dynamic dispatch, state handling and automatic command merging.

[![Build Status](https://travis-ci.org/evenorog/undo.svg?branch=master)](https://travis-ci.org/evenorog/undo)
[![Crates.io](https://img.shields.io/crates/v/undo.svg)](https://crates.io/crates/undo)
[![Docs](https://docs.rs/undo/badge.svg)](https://docs.rs/undo)

## About
It uses the [Command Pattern] where the user implements the `UndoCmd` trait for each command.

The `UndoStack` has two states, clean and dirty. The stack is clean when no more commands can
be redone, otherwise it is dirty. The stack will notice when it's state changes to either dirty
or clean, and call the user defined methods set in [`on_clean`] and [`on_dirty`]. This is useful if
you want to trigger some event when the state changes, eg. enabling and disabling buttons in an ui.

It also supports [automatic merging] of commands with the same id.

## Redo vs Undo
|                 | Redo         | Undo            |
|-----------------|--------------|-----------------|
| Dispatch        | Static       | Dynamic         |
| State Handling  | Yes          | Yes             |
| Command Merging | Yes (manual) | Yes (automatic) |

`undo` uses [dynamic dispatch] instead of [static dispatch] to store the commands, which means
it has some additional overhead compared to [`redo`]. However, this has the benefit that you
can store multiple types of commands in a `UndoStack` at a time. Both supports state handling
and command merging but `undo` will automatically merge commands with the same id, while
in `redo` you need to implement the merge method yourself.

## Disable State Handling
If state handling is not needed, it can be disabled by setting the `no_state` feature flag.

```toml
[dependencies]
undo = { version = "0.5.2", features = ["no_state"] }
```

## Examples
```toml
[dependencies]
undo = "0.5.2"
```

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

*An unsafe implementation of `redo` and `undo` is used in examples since it is less verbose and
makes the examples easier to follow.*

[Command Pattern]: https://en.wikipedia.org/wiki/Command_pattern
[`on_clean`]: struct.UndoStack.html#method.on_clean
[`on_dirty`]: struct.UndoStack.html#method.on_dirty
[automatic merging]: trait.UndoCmd.html#method.id
[static dispatch]: https://doc.rust-lang.org/stable/book/trait-objects.html#static-dispatch
[dynamic dispatch]: https://doc.rust-lang.org/stable/book/trait-objects.html#dynamic-dispatch
[`redo`]: https://crates.io/crates/redo
