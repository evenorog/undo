use std::fmt::{self, Display, Formatter};
use undo::{Action, Record};

struct Push(char);

impl Action for Push {
    type Target = String;
    type Output = ();

    fn apply(&mut self, target: &mut String) {
        target.push(self.0);
    }

    fn undo(&mut self, target: &mut String) {
        self.0 = target.pop().expect("cannot pop empty string");
    }
}

impl Display for Push {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Push '{}'", self.0)
    }
}

fn main() {
    let mut record = Record::new();
    let mut target = String::new();

    record.apply(&mut target, Push('a'));
    record.apply(&mut target, Push('b'));
    record.apply(&mut target, Push('c'));
    assert_eq!(target, "abc");

    record.undo(&mut target);
    record.undo(&mut target);
    assert_eq!(target, "a");

    record.redo(&mut target);
    record.redo(&mut target);
    assert_eq!(target, "abc");

    println!("{}", record.display());
}
