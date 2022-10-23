use crate::Action;
use alloc::boxed::Box;
use core::fmt::{self, Debug, Formatter};

/// Any action that shares the target, output, and error.
pub struct AnyAction<T, O, E> {
    id: i32,
    action: Box<dyn Action<Target = T, Output = O, Error = E>>,
}

impl<T, O, E> AnyAction<T, O, E> {
    /// Creates an `AnyAction` from the provided action.
    pub fn new<A>(action: A) -> AnyAction<T, O, E>
    where
        A: Action<Target = T, Output = O, Error = E>,
        A: 'static,
    {
        AnyAction {
            id: 0,
            action: Box::new(action),
        }
    }
}

impl<T, O, E> Action for AnyAction<T, O, E> {
    type Target = T;
    type Output = O;
    type Error = E;

    fn apply(&mut self, target: &mut Self::Target) -> crate::Result<Self> {
        self.action.apply(target)
    }

    fn undo(&mut self, target: &mut Self::Target) -> crate::Result<Self> {
        self.action.undo(target)
    }

    fn redo(&mut self, target: &mut Self::Target) -> crate::Result<Self> {
        self.action.redo(target)
    }
}

impl<T, O, E> Debug for AnyAction<T, O, E> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("AnyAction")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Action, AnyAction, Record, Result};
    use alloc::string::String;

    struct Add(char);

    impl Action for Add {
        type Target = String;
        type Output = ();
        type Error = &'static str;

        fn apply(&mut self, s: &mut String) -> Result<Add> {
            s.push(self.0);
            Ok(())
        }

        fn undo(&mut self, s: &mut String) -> Result<Add> {
            self.0 = s.pop().ok_or("s is empty")?;
            Ok(())
        }
    }

    #[test]
    fn any() {
        let mut target = String::new();
        let mut record = Record::new();
        record.apply(&mut target, AnyAction::new(Add('a'))).unwrap();
        assert_eq!(target, "a");
        record.undo(&mut target).unwrap().unwrap();
        assert_eq!(target, "");
        record.redo(&mut target).unwrap().unwrap();
        assert_eq!(target, "a");
    }
}
