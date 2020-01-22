# undo

[![Travis](https://travis-ci.com/evenorog/undo.svg?branch=master)](https://travis-ci.com/evenorog/undo)
[![Crates.io](https://img.shields.io/crates/v/undo.svg)](https://crates.io/crates/undo)
[![Docs](https://docs.rs/undo/badge.svg)](https://docs.rs/undo)

Provides simple undo-redo functionality with dynamic dispatch.

It is an implementation of the command pattern, where all modifications are done
by creating objects of commands that applies the modifications. All commands knows
how to undo the changes it applies, and by using the provided data structures
it is easy to apply, undo, and redo changes made to a target.

## Features

* [Command] provides the base functionality for all commands.
* [Record] provides linear undo-redo functionality.
* [Queue] wraps a [Record] and extends it with queue functionality.
* [Checkpoint] wraps a [Record] and extends it with checkpoint functionality.
* Commands can be merged after being applied to the data-structures by implementing the [merge] method on the command.
  This allows smaller changes made gradually to be merged into larger operations that can be undone and redone
  in a single step.
* The target can be marked as being saved to disk and the data-structures can track the saved state and notify
  when it changes.
* The amount of changes being tracked can be configured by the user so only the `N` most recent changes are stored.

*If you need more advanced features, check out the [redo] crate.*

## Examples

Add this to `Cargo.toml`:

```toml
[dependencies]
undo = "0.40"
```

And this to `main.rs`:

```rust
use undo::{Command, Record};

#[derive(Debug)]
struct Add(char);

impl Command<String> for Add {
    fn apply(&mut self, s: &mut String) -> undo::Result {
        s.push(self.0);
        Ok(())
    }

    fn undo(&mut self, s: &mut String) -> undo::Result {
        self.0 = s.pop().ok_or("`s` is empty")?;
        Ok(())
    }
}

fn main() -> undo::Result {
    let mut record = Record::default();
    record.apply(Add('a'))?;
    record.apply(Add('b'))?;
    record.apply(Add('c'))?;
    assert_eq!(record.target(), "abc");
    record.undo()?;
    record.undo()?;
    record.undo()?;
    assert_eq!(record.target(), "");
    record.redo()?;
    record.redo()?;
    record.redo()?;
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

[Command]: https://docs.rs/undo/latest/undo/trait.Command.html
[Record]: https://docs.rs/undo/latest/undo/struct.Record.html
[Queue]: https://docs.rs/undo/latest/undo/struct.Queue.html
[Checkpoint]: https://docs.rs/undo/latest/undo/struct.Checkpoint.html
[Chain]: https://docs.rs/undo/latest/undo/struct.Chain.html
[merge]: https://docs.rs/undo/latest/undo/trait.Command.html#method.merge
[redo]: https://github.com/evenorog/redo
