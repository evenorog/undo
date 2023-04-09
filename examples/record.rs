use core::fmt::{self, Display, Formatter};
use undo::{Edit, Record};

struct Add(char);

impl Edit for Add {
    type Target = String;
    type Output = ();

    fn edit(&mut self, target: &mut String) {
        target.push(self.0);
    }

    fn undo(&mut self, target: &mut String) {
        self.0 = target.pop().expect("cannot pop empty string");
    }
}

impl Display for Add {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Add '{}'", self.0)
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
    assert_eq!(target, "a");

    record.redo(&mut target);
    record.redo(&mut target);
    assert_eq!(target, "abc");

    println!("{}", record.display());
}
