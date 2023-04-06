use core::fmt::{self, Display, Formatter};
use undo::{Action, History};

struct Push(char);

impl Action for Push {
    type Target = String;
    type Output = ();

    fn apply(&mut self, string: &mut String) {
        string.push(self.0);
    }

    fn undo(&mut self, string: &mut String) {
        self.0 = string.pop().expect("cannot pop empty string");
    }
}

impl Display for Push {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Push '{}'", self.0)
    }
}

fn main() {
    let mut history = History::new();
    let mut target = String::new();

    history.apply(&mut target, Push('a'));
    history.apply(&mut target, Push('b'));
    history.apply(&mut target, Push('c'));
    assert_eq!(target, "abc");

    let abc_branch = history.branch();
    let abc_current = history.current();

    history.undo(&mut target);
    assert_eq!(target, "ab");

    history.apply(&mut target, Push('d'));
    history.apply(&mut target, Push('e'));
    history.apply(&mut target, Push('f'));
    assert_eq!(target, "abdef");

    let abdef_branch = history.branch();
    let abdef_current = history.current();

    history.go_to(&mut target, abc_branch, abc_current);
    assert_eq!(target, "abc");

    history.go_to(&mut target, abdef_branch, abdef_current);
    assert_eq!(target, "abdef");

    println!("{}", history.display());
}
