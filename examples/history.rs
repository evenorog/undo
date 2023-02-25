use undo::{Action, History};

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
    let mut history = History::new();
    let mut target = String::new();

    history.apply(&mut target, Push('a'));
    history.apply(&mut target, Push('b'));
    history.apply(&mut target, Push('c'));
    assert_eq!(target, "abc");

    let (branch_one, current_one) = (history.branch(), history.current());

    history.undo(&mut target);
    assert_eq!(target, "ab");

    history.apply(&mut target, Push('d'));
    history.apply(&mut target, Push('e'));
    history.apply(&mut target, Push('f'));
    assert_eq!(target, "abdef");

    let (branch_two, current_two) = (history.branch(), history.current());

    history.go_to(&mut target, branch_one, current_one);
    assert_eq!(target, "abc");

    history.go_to(&mut target, branch_two, current_two);
    assert_eq!(target, "abdef");

    println!("History: {}", history.display());
}
