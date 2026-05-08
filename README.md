# undo

**An undo-redo library.**

[![Rust](https://github.com/evenorog/undo/actions/workflows/rust.yml/badge.svg)](https://github.com/evenorog/undo/actions/workflows/rust.yml)
[![Crates.io](https://img.shields.io/crates/v/undo.svg)](https://crates.io/crates/undo)
[![Docs](https://docs.rs/undo/badge.svg)](https://docs.rs/undo)

An implementation of the [command pattern](https://en.wikipedia.org/wiki/Command_pattern).
Each modification to a target value is represented as a struct that knows how to apply
itself and how to reverse the change. The provided data structures wrap these commands
and give you linear or tree-shaped undo-redo navigation over any target type.

## Features

- Linear undo-redo with [`Record`](https://docs.rs/undo/latest/undo/struct.Record.html) or full branching history with [`History`](https://docs.rs/undo/latest/undo/struct.History.html)
- Merge consecutive edits into a single undo step
- Mark the target as saved and receive notifications when that state changes
- Cap the undo stack to retain only the *N* most recent edits
- Batch edits with a queue, or apply tentative edits with a checkpoint
- Colored and configurable display formatting
- `no_std` compatible via the `alloc` feature
- Optional `serde` support

## Example

```rust
use undo::{Edit, Record};

struct Add(char);

impl Edit for Add {
    type Target = String;
    type Output = ();

    fn edit(&mut self, target: &mut String) {
        target.push(self.0);
    }

    fn undo(&mut self, target: &mut String) {
        self.0 = target.pop().unwrap();
    }
}

fn main() {
    let mut target = String::new();
    let mut record = Record::new();

    record.edit(&mut target, Add('a'));
    record.edit(&mut target, Add('b'));
    record.edit(&mut target, Add('c'));
    assert_eq!(target, "abc");

    record.undo(&mut target);
    record.undo(&mut target);
    record.undo(&mut target);
    assert_eq!(target, "");

    record.redo(&mut target);
    record.redo(&mut target);
    record.redo(&mut target);
    assert_eq!(target, "abc");
}
```

## Record vs History

Use `Record` for a simple linear undo stack. Applying a new edit after undoing discards the
now-unreachable edits. Use `History` when you need a full undo tree: undoing and then making
a new edit creates a branch, and you can navigate back to any prior state at any time.

See the [documentation](https://docs.rs/undo) and [examples](https://github.com/evenorog/undo/tree/master/examples) for more information.

### License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
