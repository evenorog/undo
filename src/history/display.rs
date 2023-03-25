use crate::{At, Entry, Format, History};
use std::fmt;
use std::fmt::Write;
use std::time::SystemTime;

/// Configurable display formatting for the history.
pub struct Display<'a, A, S> {
    history: &'a History<A, S>,
    format: Format,
}

impl<A, S> Display<'_, A, S> {
    /// Show colored output (on by default).
    ///
    /// Requires the `colored` feature to be enabled.
    #[cfg(feature = "colored")]
    pub fn colored(&mut self, on: bool) -> &mut Self {
        self.format.colored = on;
        self
    }

    /// Show the current position in the output (on by default).
    pub fn current(&mut self, on: bool) -> &mut Self {
        self.format.current = on;
        self
    }

    /// Show detailed output (on by default).
    pub fn detailed(&mut self, on: bool) -> &mut Self {
        self.format.detailed = on;
        self
    }

    /// Show the position of the action (on by default).
    pub fn position(&mut self, on: bool) -> &mut Self {
        self.format.position = on;
        self
    }

    /// Show the saved action (on by default).
    pub fn saved(&mut self, on: bool) -> &mut Self {
        self.format.saved = on;
        self
    }
}

impl<A: fmt::Display, S> Display<'_, A, S> {
    fn fmt_list(
        &self,
        f: &mut fmt::Formatter,
        at: At,
        entry: Option<&Entry<A>>,
        level: usize,
        now: SystemTime,
    ) -> fmt::Result {
        self.format.mark(f, level)?;
        self.format.position(f, at, true)?;

        if let Some(entry) = entry {
            if self.format.detailed {
                self.format.elapsed(f, now, entry.created_at)?;
            }
        }

        self.format.labels(
            f,
            at,
            At::new(self.history.branch(), self.history.current()),
            self.history
                .record
                .saved
                .map(|saved| At::new(self.history.branch(), saved))
                .or(self.history.saved),
        )?;

        if let Some(entry) = entry {
            if self.format.detailed {
                writeln!(f)?;
                self.format.message(f, entry, Some(level))?;
            } else {
                f.write_char(' ')?;
                self.format.message(f, entry, Some(level))?;
                writeln!(f)?;
            }
        }
        Ok(())
    }

    fn fmt_graph(
        &self,
        f: &mut fmt::Formatter,
        at: At,
        entry: Option<&Entry<A>>,
        level: usize,
        now: SystemTime,
    ) -> fmt::Result {
        for (&i, branch) in self
            .history
            .branches
            .iter()
            .filter(|(_, branch)| branch.parent == at)
        {
            for (j, entry) in branch.entries.iter().enumerate().rev() {
                let at = At::new(i, j + branch.parent.current + 1);
                self.fmt_graph(f, at, Some(entry), level + 1, now)?;
            }

            for j in 0..level {
                self.format.edge(f, j)?;
                f.write_char(' ')?;
            }

            self.format.split(f, level)?;
            writeln!(f)?;
        }

        for i in 0..level {
            self.format.edge(f, i)?;
            f.write_char(' ')?;
        }

        self.fmt_list(f, at, entry, level, now)
    }
}

impl<'a, A, S> From<&'a History<A, S>> for Display<'a, A, S> {
    fn from(history: &'a History<A, S>) -> Self {
        Display {
            history,
            format: Format::default(),
        }
    }
}

impl<A: fmt::Display, S> fmt::Display for Display<'_, A, S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let now = SystemTime::now();
        let branch = self.history.branch();
        for (i, entry) in self.history.record.entries.iter().enumerate().rev() {
            let at = At::new(branch, i + 1);
            self.fmt_graph(f, at, Some(entry), 0, now)?;
        }
        self.fmt_graph(f, At::new(branch, 0), None, 0, now)
    }
}
