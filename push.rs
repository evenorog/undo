pub struct Push(char);

impl undo::Action for Push {
    type Target = String;
    type Output = ();

    fn apply(&mut self, s: &mut String) {
        s.push(self.0);
    }

    fn undo(&mut self, s: &mut String) {
        self.0 = s.pop().unwrap();
    }
}
