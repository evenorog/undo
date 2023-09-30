use undo::{Add, Record};

const A: Add = Add('a');
const B: Add = Add('b');
const C: Add = Add('c');
const D: Add = Add('d');
const E: Add = Add('e');
const F: Add = Add('f');
const G: Add = Add('g');
const H: Add = Add('h');
const I: Add = Add('i');

#[test]
fn go_to() {
    let mut target = String::new();
    let mut record = Record::new();
    record.edit(&mut target, A);
    record.edit(&mut target, B);
    record.edit(&mut target, C);
    record.edit(&mut target, D);
    record.edit(&mut target, E);

    record.go_to(&mut target, 0);
    assert_eq!(record.head(), 0);
    assert_eq!(target, "");
    record.go_to(&mut target, 5);
    assert_eq!(record.head(), 5);
    assert_eq!(target, "abcde");
    record.go_to(&mut target, 1);
    assert_eq!(record.head(), 1);
    assert_eq!(target, "a");
    record.go_to(&mut target, 4);
    assert_eq!(record.head(), 4);
    assert_eq!(target, "abcd");
    record.go_to(&mut target, 2);
    assert_eq!(record.head(), 2);
    assert_eq!(target, "ab");
    record.go_to(&mut target, 3);
    assert_eq!(record.head(), 3);
    assert_eq!(target, "abc");
    assert!(record.go_to(&mut target, 6).is_empty());
    assert_eq!(record.head(), 3);
}

#[test]
fn edits() {
    let mut target = String::new();
    let mut record = Record::new();
    record.edit(&mut target, A);
    record.edit(&mut target, B);
    let collected = record.entries().map(|e| e.get()).collect::<Vec<_>>();
    assert_eq!(&collected[..], &[&A, &B][..]);
}

#[test]
fn checkpoint_commit() {
    let mut target = String::new();
    let mut record = Record::new();
    let mut cp1 = record.checkpoint();
    cp1.edit(&mut target, A);
    cp1.edit(&mut target, B);
    cp1.edit(&mut target, C);
    assert_eq!(target, "abc");
    let mut cp2 = cp1.checkpoint();
    cp2.edit(&mut target, D);
    cp2.edit(&mut target, E);
    cp2.edit(&mut target, F);
    assert_eq!(target, "abcdef");
    let mut cp3 = cp2.checkpoint();
    cp3.edit(&mut target, G);
    cp3.edit(&mut target, H);
    cp3.edit(&mut target, I);
    assert_eq!(target, "abcdefghi");
    cp3.commit();
    cp2.commit();
    cp1.commit();
    assert_eq!(target, "abcdefghi");
}

#[test]
fn checkpoint_cancel() {
    let mut target = String::new();
    let mut record = Record::new();
    let mut cp1 = record.checkpoint();
    cp1.edit(&mut target, A);
    cp1.edit(&mut target, B);
    cp1.edit(&mut target, C);
    let mut cp2 = cp1.checkpoint();
    cp2.edit(&mut target, D);
    cp2.edit(&mut target, E);
    cp2.edit(&mut target, F);
    let mut cp3 = cp2.checkpoint();
    cp3.edit(&mut target, G);
    cp3.edit(&mut target, H);
    cp3.edit(&mut target, I);
    assert_eq!(target, "abcdefghi");
    cp3.cancel(&mut target);
    assert_eq!(target, "abcdef");
    cp2.cancel(&mut target);
    assert_eq!(target, "abc");
    cp1.cancel(&mut target);
    assert_eq!(target, "");
}

#[test]
fn checkpoint_saved() {
    let mut target = String::new();
    let mut record = Record::new();
    record.edit(&mut target, A);
    record.edit(&mut target, B);
    record.edit(&mut target, C);
    record.set_saved(true);
    record.undo(&mut target).unwrap();
    record.undo(&mut target).unwrap();
    record.undo(&mut target).unwrap();
    let mut cp = record.checkpoint();
    cp.edit(&mut target, D);
    cp.edit(&mut target, E);
    cp.edit(&mut target, F);
    assert_eq!(target, "def");
    cp.cancel(&mut target);
    assert_eq!(target, "");
    record.redo(&mut target).unwrap();
    record.redo(&mut target).unwrap();
    record.redo(&mut target).unwrap();
    assert!(record.is_saved());
    assert_eq!(target, "abc");
}

#[test]
fn queue_commit() {
    let mut target = String::new();
    let mut record = Record::new();
    let mut q1 = record.queue();
    q1.redo();
    q1.redo();
    q1.redo();
    let mut q2 = q1.queue();
    q2.undo();
    q2.undo();
    q2.undo();
    let mut q3 = q2.queue();
    q3.edit(A);
    q3.edit(B);
    q3.edit(C);
    assert_eq!(target, "");
    q3.commit(&mut target);
    assert_eq!(target, "abc");
    q2.commit(&mut target);
    assert_eq!(target, "");
    q1.commit(&mut target);
    assert_eq!(target, "abc");
}
