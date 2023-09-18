use chrono::{DateTime, Local};
use std::time::SystemTime;
use undo::{Add, History};

fn custom_st_fmt(_: SystemTime, at: SystemTime) -> String {
    let time: DateTime<Local> = at.into();
    time.time().to_string()
}

fn main() {
    let mut target = String::new();
    let mut history = History::new();

    history.edit(&mut target, Add('a'));
    history.edit(&mut target, Add('b'));
    history.edit(&mut target, Add('c'));
    assert_eq!(target, "abc");

    let abc_branch = history.branch();
    let abc_current = history.index();

    history.undo(&mut target);
    assert_eq!(target, "ab");

    history.edit(&mut target, Add('d'));
    history.edit(&mut target, Add('e'));
    history.edit(&mut target, Add('f'));
    assert_eq!(target, "abdef");

    let abdef_branch = history.branch();
    let abdef_current = history.index();

    history.go_to(&mut target, abc_branch, abc_current);
    assert_eq!(target, "abc");

    history.go_to(&mut target, abdef_branch, abdef_current);
    assert_eq!(target, "abdef");

    println!("{}", history.display().set_st_fmt(&custom_st_fmt));
}
