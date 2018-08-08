# undo
[![Travis](https://travis-ci.org/evenorog/undo.svg?branch=master)](https://travis-ci.org/evenorog/undo)
[![Appveyor](https://ci.appveyor.com/api/projects/status/89qqvql6a0co558h/branch/master?svg=true)](https://ci.appveyor.com/project/evenorog/undo/branch/master)
[![Crates.io](https://img.shields.io/crates/v/undo.svg)](https://crates.io/crates/undo)
[![Docs](https://docs.rs/undo/badge.svg)](https://docs.rs/undo)

An undo-redo library with dynamic dispatch and automatic command merging.

It uses the [command pattern] where the user modifies the receiver by
applying commands on it. Since each command knows how to undo and redo
the changes it applies to the receiver, the state of the receiver can
be rolled forwards or backwards by calling undo or redo in the correct order.

The [Record] and [History] provides functionality to store and keep track
of the applied commands, and makes it easy to undo and redo changes.
The Record provides a stack based undo-redo functionality, while the
History provides a tree based undo-redo functionality where you can
jump between different branches.

Commands can be merged using the [`merge!`] macro or the [`id`] method.
When two commands are merged, undoing and redoing them are done in a single step.

## Examples

Add this to `Cargo.toml`:

```toml
[dependencies]
undo = "0.23"
```

And this to `main.rs`:

 ```rust
#[derive(Debug)]
struct Add(char);

impl Command<String> for Add {
    fn apply(&mut self, s: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
        s.push(self.0);
        Ok(())
    }

    fn undo(&mut self, s: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.0 = s.pop().ok_or("`s` is empty")?;
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
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

[command pattern]: https://en.wikipedia.org/wiki/Command_pattern
[Record]: https://docs.rs/undo/latest/undo/struct.Record.html
[History]: https://docs.rs/undo/latest/undo/struct.History.html
[`merge!`]: https://docs.rs/undo/latest/undo/macro.merge.html
[`id`]: https://docs.rs/undo/latest/undo/trait.Command.html#method.id
