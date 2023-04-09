use core::fmt::{self, Display, Formatter};
use undo::{Edit, History};

struct Add(char);

impl Edit for Add {
    type Target = String;
    type Output = ();

    fn edit(&mut self, string: &mut String) {
        string.push(self.0);
    }

    fn undo(&mut self, string: &mut String) {
        self.0 = string.pop().expect("cannot pop empty string");
    }
}

impl Display for Add {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Add '{}'", self.0)
    }
}

fn main() {
    let mut target = String::new();
    let mut history = History::new();

    history.edit(&mut target, Add('a'));
    history.edit(&mut target, Add('b'));
    history.edit(&mut target, Add('c'));
    assert_eq!(target, "abc");

    let abc_branch = history.branch();
    let abc_current = history.current();

    history.undo(&mut target);
    assert_eq!(target, "ab");

    history.edit(&mut target, Add('d'));
    history.edit(&mut target, Add('e'));
    history.edit(&mut target, Add('f'));
    assert_eq!(target, "abdef");

    let abdef_branch = history.branch();
    let abdef_current = history.current();

    history.go_to(&mut target, abc_branch, abc_current);
    assert_eq!(target, "abc");

    history.go_to(&mut target, abdef_branch, abdef_current);
    assert_eq!(target, "abdef");

    println!("{}", history.display());
}
