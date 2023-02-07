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
