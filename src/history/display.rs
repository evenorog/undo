use crate::{At, Entry, Format, History};
use core::fmt::{self, Write};
#[cfg(feature = "std")]
use std::time::SystemTime;

/// Configurable display formatting for the [`History`].
pub struct Display<'a, E, S> {
    history: &'a History<E, S>,
    format: Format,
    #[cfg(feature = "std")]
    st_fmt: &'a dyn Fn(SystemTime, SystemTime) -> String,
}

impl<'a, E, S> Display<'a, E, S> {
    /// Show colored output (on by default).
    ///
    /// Requires the `colored` feature to be enabled.
    #[cfg(feature = "colored")]
    pub fn colored(&mut self, on: bool) -> &mut Self {
        self.format.colored = on;
        self
    }

    /// Show detailed output (on by default).
    pub fn detailed(&mut self, on: bool) -> &mut Self {
        self.format.detailed = on;
        self
    }

    /// Show the current position in the output (on by default).
    pub fn head(&mut self, on: bool) -> &mut Self {
        self.format.head = on;
        self
    }

    /// Show the saved edit (on by default).
    pub fn saved(&mut self, on: bool) -> &mut Self {
        self.format.saved = on;
        self
    }

    /// Sets the format used to display [`SystemTime`]s.
    ///
    /// The first input parameter is the current system time.
    /// The second input parameter is the system time of the event.
    #[cfg(feature = "std")]
    pub fn set_st_fmt(
        &mut self,
        st_fmt: &'a dyn Fn(SystemTime, SystemTime) -> String,
    ) -> &mut Self {
        self.st_fmt = st_fmt;
        self
    }
}

impl<E: fmt::Display, S> Display<'_, E, S> {
    fn fmt_list(
        &self,
        f: &mut fmt::Formatter,
        at: At,
        entry: Option<&Entry<E>>,
        level: usize,
        #[cfg(feature = "std")] now: SystemTime,
    ) -> fmt::Result {
        self.format.mark(f, level)?;
        self.format.position(f, at, true)?;

        #[cfg(feature = "std")]
        if let Some(entry) = entry {
            if self.format.detailed {
                let st_fmt = self.st_fmt;
                self.format.elapsed(f, st_fmt(now, entry.created_at))?;
                self.format.text(f, ",", 3)?;
                self.format.elapsed(f, st_fmt(now, entry.updated_at))?;
            }
        }

        self.format.labels(
            f,
            at,
            self.history.head(),
            self.history
                .record
                .saved
                .map(|saved| At::new(self.history.root, saved))
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
        entry: Option<&Entry<E>>,
        level: usize,
        #[cfg(feature = "std")] now: SystemTime,
    ) -> fmt::Result {
        for (&i, branch) in self
            .history
            .branches
            .iter()
            .filter(|(_, branch)| branch.parent == at)
        {
            for (j, entry) in branch.entries.iter().enumerate().rev() {
                let at = At::new(i, j + branch.parent.index + 1);
                self.fmt_graph(
                    f,
                    at,
                    Some(entry),
                    level + 1,
                    #[cfg(feature = "std")]
                    now,
                )?;
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

        self.fmt_list(
            f,
            at,
            entry,
            level,
            #[cfg(feature = "std")]
            now,
        )
    }
}

impl<'a, E, S> From<&'a History<E, S>> for Display<'a, E, S> {
    fn from(history: &'a History<E, S>) -> Self {
        Display {
            history,
            format: Format::default(),
            #[cfg(feature = "std")]
            st_fmt: &crate::format::default_st_fmt,
        }
    }
}

impl<E: fmt::Display, S> fmt::Display for Display<'_, E, S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[cfg(feature = "std")]
        let now = SystemTime::now();
        let branch = self.history.root;
        for (i, entry) in self.history.record.entries.iter().enumerate().rev() {
            let at = At::new(branch, i + 1);
            self.fmt_graph(
                f,
                at,
                Some(entry),
                0,
                #[cfg(feature = "std")]
                now,
            )?;
        }
        self.fmt_graph(
            f,
            At::new(branch, 0),
            None,
            0,
            #[cfg(feature = "std")]
            now,
        )
    }
}
