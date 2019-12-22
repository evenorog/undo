use crate::{At, Entry, History, Record};
#[cfg(feature = "chrono")]
use chrono::{DateTime, Local, Utc};
use colored::{Color, Colorize};
use std::fmt::{self, Write};

/// Configurable display formatting of structures.
///
/// # Examples
/// ```no_run
/// # use undo::{Command, History};
/// # fn foo() -> History<String> {
/// let history = History::default();
/// println!("{}", history.display().colored(true).detailed(false));
/// # history
/// # }
/// ```
#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct Display<'a, T> {
    data: &'a T,
    config: Config,
}

impl<T> Display<'_, T> {
    /// Show colored output (off by default).
    #[inline]
    pub fn colored(&mut self, on: bool) -> &mut Self {
        self.config.colored = on;
        self
    }

    /// Show the current position in the output (on by default).
    #[inline]
    pub fn current(&mut self, on: bool) -> &mut Self {
        self.config.current = on;
        self
    }

    /// Show detailed output (on by default).
    #[inline]
    pub fn detailed(&mut self, on: bool) -> &mut Self {
        self.config.detailed = on;
        self
    }

    /// Show the position of the command (on by default).
    #[inline]
    pub fn position(&mut self, on: bool) -> &mut Self {
        self.config.position = on;
        self
    }

    /// Show the saved command (on by default).
    #[inline]
    pub fn saved(&mut self, on: bool) -> &mut Self {
        self.config.saved = on;
        self
    }
}

impl<T> Display<'_, Record<T>> {
    #[inline]
    fn fmt_list(&self, f: &mut fmt::Formatter, at: At, entry: &Entry<T>) -> fmt::Result {
        self.config.mark(f, 0)?;
        self.config.position(f, at, false)?;
        if self.config.detailed {
            #[cfg(feature = "chrono")]
            self.config.timestamp(f, &entry.timestamp)?;
        }
        self.config
            .current(f, at, At::new(0, self.data.current()))?;
        self.config
            .saved(f, at, self.data.saved.map(|saved| At::new(0, saved)))?;
        if self.config.detailed {
            writeln!(f)?;
            self.config.message(f, entry, 0)
        } else {
            f.write_char(' ')?;
            self.config.message(f, entry, 0)?;
            writeln!(f)
        }
    }
}

impl<T> Display<'_, History<T>> {
    #[inline]
    fn fmt_list(
        &self,
        f: &mut fmt::Formatter,
        at: At,
        entry: &Entry<T>,
        level: usize,
    ) -> fmt::Result {
        self.config.mark(f, level)?;
        self.config.position(f, at, true)?;
        if self.config.detailed {
            #[cfg(feature = "chrono")]
            self.config.timestamp(f, &entry.timestamp)?;
        }
        self.config
            .current(f, at, At::new(self.data.branch(), self.data.current()))?;
        self.config.saved(
            f,
            at,
            self.data
                .record
                .saved
                .map(|saved| At::new(self.data.branch(), saved))
                .or(self.data.saved),
        )?;
        if self.config.detailed {
            writeln!(f)?;
            self.config.message(f, entry, level)
        } else {
            f.write_char(' ')?;
            self.config.message(f, entry, level)?;
            writeln!(f)
        }
    }

    #[inline]
    fn fmt_graph(
        &self,
        f: &mut fmt::Formatter,
        at: At,
        entry: &Entry<T>,
        level: usize,
    ) -> fmt::Result {
        for (&i, branch) in self
            .data
            .branches
            .iter()
            .filter(|(_, branch)| branch.parent == at)
        {
            for (j, cmd) in branch.entries.iter().enumerate().rev() {
                let at = At::new(i, j + branch.parent.current + 1);
                self.fmt_graph(f, at, cmd, level + 1)?;
            }
            for j in 0..level {
                self.config.edge(f, j)?;
                f.write_char(' ')?;
            }
            self.config.split(f, level)?;
            writeln!(f)?;
        }
        for i in 0..level {
            self.config.edge(f, i)?;
            f.write_char(' ')?;
        }
        self.fmt_list(f, at, entry, level)
    }
}

impl<'a, T> From<&'a T> for Display<'a, T> {
    #[inline]
    fn from(data: &'a T) -> Self {
        Display {
            data,
            config: Config::default(),
        }
    }
}

