# undo

**A undo-redo library.**

[![Rust](https://github.com/evenorog/undo/actions/workflows/rust.yml/badge.svg)](https://github.com/evenorog/undo/actions/workflows/rust.yml)
[![Crates.io](https://img.shields.io/crates/v/undo.svg)](https://crates.io/crates/undo)
[![Docs](https://docs.rs/undo/badge.svg)](https://docs.rs/undo)

It is an implementation of the command pattern, where all modifications are done
by creating objects that applies the modifications. All objects knows
how to undo the changes it applies, and by using the provided data structures
it is easy to apply, undo, and redo changes made to a target.

## Features

* [Action](https://docs.rs/undo/latest/undo/trait.Action.html) provides the base functionality for all actions.
* [Record](https://docs.rs/undo/latest/undo/record/struct.Record.html) provides basic undo-redo functionality.
* [Timeline](https://docs.rs/undo/latest/undo/timeline/struct.Timeline.html) provides basic undo-redo functionality using a fixed size.
* [History](https://docs.rs/undo/latest/undo/history/struct.History.html) provides non-linear undo-redo functionality that allows you to jump between different branches.
* A queue that wraps a record or history and extends them with queue functionality.
* A checkpoint that wraps a record or history and extends them with checkpoint functionality.
* Actions can be merged into a single action by implementing the
  [merge](https://docs.rs/undo/latest/undo.Action.html#method.merge) method on the action.
  This allows smaller actions to be used to build more complex operations, or smaller incremental changes to be
  merged into larger changes that can be undone and redone in a single step.
* The target can be marked as being saved to disk and the data-structures can track the saved state and notify
  when it changes.
* The amount of changes being tracked can be configured by the user so only the `N` most recent changes are stored.
* Configurable display formatting using the display structure.
* The library can be used as `no_std`.

## Cargo Feature Flags

* `alloc`: Enables the use of the alloc crate, enabled by default.
* `colored`: Enables colored output when visualizing the display structures, enabled by default.
* `chrono`: Enables time stamps and time travel.
* `serde`: Enables serialization and deserialization.

## Examples

```rust
use undo::{Action, History};

struct Add(char);

impl Action for Add {
    type Target = String;
    type Output = ();
    type Error = &'static str;

    fn apply(&mut self, s: &mut String) -> undo::Result<Add> {
        s.push(self.0);
        Ok(())
    }

    fn undo(&mut self, s: &mut String) -> undo::Result<Add> {
        self.0 = s.pop().ok_or("s is empty")?;
        Ok(())
    }
}

fn main() {
    let mut target = String::new();
    let mut history = History::new();
    history.apply(&mut target, Add('a')).unwrap();
    history.apply(&mut target, Add('b')).unwrap();
    history.apply(&mut target, Add('c')).unwrap();
    assert_eq!(target, "abc");
    history.undo(&mut target).unwrap();
    history.undo(&mut target).unwrap();
    history.undo(&mut target).unwrap();
    assert_eq!(target, "");
    history.redo(&mut target).unwrap();
    history.redo(&mut target).unwrap();
    history.redo(&mut target).unwrap();
    assert_eq!(target, "abc");
}
```

### License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
