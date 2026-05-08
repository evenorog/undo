use std::cell::Cell;
use std::rc::Rc;
use undo::{Add, Edit, Merged, Record};

const A: Add = Add('a');
const B: Add = Add('b');
const C: Add = Add('c');
const D: Add = Add('d');
const E: Add = Add('e');
const F: Add = Add('f');

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
    let collected = record.entries().map(AsRef::as_ref).collect::<Vec<_>>();
    assert_eq!(&collected[..], &[&A, &B][..]);
}

#[test]
fn checkpoint_saved() {
    let mut target = String::new();
    let mut record = Record::new();
    record.edit(&mut target, A);
    record.edit(&mut target, B);
    record.edit(&mut target, C);
    record.set_saved();
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

// An edit that counts how many times Edit::drop has been called.
#[derive(Clone)]
struct CountDrop {
    char: char,
    drops: Rc<Cell<u32>>,
}

impl CountDrop {
    fn new(char: char, drops: &Rc<Cell<u32>>) -> Self {
        CountDrop {
            char,
            drops: drops.clone(),
        }
    }
}

impl Edit for CountDrop {
    type Target = String;
    type Output = ();

    fn edit(&mut self, target: &mut String) {
        target.push(self.char);
    }

    fn undo(&mut self, target: &mut String) {
        target.pop();
    }

    fn drop(&mut self, _target: &mut String) {
        self.drops.set(self.drops.get() + 1);
    }
}

// An edit that annuls its predecessor when pushed.
struct Annulling(char, Rc<Cell<u32>>);

impl Edit for Annulling {
    type Target = String;
    type Output = ();

    fn edit(&mut self, target: &mut String) {
        target.push(self.0);
    }

    fn undo(&mut self, target: &mut String) {
        target.pop();
    }

    fn drop(&mut self, _target: &mut String) {
        self.1.set(self.1.get() + 1);
    }

    fn merge(&mut self, _other: Self) -> Merged<Self> {
        // Any two Annulling edits cancel each other out.
        Merged::Annul
    }
}

#[test]
fn drop_on_discard() {
    let drops = Rc::new(Cell::new(0u32));
    let mut record = Record::new();
    let mut target = String::new();

    record.edit(&mut target, CountDrop::new('a', &drops));
    record.edit(&mut target, CountDrop::new('b', &drops));
    record.edit(&mut target, CountDrop::new('c', &drops));
    assert_eq!(drops.get(), 0); // nothing dropped yet

    record.undo(&mut target);
    record.undo(&mut target);
    assert_eq!(drops.get(), 0); // undo does not drop, edits are still reachable

    // Applying a new edit discards the two undone edits (b and c).
    record.edit(&mut target, CountDrop::new('d', &drops));
    assert_eq!(drops.get(), 2);
    assert_eq!(target, "ad");
}

#[test]
fn drop_on_limit() {
    let drops = Rc::new(Cell::new(0u32));
    let mut record: Record<CountDrop> = Record::builder().limit(3).build();
    let mut target = String::new();

    record.edit(&mut target, CountDrop::new('a', &drops));
    record.edit(&mut target, CountDrop::new('b', &drops));
    record.edit(&mut target, CountDrop::new('c', &drops));
    assert_eq!(drops.get(), 0);

    // 'a' is evicted when the limit is exceeded.
    record.edit(&mut target, CountDrop::new('d', &drops));
    assert_eq!(drops.get(), 1);

    // 'b' is evicted next.
    record.edit(&mut target, CountDrop::new('e', &drops));
    assert_eq!(drops.get(), 2);
}

#[test]
fn drop_on_annul() {
    let drops = Rc::new(Cell::new(0u32));
    let mut record = Record::new();
    let mut target = String::new();

    record.edit(&mut target, Annulling('a', drops.clone()));
    assert_eq!(drops.get(), 0);

    // Pushing a second Annulling removes the first from the record.
    record.edit(&mut target, Annulling('b', drops.clone()));
    assert_eq!(drops.get(), 1); // first entry was dropped
    assert_eq!(record.len(), 0);
}

#[test]
fn drop_on_checkpoint_cancel() {
    let drops = Rc::new(Cell::new(0u32));
    let mut record = Record::new();
    let mut target = String::new();

    record.edit(&mut target, CountDrop::new('a', &drops));
    let mut cp = record.checkpoint();
    cp.edit(&mut target, CountDrop::new('b', &drops));
    cp.edit(&mut target, CountDrop::new('c', &drops));
    assert_eq!(target, "abc");
    assert_eq!(drops.get(), 0);

    cp.cancel(&mut target);
    // b and c were cancelled; Edit::drop is called on each.
    assert_eq!(drops.get(), 2);
    assert_eq!(target, "a");
}

#[test]
fn drop_not_called_on_checkpoint_tail_restore() {
    let drops = Rc::new(Cell::new(0u32));
    let mut record = Record::new();
    let mut target = String::new();

    record.edit(&mut target, CountDrop::new('a', &drops));
    record.edit(&mut target, CountDrop::new('b', &drops));
    record.edit(&mut target, CountDrop::new('c', &drops));
    record.undo(&mut target);
    record.undo(&mut target);
    assert_eq!(target, "a");

    let mut cp = record.checkpoint();
    cp.edit(&mut target, CountDrop::new('d', &drops));
    assert_eq!(target, "ad");
    assert_eq!(drops.get(), 0);

    cp.cancel(&mut target);
    // The checkpoint edit is removed, but the discarded redo tail (b, c) is restored.
    assert_eq!(drops.get(), 1);
    assert_eq!(target, "a");

    record.redo(&mut target);
    record.redo(&mut target);
    assert_eq!(target, "abc");
    assert_eq!(drops.get(), 1);
}

#[test]
fn drop_not_called_on_clear_or_checkpoint_commit() {
    let drops = Rc::new(Cell::new(0u32));
    let mut record = Record::new();
    let mut target = String::new();

    record.edit(&mut target, CountDrop::new('a', &drops));
    {
        let mut cp = record.checkpoint();
        cp.edit(&mut target, CountDrop::new('b', &drops));
        cp.commit();
    }
    assert_eq!(drops.get(), 0);

    record.clear();
    assert_eq!(drops.get(), 0);
    assert!(record.is_empty());
    assert_eq!(target, "ab");
}

struct MarkDrop {
    edit: char,
    drop: char,
}

impl Edit for MarkDrop {
    type Target = String;
    type Output = ();

    fn edit(&mut self, target: &mut String) {
        target.push(self.edit);
    }

    fn undo(&mut self, target: &mut String) {
        target.pop();
    }

    fn drop(&mut self, target: &mut String) {
        target.push(self.drop);
    }
}

#[test]
fn drop_receives_target_and_runs_in_discard_order() {
    let mut record = Record::new();
    let mut target = String::new();

    record.edit(
        &mut target,
        MarkDrop {
            edit: 'a',
            drop: 'A',
        },
    );
    record.edit(
        &mut target,
        MarkDrop {
            edit: 'b',
            drop: 'B',
        },
    );
    record.edit(
        &mut target,
        MarkDrop {
            edit: 'c',
            drop: 'C',
        },
    );
    record.undo(&mut target);
    record.undo(&mut target);

    record.edit(
        &mut target,
        MarkDrop {
            edit: 'd',
            drop: 'D',
        },
    );
    assert_eq!(target, "adBC");
}
