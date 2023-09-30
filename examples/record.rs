use undo::{Add, Record};

fn main() {
    let mut target = String::new();
    let mut record = Record::new();

    record.edit(&mut target, Add('a'));
    record.edit(&mut target, Add('b'));
    record.edit(&mut target, Add('c'));
    record.edit(&mut target, Add('d'));
    record.edit(&mut target, Add('e'));
    record.edit(&mut target, Add('f'));
    assert_eq!(target, "abcdef");

    record.set_saved();

    record.undo(&mut target);
    record.undo(&mut target);
    assert_eq!(target, "abcd");

    println!("{}", record.display());
}
