use crate::{Command, Entry, Record};
use chrono::{DateTime, Local, Utc};
use colored::{Color, Colorize};
use std::fmt::{self, Write};

/// Configurable display formatting of structures.
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
    config: Config,
}

impl<T> Display<'_, T> {
    /// Show colored output (on by default).
    pub fn colored(&mut self, on: bool) -> &mut Self {
        self.config.colored = on;
        self
    }

    /// Show the current position in the output (on by default).
    pub fn current(&mut self, on: bool) -> &mut Self {
        self.config.current = on;
        self
    }

    /// Show detailed output (on by default).
    pub fn detailed(&mut self, on: bool) -> &mut Self {
        self.config.detailed = on;
        self
    }

    /// Show the position of the command (on by default).
    pub fn position(&mut self, on: bool) -> &mut Self {
        self.config.position = on;
        self
    }

    /// Show the saved command (on by default).
    pub fn saved(&mut self, on: bool) -> &mut Self {
        self.config.saved = on;
        self
    }
}

impl<T> Display<'_, T> {
    fn fmt_list(&self, f: &mut fmt::Formatter, at: usize, entry: &Entry<T>) -> fmt::Result {
        self.config.mark(f, 0)?;
        self.config.position(f, at)?;
        if self.config.detailed {
            self.config.timestamp(f, &entry.timestamp)?;
        }
        self.config.current(f, at, self.record.current())?;
        self.config.saved(f, at, self.record.saved)?;
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

impl<'a, T> From<&'a Record<T>> for Display<'a, T> {
    fn from(data: &'a Record<T>) -> Self {
        Display {
            record: data,
            config: Config::default(),
        }
    }
}

impl<T> fmt::Display for Display<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, entry) in self.record.entries.iter().enumerate().rev() {
            self.fmt_list(f, i + 1, entry)?;
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
    fn default() -> Self {
        Config {
            colored: true,
            current: true,
            detailed: true,
            position: true,
            saved: true,
        }
    }
}

impl Config {
    fn message<T>(
        self,
        f: &mut fmt::Formatter,
        command: &impl Command<T>,
        level: usize,
    ) -> fmt::Result {
        let msg = command.text();
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

    fn mark(self, f: &mut fmt::Formatter, level: usize) -> fmt::Result {
        if self.colored {
            write!(f, "{}", "*".color(color_of_level(level)))
        } else {
            f.write_char('*')
        }
    }

    fn edge(self, f: &mut fmt::Formatter, level: usize) -> fmt::Result {
        if self.colored {
            write!(f, "{}", "|".color(color_of_level(level)))
        } else {
            f.write_char('|')
        }
    }

    fn position(self, f: &mut fmt::Formatter, at: usize) -> fmt::Result {
        if self.position {
            if self.colored {
                write!(f, " {}", format!("[{}]", at).yellow())
            } else {
                write!(f, " [{}]", at)
            }
        } else {
            Ok(())
        }
    }

    fn current(self, f: &mut fmt::Formatter, at: usize, current: usize) -> fmt::Result {
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

    fn saved(self, f: &mut fmt::Formatter, at: usize, saved: Option<usize>) -> fmt::Result {
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

    fn timestamp(self, f: &mut fmt::Formatter, timestamp: &DateTime<Utc>) -> fmt::Result {
        let rfc2822 = timestamp.with_timezone(&Local).to_rfc2822();
        if self.colored {
            write!(f, " {}{}{}", "[".yellow(), rfc2822.yellow(), "]".yellow())
        } else {
            write!(f, " [{}]", rfc2822)
        }
    }
}

fn color_of_level(i: usize) -> Color {
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
