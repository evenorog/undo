use undo::{Action, Record};

struct Push(char);

impl std::fmt::Display for Push {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Push({:?})", self.0)
    }
}

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
    let mut record = Record::new();
    let mut target = String::new();

    record.apply(&mut target, Push('r'));
    record.apply(&mut target, Push('u'));
    record.apply(&mut target, Push('s'));
    record.apply(&mut target, Push('t'));
    assert_eq!(target, "rust");

    record.undo(&mut target);
    record.undo(&mut target);
    record.undo(&mut target);
    assert_eq!(target, "r");

    record.redo(&mut target);
    record.redo(&mut target);
    assert_eq!(target, "rus");

    println!("Record: {}", record.display());
}
