use undo::{Action, AnyAction, Record};

struct Push(char);

impl Action for Push {
    type Target = String;
    type Output = ();

    fn apply(&mut self, s: &mut String) {
        s.push(self.0);
    }

    fn undo(&mut self, s: &mut String) {
        self.0 = s
            .pop()
            .expect("cannot remove more characters than were added");
    }
}

struct LongPush<'a>(&'a str);

impl Action for LongPush<'_> {
    type Target = String;
    type Output = ();

    fn apply(&mut self, string: &mut String) {
        string.push_str(self.0);
    }

    fn undo(&mut self, string: &mut String) {
        string.truncate(string.len() - self.0.len());
    }
}

fn main() {
    let mut record = Record::<AnyAction<String, ()>>::new();
    let mut target = String::new();

    record.apply(&mut target, AnyAction::new(LongPush("rust")));
    assert_eq!(target, "rust");

    record.apply(&mut target, AnyAction::new(Push('y')));
    assert_eq!(target, "rusty");

    record.undo(&mut target);
    assert_eq!(target, "rust");

    record.apply(&mut target, AnyAction::new(LongPush("acean")));
    assert_eq!(target, "rustacean");

    record.undo(&mut target);
    record.redo(&mut target);
    assert_eq!(target, "rustacean");

    assert!(record.redo(&mut target).is_none());
}
