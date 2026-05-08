use std::cell::Cell;
use std::rc::Rc;
use undo::{Add, At, Edit, History};

const A: Add = Add('a');
const B: Add = Add('b');
const C: Add = Add('c');
const D: Add = Add('d');
const E: Add = Add('e');
const F: Add = Add('f');
const G: Add = Add('g');
const H: Add = Add('h');
const I: Add = Add('i');
const J: Add = Add('j');
const K: Add = Add('k');
const L: Add = Add('l');
const M: Add = Add('m');
const N: Add = Add('n');
const O: Add = Add('o');
const P: Add = Add('p');
const Q: Add = Add('q');

#[test]
fn go_to() {
    //          m
    //          |
    //    j  k  l
    //     \ | /
    //       i
    //       |
    // e  g  h
    // |  | /
    // d  f  p - q *
    // | /  /
    // c  n - o
    // | /
    // b
    // |
    // a
    let mut target = String::new();
    let mut history = History::new();
    history.edit(&mut target, A);
    history.edit(&mut target, B);
    history.edit(&mut target, C);
    history.edit(&mut target, D);
    history.edit(&mut target, E);
    assert_eq!(target, "abcde");
    history.undo(&mut target).unwrap();
    history.undo(&mut target).unwrap();
    assert_eq!(target, "abc");
    let abc = history.head();

    history.edit(&mut target, F);
    history.edit(&mut target, G);
    assert_eq!(target, "abcfg");
    let abcfg = history.head();

    history.undo(&mut target).unwrap();
    history.edit(&mut target, H);
    history.edit(&mut target, I);
    history.edit(&mut target, J);
    assert_eq!(target, "abcfhij");
    let abcfhij = history.head();

    history.undo(&mut target).unwrap();
    history.edit(&mut target, K);
    assert_eq!(target, "abcfhik");
    let abcfhik = history.head();

    history.undo(&mut target).unwrap();
    history.edit(&mut target, L);
    assert_eq!(target, "abcfhil");
    history.edit(&mut target, M);
    assert_eq!(target, "abcfhilm");
    let abcfhilm = history.head();
    history.go_to(&mut target, At::new(abc.root, 2));
    history.edit(&mut target, N);
    history.edit(&mut target, O);
    assert_eq!(target, "abno");
    let abno = history.head();

    history.undo(&mut target).unwrap();
    history.edit(&mut target, P);
    history.edit(&mut target, Q);
    assert_eq!(target, "abnpq");

    let abnpq = history.head();
    history.go_to(&mut target, abc);
    assert_eq!(target, "abc");
    history.go_to(&mut target, abcfg);
    assert_eq!(target, "abcfg");
    history.go_to(&mut target, abcfhij);
    assert_eq!(target, "abcfhij");
    history.go_to(&mut target, abcfhik);
    assert_eq!(target, "abcfhik");
    history.go_to(&mut target, abcfhilm);
    assert_eq!(target, "abcfhilm");
    history.go_to(&mut target, abno);
    assert_eq!(target, "abno");
    history.go_to(&mut target, abnpq);
    assert_eq!(target, "abnpq");
}

#[test]
fn checkpoint() {
    let mut target = String::new();
    let mut history = History::new();
    let mut checkpoint = history.checkpoint();

    checkpoint.edit(&mut target, A);
    checkpoint.edit(&mut target, B);
    checkpoint.edit(&mut target, C);
    assert_eq!(target, "abc");

    checkpoint.undo(&mut target);
    checkpoint.undo(&mut target);
    assert_eq!(target, "a");

    checkpoint.edit(&mut target, D);
    checkpoint.edit(&mut target, E);
    assert_eq!(target, "ade");

    checkpoint.cancel(&mut target);
    assert_eq!(target, "");
}

#[test]
fn next_and_prev() {
    let mut target = String::new();
    let mut history = History::new();
    history.edit(&mut target, A);
    history.edit(&mut target, B);
    history.edit(&mut target, C);
    assert_eq!(history.next_branch_head(), None);
    assert_eq!(history.prev_branch_head(), None);

    history.undo(&mut target).unwrap();
    history.undo(&mut target).unwrap();
    history.edit(&mut target, D);
    history.edit(&mut target, E);
    assert_eq!(history.next_branch_head(), None);
    let prev_head = history.prev_branch_head().unwrap();
    assert_eq!(prev_head, At::new(0, 2));

    history.go_to(&mut target, prev_head);
    assert_eq!(history.next_branch_head(), Some(At::new(1, 2)));
    assert_eq!(history.prev_branch_head(), None);
}

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

#[test]
fn drop_not_called_on_branch() {
    // Unlike Record, History never discards edits when branching — undone edits
    // are preserved in a branch and remain accessible via go_to, so drop() is
    // not called on them.
    let drops = Rc::new(Cell::new(0u32));
    let mut history = History::new();
    let mut target = String::new();

    history.edit(&mut target, CountDrop::new('a', &drops));
    history.edit(&mut target, CountDrop::new('b', &drops));
    history.edit(&mut target, CountDrop::new('c', &drops));

    history.undo(&mut target);
    history.undo(&mut target);
    assert_eq!(drops.get(), 0);

    // Editing after undoing creates a branch for b and c — they are NOT dropped.
    history.edit(&mut target, CountDrop::new('d', &drops));
    assert_eq!(drops.get(), 0); // b and c are in a branch, not gone
}

