use core::fmt::{self, Display, Formatter};
use undo::{Edit, History};

struct Push(char);

impl Edit for Push {
    type Target = String;
    type Output = ();

    fn edit(&mut self, string: &mut String) {
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
    let mut target = String::new();
    let mut history = History::new();

    history.edit(&mut target, Push('a'));
    history.edit(&mut target, Push('b'));
    history.edit(&mut target, Push('c'));
    assert_eq!(target, "abc");

    let abc_branch = history.branch();
    let abc_current = history.current();

    history.undo(&mut target);
    assert_eq!(target, "ab");

    history.edit(&mut target, Push('d'));
    history.edit(&mut target, Push('e'));
    history.edit(&mut target, Push('f'));
    assert_eq!(target, "abdef");

    let abdef_branch = history.branch();
    let abdef_current = history.current();

    history.go_to(&mut target, abc_branch, abc_current);
    assert_eq!(target, "abc");

    history.go_to(&mut target, abdef_branch, abdef_current);
    assert_eq!(target, "abdef");

    println!("{}", history.display());
}
