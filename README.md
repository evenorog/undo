# undo
[![Travis](https://travis-ci.org/evenorog/undo.svg?branch=master)](https://travis-ci.org/evenorog/undo)
[![Appveyor](https://ci.appveyor.com/api/projects/status/89qqvql6a0co558h/branch/master?svg=true)](https://ci.appveyor.com/project/evenorog/undo/branch/master)
[![Crates.io](https://img.shields.io/crates/v/undo.svg)](https://crates.io/crates/undo)
[![Docs](https://docs.rs/undo/badge.svg)](https://docs.rs/undo)

An undo/redo library with dynamic dispatch and automatic command merging.
It uses the [Command Pattern](https://en.wikipedia.org/wiki/Command_pattern) 
where the user modifies a receiver by applying `Command`s on it.

The library has currently two data structures that can be used to modify the receiver:

* A simple `Stack` that pushes and pops commands to modify the receiver.
* A `Record` that can roll the state of the receiver forwards and backwards.

### License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