impl<T> fmt::Display for Display<'_, Record<T>> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, entry) in self.data.entries.iter().enumerate().rev() {
            self.fmt_list(f, At::new(0, i + 1), entry)?;
        }
        Ok(())
    }
}

impl<T> fmt::Display for Display<'_, History<T>> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, cmd) in self.data.record.entries.iter().enumerate().rev() {
            let at = At::new(self.data.branch(), i + 1);
            self.fmt_graph(f, at, cmd, 0)?;
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
struct Config {
    colored: bool,
    current: bool,
    detailed: bool,
    position: bool,
    saved: bool,
}

impl Default for Config {
    #[inline]
    fn default() -> Self {
        Config {
            colored: false,
            current: true,
            detailed: true,
            position: true,
            saved: true,
        }
    }
}

impl Config {
    #[inline]
    fn message(self, f: &mut fmt::Formatter, msg: &impl ToString, level: usize) -> fmt::Result {
        let msg = msg.to_string();
        let lines = msg.lines();
        if self.detailed {
            for line in lines {
                for i in 0..=level {
                    self.edge(f, i)?;
                    f.write_char(' ')?;
                }
                writeln!(f, "{}", line.trim())?;
            }
        } else if let Some(line) = lines.map(str::trim).find(|s| !s.is_empty()) {
            f.write_str(&line)?;
        }
        Ok(())
    }

    #[inline]
    fn mark(self, f: &mut fmt::Formatter, level: usize) -> fmt::Result {
        if self.colored {
            write!(f, "{}", "*".color(to_color(level)))
        } else {
            f.write_char('*')
        }
    }

    #[inline]
    fn edge(self, f: &mut fmt::Formatter, level: usize) -> fmt::Result {
        if self.colored {
            write!(f, "{}", "|".color(to_color(level)))
        } else {
            f.write_char('|')
        }
    }

    #[inline]
    fn split(self, f: &mut fmt::Formatter, level: usize) -> fmt::Result {
        if self.colored {
            write!(
                f,
                "{}{}",
                "|".color(to_color(level)),
                "/".color(to_color(level + 1))
            )
        } else {
            f.write_str("|/")
        }
    }

    #[inline]
    fn position(self, f: &mut fmt::Formatter, at: At, use_branch: bool) -> fmt::Result {
        if self.position {
            if self.colored {
                let position = if use_branch {
                    format!("[{}:{}]", at.branch, at.current)
                } else {
                    format!("[{}]", at.current)
                };
                write!(f, " {}", position.yellow())
            } else if use_branch {
                write!(f, " [{}:{}]", at.branch, at.current)
            } else {
                write!(f, " [{}]", at.current)
            }
        } else {
            Ok(())
        }
    }

    #[inline]
    fn current(self, f: &mut fmt::Formatter, at: At, current: At) -> fmt::Result {
        if self.current && at == current {
            if self.colored {
                write!(f, " {}{}{}", "(".yellow(), "current".cyan(), ")".yellow())
            } else {
                f.write_str(" (current)")
            }
        } else {
            Ok(())
        }
    }

    #[inline]
    fn saved(self, f: &mut fmt::Formatter, at: At, saved: Option<At>) -> fmt::Result {
        if self.saved && saved.map_or(false, |saved| saved == at) {
            if self.colored {
                write!(
                    f,
                    " {}{}{}",
                    "(".yellow(),
                    "saved".bright_green(),
                    ")".yellow()
                )
            } else {
                f.write_str(" (saved)")
            }
        } else {
            Ok(())
        }
    }

    #[inline]
    #[cfg(feature = "chrono")]
    fn timestamp(self, f: &mut fmt::Formatter, timestamp: &DateTime<Utc>) -> fmt::Result {
        let rfc2822 = timestamp.with_timezone(&Local).to_rfc2822();
        if self.colored {
            write!(f, " {}{}{}", "[".yellow(), rfc2822.yellow(), "]".yellow())
        } else {
            write!(f, " [{}]", rfc2822)
        }
    }
}

#[inline]
fn to_color(i: usize) -> Color {
    match i % 6 {
        0 => Color::Cyan,
        1 => Color::Red,
        2 => Color::Magenta,
        3 => Color::Yellow,
        4 => Color::Green,
        5 => Color::Blue,
        _ => unreachable!(),
    }
}
