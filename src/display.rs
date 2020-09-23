use crate::{Command, Entry, Record};
use chrono::{DateTime, Local, Utc};
use colored::Colorize;
use std::fmt::{self, Write};

/// Configurable display formatting of a record.
///
/// # Examples
/// ```no_run
/// # use undo::{Command, Record};
/// # fn foo() -> Record<String> {
/// let record = Record::default();
/// println!("{}", record.display().colored(false).detailed(false));
/// # record
/// # }
/// ```
#[derive(Copy, Clone, Debug)]
pub struct Display<'a, T: 'static> {
    record: &'a Record<T>,
    format: Format,
}

impl<T> Display<'_, T> {
    /// Show colored output (on by default).
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

    /// Show the position of the command (on by default).
    pub fn position(&mut self, on: bool) -> &mut Self {
        self.format.position = on;
        self
    }

    /// Show the saved command (on by default).
    pub fn saved(&mut self, on: bool) -> &mut Self {
        self.format.saved = on;
        self
    }
}

impl<T> Display<'_, T> {
    fn fmt_list(&self, f: &mut fmt::Formatter, at: usize, entry: Option<&Entry<T>>) -> fmt::Result {
        self.format.position(f, at)?;
        if let Some(entry) = entry {
            if self.format.detailed {
                self.format.timestamp(f, &entry.timestamp)?;
            }
        }
        self.format
            .labels(f, at, self.record.current(), self.record.saved)?;
        if let Some(entry) = entry {
            if self.format.detailed {
                writeln!(f)?;
                self.format.message(f, entry.text())?;
            } else {
                f.write_char(' ')?;
                self.format.message(f, entry.text())?;
                writeln!(f)?;
            }
        }
        Ok(())
    }
}

impl<'a, T> From<&'a Record<T>> for Display<'a, T> {
    fn from(record: &'a Record<T>) -> Self {
        Display {
            record,
            format: Format::default(),
        }
    }
}

impl<T> fmt::Display for Display<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, entry) in self.record.entries.iter().enumerate().rev() {
            self.fmt_list(f, i + 1, Some(entry))?;
        }
        self.fmt_list(f, 0, None)
    }
}

#[derive(Copy, Clone, Debug)]
struct Format {
    colored: bool,
    current: bool,
    detailed: bool,
    position: bool,
    saved: bool,
}

impl Default for Format {
    fn default() -> Self {
        Format {
            colored: true,
            current: true,
            detailed: true,
            position: true,
            saved: true,
        }
    }
}

impl Format {
    fn message(self, f: &mut fmt::Formatter, msg: String) -> fmt::Result {
        let lines = msg.lines();
        if self.detailed {
            for line in lines {
                writeln!(f, "{}", line.trim())?;
            }
        } else if let Some(line) = lines.map(str::trim).find(|s| !s.is_empty()) {
            f.write_str(&line)?;
        }
        Ok(())
    }

    fn position(self, f: &mut fmt::Formatter, at: usize) -> fmt::Result {
        if self.position {
            if self.colored {
                write!(f, "{}", format!("{}", at).yellow().bold())
            } else {
                write!(f, "{}", at)
            }
        } else {
            Ok(())
        }
    }

    fn labels(
        self,
        f: &mut fmt::Formatter,
        at: usize,
        current: usize,
        saved: Option<usize>,
    ) -> fmt::Result {
        match (
            self.current && at == current,
            self.saved && saved.map_or(false, |saved| saved == at),
            self.colored,
        ) {
            (true, true, true) => write!(
                f,
                " {}{}{} {}{}",
                "(".yellow(),
                "current".cyan().bold(),
                ",".yellow(),
                "saved".green().bold(),
                ")".yellow()
            ),
            (true, true, false) => f.write_str(" (current, saved)"),
            (true, false, true) => write!(
                f,
                " {}{}{}",
                "(".yellow(),
                "current".cyan().bold(),
                ")".yellow()
            ),
            (true, false, false) => f.write_str(" (current)"),
            (false, true, true) => write!(
                f,
                " {}{}{}",
                "(".yellow(),
                "saved".green().bold(),
                ")".yellow()
            ),
            (false, true, false) => f.write_str(" (saved)"),
            (false, false, _) => Ok(()),
        }
    }

    fn timestamp(self, f: &mut fmt::Formatter, timestamp: &DateTime<Utc>) -> fmt::Result {
        let rfc2822 = timestamp.with_timezone(&Local).to_rfc2822();
        if self.colored {
            write!(f, " {}{}{}", "".yellow(), rfc2822.yellow(), "".yellow())
        } else {
            write!(f, " [{}]", rfc2822)
        }
    }
}
