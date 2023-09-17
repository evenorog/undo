/// This is the edit used in all the examples.
///
/// Not part of the API and can change at any time.
#[doc(hidden)]
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
