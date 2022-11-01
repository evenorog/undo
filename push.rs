pub struct Push(char);

impl undo::Action for Push {
    type Target = String;
    type Output = ();
    type Error = &'static str;

    fn apply(&mut self, s: &mut String) -> undo::Result<Push> {
        s.push(self.0);
        Ok(())
    }

    fn undo(&mut self, s: &mut String) -> undo::Result<Push> {
        self.0 = s.pop().ok_or("s is empty")?;
        Ok(())
    }
}
