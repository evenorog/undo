# undo

**An undo-redo library.**

[![Rust](https://github.com/evenorog/undo/actions/workflows/rust.yml/badge.svg)](https://github.com/evenorog/undo/actions/workflows/rust.yml)
[![Crates.io](https://img.shields.io/crates/v/undo.svg)](https://crates.io/crates/undo)
[![Docs](https://docs.rs/undo/badge.svg)](https://docs.rs/undo)

It is an implementation of the command pattern, where all modifications are done
by creating objects that applies the modifications. All objects knows
how to undo the changes it applies, and by using the provided data structures
it is easy to apply, undo, and redo changes made to a target.

## Examples

```rust
use undo::{Action, Record};

struct Push(char);

impl Action for Push {
    type Target = String;
    type Output = ();

    fn apply(&mut self, string: &mut String) {
        string.push(self.0);
    }

    fn undo(&mut self, string: &mut String) {
        self.0 = string
            .pop()
            .expect("cannot remove more characters than have been added");
    }
}

fn main() {
    let mut target = String::new();
    let mut record = Record::new();

    record.apply(&mut target, Push('a'));
    record.apply(&mut target, Push('b'));
    record.apply(&mut target, Push('c'));
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

### License

Licensed under either of

-   Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
-   MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
