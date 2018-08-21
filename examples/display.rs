extern crate undo;

use std::{error::Error, fmt};
use undo::{Command, History};

#[derive(Debug)]
struct Add(char);

impl Command<String> for Add {
    fn apply(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
        receiver.push(self.0);
        Ok(())
    }

    fn undo(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.0 = receiver.pop().ok_or("`receiver` is empty")?;
        Ok(())
    }
}

impl fmt::Display for Add {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Add '{}'.", self.0)
    }
}

fn main() {
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
    let mut history = History::default();
    assert!(history.apply(Add('a')).unwrap().is_none());
    assert!(history.apply(Add('b')).unwrap().is_none());
    assert!(history.apply(Add('c')).unwrap().is_none());
    assert!(history.apply(Add('d')).unwrap().is_none());
    assert!(history.apply(Add('e')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abcde");
    history.undo().unwrap().unwrap();
    history.undo().unwrap().unwrap();
    assert_eq!(history.as_receiver(), "abc");
    let abcde = history.apply(Add('f')).unwrap().unwrap();
    assert!(history.apply(Add('g')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abcfg");
    history.undo().unwrap().unwrap();
    let _ = history.apply(Add('h')).unwrap().unwrap();
    assert!(history.apply(Add('i')).unwrap().is_none());
    assert!(history.apply(Add('j')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abcfhij");
    history.undo().unwrap().unwrap();
    let _ = history.apply(Add('k')).unwrap().unwrap();
    assert_eq!(history.as_receiver(), "abcfhik");
    history.undo().unwrap().unwrap();
    let _ = history.apply(Add('l')).unwrap().unwrap();
    assert_eq!(history.as_receiver(), "abcfhil");
    assert!(history.apply(Add('m')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abcfhilm");
    let _ = history.go_to(abcde, 2).unwrap().unwrap();
    history.apply(Add('n')).unwrap().unwrap();
    assert!(history.apply(Add('o')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abno");
    history.undo().unwrap().unwrap();
    let _ = history.apply(Add('p')).unwrap().unwrap();
    assert!(history.apply(Add('q')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abnpq");
    history.set_saved(true);
    history.undo().unwrap().unwrap();
    history.undo().unwrap().unwrap();

    #[cfg(feature = "display")]
    {
        let view = history.display();
        println!("{}", view);
    }
}
