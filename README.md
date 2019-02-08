<img src="https://raw.githubusercontent.com/evenorog/undo/master/undo.svg?sanitize=true" alt="undo" width="100%">

[![Travis](https://travis-ci.org/evenorog/undo.svg?branch=master)](https://travis-ci.org/evenorog/undo)
[![Crates.io](https://img.shields.io/crates/v/undo.svg)](https://crates.io/crates/undo)
[![Docs](https://docs.rs/undo/badge.svg)](https://docs.rs/undo)

Provides undo-redo functionality with dynamic dispatch and automatic command merging.

It is an implementation of the command pattern, where all modifications are done
by creating objects of commands that applies the modifications. All commands knows
how to undo the changes it applies, and by using the provided data structures
it is easy to apply, undo, and redo changes made to a receiver.

# Contents

* [Command] provides the base functionality for all commands.
* [Record] provides stack based undo-redo functionality.
* [History] provides tree based undo-redo functionality that allows you to jump between different branches.
* [Queue] wraps a [Record] or [History] and extends them with queue functionality.
* [Checkpoint] wraps a [Record] or [History] and extends them with checkpoint functionality.
* Commands can be merged using the [merge!] macro or the [merge] method.
  When two commands are merged, undoing and redoing them are done in a single step.
* Configurable display formatting is provided when the `display` feature is enabled.
* Time stamps and time travel is provided when the `chrono` feature is enabled.

# Examples

Add this to `Cargo.toml`:

```toml
[dependencies]
undo = "0.31"
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
    assert_eq!(record.as_receiver(), "abc");
    record.undo().unwrap()?;
    record.undo().unwrap()?;
    record.undo().unwrap()?;
    assert_eq!(record.as_receiver(), "");
    record.redo().unwrap()?;
    record.redo().unwrap()?;
    record.redo().unwrap()?;
    assert_eq!(record.as_receiver(), "abc");
    Ok(())
}
 ```

## License

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
[History]: https://docs.rs/undo/latest/undo/struct.History.html
[Queue]: https://docs.rs/undo/latest/undo/struct.Queue.html
[Checkpoint]: https://docs.rs/undo/latest/undo/struct.Checkpoint.html
[merge!]: https://docs.rs/undo/latest/undo/macro.merge.html
[merge]: https://docs.rs/undo/latest/undo/trait.Command.html#method.merge
