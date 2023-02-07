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
    let mut target = String::new();
    let mut record = Record::<AnyAction<String, ()>>::new();

    record.apply(&mut target, AnyAction::new(Push('a')));
    record.apply(&mut target, AnyAction::new(Push('b')));
    record.apply(&mut target, AnyAction::new(Push('c')));
    assert_eq!(target, "abc");

    record.apply(&mut target, AnyAction::new(LongPush("def")));
    assert_eq!(target, "abcdef");

    record.undo(&mut target);
    assert_eq!(target, "abc");

    record.undo(&mut target);
    record.undo(&mut target);
    record.undo(&mut target);
    assert_eq!(target, "");

    record.redo(&mut target);
    record.redo(&mut target);
    record.redo(&mut target);
    assert_eq!(target, "abc");

    record.redo(&mut target);
    assert_eq!(target, "abcdef");

    assert!(record.redo(&mut target).is_none());
}
