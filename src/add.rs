use alloc::string::String;
use core::fmt::{self, Display, Formatter};

/// This is the edit used in all the examples.
///
/// Not part of the API and can change at any time.
#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct Add(pub char);

impl crate::Edit for Add {
    type Target = String;
    type Output = ();

    fn edit(&mut self, string: &mut String) {
        string.push(self.0);
    }

    fn undo(&mut self, string: &mut String) {
        self.0 = string.pop().unwrap();
    }
}

impl Display for Add {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Add '{}'", self.0)
    }
}