#[test]
fn drop_on_limit() {
    let drops = Rc::new(Cell::new(0u32));
    let mut history: History<CountDrop> = History::builder().limit(3).build();
    let mut target = String::new();

    history.edit(&mut target, CountDrop::new('a', &drops));
    history.edit(&mut target, CountDrop::new('b', &drops));
    history.edit(&mut target, CountDrop::new('c', &drops));
    assert_eq!(drops.get(), 0);

    // 'a' is evicted when the limit is exceeded.
    history.edit(&mut target, CountDrop::new('d', &drops));
    assert_eq!(drops.get(), 1);
}

#[test]
fn drop_on_branch_evicted_by_limit() {
    // When the oldest edit is evicted by the limit, any branches parented at
    // that position are removed and their entries have drop() called.
    let drops = Rc::new(Cell::new(0u32));
    let mut history: History<CountDrop> = History::builder().limit(2).build();
    let mut target = String::new();

    history.edit(&mut target, CountDrop::new('a', &drops));
    history.edit(&mut target, CountDrop::new('b', &drops));

    // Undo all and branch from the origin — the [a, b] branch is parented at
    // index 0 (the origin), so it will be removed the moment any eviction
    // reaches index 0.
    history.undo(&mut target);
    history.undo(&mut target);
    history.edit(&mut target, CountDrop::new('c', &drops)); // branch for [a, b]
    history.edit(&mut target, CountDrop::new('d', &drops)); // record = [c, d]
    assert_eq!(drops.get(), 0);

    // Pushing 'e' evicts 'c' (limit=2) and calls rm_child_of on index 0,
    // which removes the [a, b] branch since it was parented at the origin.
    history.edit(&mut target, CountDrop::new('e', &drops));
    assert_eq!(drops.get(), 3); // c evicted + a and b from the pruned branch
}

#[test]
fn drop_on_checkpoint_cancel() {
    let drops = Rc::new(Cell::new(0u32));
    let mut history = History::new();
    let mut target = String::new();

    history.edit(&mut target, CountDrop::new('a', &drops));
    let mut cp = history.checkpoint();
    cp.edit(&mut target, CountDrop::new('b', &drops));
    cp.edit(&mut target, CountDrop::new('c', &drops));
    assert_eq!(target, "abc");
    assert_eq!(drops.get(), 0);

    cp.cancel(&mut target);
    // b and c are cancelled and dropped.
    assert_eq!(drops.get(), 2);
    assert_eq!(target, "a");
}

#[test]
fn drop_not_called_when_navigating_branches() {
    let drops = Rc::new(Cell::new(0u32));
    let mut history = History::new();
    let mut target = String::new();

    history.edit(&mut target, CountDrop::new('a', &drops));
    history.edit(&mut target, CountDrop::new('b', &drops));
    history.edit(&mut target, CountDrop::new('c', &drops));
    let abc = history.head();

    history.undo(&mut target);
    history.undo(&mut target);
    history.edit(&mut target, CountDrop::new('d', &drops));
    let ad = history.head();

    history.go_to(&mut target, abc);
    assert_eq!(target, "abc");
    history.go_to(&mut target, ad);
    assert_eq!(target, "ad");
    assert_eq!(drops.get(), 0);
}

#[test]
fn drop_on_checkpoint_cancel_after_fork_restores_branch() {
    let drops = Rc::new(Cell::new(0u32));
    let mut history = History::new();
    let mut target = String::new();

    history.edit(&mut target, CountDrop::new('a', &drops));
    history.edit(&mut target, CountDrop::new('b', &drops));
    history.edit(&mut target, CountDrop::new('c', &drops));
    history.undo(&mut target);
    history.undo(&mut target);

    let mut cp = history.checkpoint();
    cp.edit(&mut target, CountDrop::new('d', &drops));
    assert_eq!(target, "ad");
    assert_eq!(drops.get(), 0);

    cp.cancel(&mut target);
    // The checkpoint fork is removed, but the original b/c branch is restored.
    assert_eq!(drops.get(), 1);
    assert_eq!(target, "a");

    history.redo(&mut target);
    history.redo(&mut target);
    assert_eq!(target, "abc");
    assert_eq!(drops.get(), 1);
}

#[test]
fn drop_not_called_on_clear_or_checkpoint_commit() {
    let drops = Rc::new(Cell::new(0u32));
    let mut history = History::new();
    let mut target = String::new();

    history.edit(&mut target, CountDrop::new('a', &drops));
    {
        let mut cp = history.checkpoint();
        cp.edit(&mut target, CountDrop::new('b', &drops));
        cp.commit();
    }
    assert_eq!(drops.get(), 0);

    history.clear();
    assert_eq!(drops.get(), 0);
    assert!(history.is_empty());
    assert_eq!(target, "ab");
}
