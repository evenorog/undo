use chrono::{DateTime, Local};
use std::time::SystemTime;
use undo::{Add, Record};

fn custom_st_fmt(_: SystemTime, at: SystemTime) -> String {
    let dt = DateTime::<Local>::from(at);
    dt.time().to_string()
}

fn main() {
    let mut target = String::new();
    let mut record = Record::new();

    record.edit(&mut target, Add('a'));
    record.edit(&mut target, Add('b'));
    record.edit(&mut target, Add('c'));
    assert_eq!(target, "abc");

    record.undo(&mut target);
    record.undo(&mut target);
    assert_eq!(target, "a");

    record.redo(&mut target);
    record.redo(&mut target);
    assert_eq!(target, "abc");

    println!("{}", record.display().set_st_fmt(&custom_st_fmt));
}
