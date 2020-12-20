# undo

**Low-level undo-redo functionality.**

[![Travis](https://travis-ci.com/evenorog/undo.svg?branch=master)](https://travis-ci.com/evenorog/undo)
[![Crates.io](https://img.shields.io/crates/v/undo.svg)](https://crates.io/crates/undo)
[![Docs](https://docs.rs/undo/badge.svg)](https://docs.rs/undo)

It is an implementation of the command pattern, where all modifications are done
by creating objects of commands that applies the modifications. All commands knows
how to undo the changes it applies, and by using the provided data structures
it is easy to apply, undo, and redo changes made to a target.

## Features

* [Command](https://docs.rs/undo/latest/undo/trait.Command.html) provides the base functionality for all commands.
* [Record](https://docs.rs/undo/latest/undo/struct.Record.html) provides basic linear undo-redo functionality.
* [History](https://docs.rs/undo/latest/undo/struct.History.html) provides non-linear undo-redo functionality that allows you to jump between different branches.
* Queue wraps a record or history and extends them with queue functionality.
* Checkpoint wraps a record or history and extends them with checkpoint functionality.
* Commands can be merged into a single command by implementing the
  [merge](https://docs.rs/undo/latest/undo.Command.html#method.merge) method on the command.
  This allows smaller commands to be used to build more complex operations, or smaller incremental changes to be
  merged into larger changes that can be undone and redone in a single step.
* The target can be marked as being saved to disk and the data-structures can track the saved state and notify
  when it changes.
* The amount of changes being tracked can be configured by the user so only the `N` most recent changes are stored.
* Configurable display formatting using the display structure.
* The library can be used as `no_std` by default.

## Cargo Feature Flags

* `chrono`: Enables time stamps and time travel.
* `serde`: Enables serialization and deserialization.
* `colored`: Enables colored output when visualizing the display structures.

## Examples

```rust
use undo::{Command, Record};

struct Add(char);

impl Command for Add {
    type Target = String;
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

fn main() -> undo::Result<Add> {
    let mut target = String::new();
    let mut record = Record::new();
    record.apply(Add('a'), &mut target)?;
    record.apply(Add('b'), &mut target)?;
    record.apply(Add('c'), &mut target)?;
    assert_eq!(record.target(), "abc");
    record.undo(&mut target)?;
    record.undo(&mut target)?;
    record.undo(&mut target)?;
    assert_eq!(record.target(), "");
    record.redo(&mut target)?;
    record.redo(&mut target)?;
    record.redo(&mut target)?;
    assert_eq!(record.target(), "abc");
    Ok(())
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
