extern crate undo;

use undo::{UndoCmd, UndoStack};

#[derive(Debug)]
struct TxtCmd(char);

impl UndoCmd for TxtCmd {
    type Err = ();

    fn redo(&mut self) -> undo::Result<()> {
        Ok(())
    }

    fn undo(&mut self) -> undo::Result<()> {
        Ok(())
    }

    fn id(&self) -> Option<u64> {
        if self.0 == ' ' { None } else { Some(1) }
    }
}

fn foo() -> undo::Result<()> {
    let mut stack = UndoStack::new();
    stack.push(TxtCmd('a'))?;
    stack.push(TxtCmd('b'))?; // 'a' and 'b' is merged.
    stack.push(TxtCmd(' '))?;
    stack.push(TxtCmd('c'))?;
    stack.push(TxtCmd('d'))?; // 'c' and 'd' is merged.

    println!("{:#?}", stack);
    Ok(())
}

fn main() {
    foo().unwrap();
}
