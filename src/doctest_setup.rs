pub struct Push(char);

impl undo::Action for Push {
    type Target = String;
    type Output = ();

    fn apply(&mut self, string: &mut String) {
        string.push(self.0);
    }

    fn undo(&mut self, string: &mut String) {
        self.0 = string
            .pop()
            .expect("cannot remove more characters than have been added");
    }
}
