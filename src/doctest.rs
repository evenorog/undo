// This file is included in the documentation examples to avoid some boilerplate.

/// This is the edit used in all the examples.
pub struct Push(char);

impl undo::Edit for Push {
    type Target = String;
    type Output = ();

    fn edit(&mut self, string: &mut String) {
        string.push(self.0);
    }

    fn undo(&mut self, string: &mut String) {
        self.0 = string.pop().unwrap();
    }
}
