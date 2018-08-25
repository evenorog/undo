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

#[derive(Debug)]
struct JumpAdd(char, String);

impl From<char> for JumpAdd {
    fn from(c: char) -> JumpAdd {
        JumpAdd(c, Default::default())
    }
}

impl Command<String> for JumpAdd {
    fn apply(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.1 = receiver.clone();
        receiver.push(self.0);
        Ok(())
    }

    fn undo(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
        *receiver = self.1.clone();
        Ok(())
    }

    fn redo(&mut self, receiver: &mut String) -> Result<(), Box<dyn Error + Send + Sync>> {
        *receiver = self.1.clone();
        receiver.push(self.0);
        Ok(())
    }
}

impl fmt::Display for JumpAdd {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Add {:?} to {:?}.", self.0, self.1)
    }
}

fn main() {
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
    let abcfg = history.apply(Add('h')).unwrap().unwrap();
    assert!(history.apply(Add('i')).unwrap().is_none());
    assert!(history.apply(Add('j')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abcfhij");
    history.undo().unwrap().unwrap();
    let abcfhij = history.apply(Add('k')).unwrap().unwrap();
    assert_eq!(history.as_receiver(), "abcfhik");
    history.undo().unwrap().unwrap();
    let abcfhik = history.apply(Add('l')).unwrap().unwrap();
    assert_eq!(history.as_receiver(), "abcfhil");
    assert!(history.apply(Add('m')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abcfhilm");
    let abcfhilm = history.go_to(abcde, 2).unwrap().unwrap();
    history.apply(Add('n')).unwrap().unwrap();
    assert!(history.apply(Add('o')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abno");
    history.undo().unwrap().unwrap();
    let abno = history.apply(Add('p')).unwrap().unwrap();
    assert!(history.apply(Add('q')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abnpq");

    let abnpq = history.go_to(abcde, 5).unwrap().unwrap();
    assert_eq!(history.as_receiver(), "abcde");
    assert_eq!(history.go_to(abcfg, 5).unwrap().unwrap(), abcde);
    assert_eq!(history.as_receiver(), "abcfg");
    assert_eq!(history.go_to(abcfhij, 7).unwrap().unwrap(), abcfg);
    assert_eq!(history.as_receiver(), "abcfhij");
    assert_eq!(history.go_to(abcfhik, 7).unwrap().unwrap(), abcfhij);
    assert_eq!(history.as_receiver(), "abcfhik");
    assert_eq!(history.go_to(abcfhilm, 8).unwrap().unwrap(), abcfhik);
    assert_eq!(history.as_receiver(), "abcfhilm");
    assert_eq!(history.go_to(abno, 4).unwrap().unwrap(), abcfhilm);
    assert_eq!(history.as_receiver(), "abno");
    assert_eq!(history.go_to(abnpq, 5).unwrap().unwrap(), abno);
    history.set_saved(true);
    assert_eq!(history.as_receiver(), "abnpq");

    #[cfg(feature = "display")]
    {
        println!("{}", history.display().colored(true));
    }
    /*
    let mut history = History::default();
    assert!(history.apply(JumpAdd::from('a')).unwrap().is_none());
    assert!(history.apply(JumpAdd::from('b')).unwrap().is_none());
    assert!(history.apply(JumpAdd::from('c')).unwrap().is_none());
    assert!(history.apply(JumpAdd::from('d')).unwrap().is_none());
    assert!(history.apply(JumpAdd::from('e')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abcde");
    history.undo().unwrap().unwrap();
    history.undo().unwrap().unwrap();
    assert_eq!(history.as_receiver(), "abc");
    let abcde = history.apply(JumpAdd::from('f')).unwrap().unwrap();
    assert!(history.apply(JumpAdd::from('g')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abcfg");
    history.undo().unwrap().unwrap();
    let abcfg = history.apply(JumpAdd::from('h')).unwrap().unwrap();
    assert!(history.apply(JumpAdd::from('i')).unwrap().is_none());
    assert!(history.apply(JumpAdd::from('j')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abcfhij");
    history.undo().unwrap().unwrap();
    let abcfhij = history.apply(JumpAdd::from('k')).unwrap().unwrap();
    assert_eq!(history.as_receiver(), "abcfhik");
    history.undo().unwrap().unwrap();
    let abcfhik = history.apply(JumpAdd::from('l')).unwrap().unwrap();
    assert_eq!(history.as_receiver(), "abcfhil");
    assert!(history.apply(JumpAdd::from('m')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abcfhilm");
    let abcfhilm = history.jump_to(abcde, 2).unwrap().unwrap();
    history.apply(JumpAdd::from('n')).unwrap().unwrap();
    assert!(history.apply(JumpAdd::from('o')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abno");
    history.undo().unwrap().unwrap();
    let abno = history.apply(JumpAdd::from('p')).unwrap().unwrap();
    assert!(history.apply(JumpAdd::from('q')).unwrap().is_none());
    assert_eq!(history.as_receiver(), "abnpq");

    let abnpq = history.jump_to(abcde, 5).unwrap().unwrap();
    assert_eq!(history.as_receiver(), "abcde");
    assert_eq!(history.jump_to(abcfg, 5).unwrap().unwrap(), abcde);
    assert_eq!(history.as_receiver(), "abcfg");
    assert_eq!(history.jump_to(abcfhij, 7).unwrap().unwrap(), abcfg);
    assert_eq!(history.as_receiver(), "abcfhij");
    assert_eq!(history.jump_to(abcfhik, 7).unwrap().unwrap(), abcfhij);
    assert_eq!(history.as_receiver(), "abcfhik");
    assert_eq!(history.jump_to(abcfhilm, 8).unwrap().unwrap(), abcfhik);
    assert_eq!(history.as_receiver(), "abcfhilm");
    assert_eq!(history.jump_to(abno, 4).unwrap().unwrap(), abcfhilm);
    assert_eq!(history.as_receiver(), "abno");
    assert_eq!(history.jump_to(abnpq, 5).unwrap().unwrap(), abno);
    history.set_saved(true);
    assert_eq!(history.as_receiver(), "abnpq");

    #[cfg(feature = "display")]
    {
        println!("{}", history.display());
    }
    */
}
