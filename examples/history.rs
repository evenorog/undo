use chrono::{DateTime, Local};
use std::time::SystemTime;
use undo::{Add, History};

fn custom_st_fmt(_: SystemTime, at: SystemTime) -> String {
    let dt = DateTime::<Local>::from(at);
    dt.time().to_string()
}

fn main() {
    let mut target = String::new();
    let mut history = History::new();

    history.edit(&mut target, Add('a'));
    history.edit(&mut target, Add('b'));
    history.edit(&mut target, Add('c'));
    assert_eq!(target, "abc");

    let abc = history.head();

    history.undo(&mut target);
    assert_eq!(target, "ab");

    history.edit(&mut target, Add('d'));
    history.edit(&mut target, Add('e'));
    history.edit(&mut target, Add('f'));
    assert_eq!(target, "abdef");

    let abdef = history.head();

    history.go_to(&mut target, abc);
    assert_eq!(target, "abc");

    history.go_to(&mut target, abdef);
    assert_eq!(target, "abdef");

    println!("{}", history.display().set_st_fmt(&custom_st_fmt));
}
