#[cfg(feature = "chrono")]
use chrono::{DateTime, Local, TimeZone};
use colored::{Color, Colorize};
use std::fmt::{self, Write};
use {At, History, Meta, Record};

/// Configurable display formatting of structures.
#[derive(Copy, Clone, Debug)]
pub struct Display<'a, T: 'a> {
    data: &'a T,
    view: View,
}

impl<'a, T: 'a> Display<'a, T> {
    /// Show colored output (off by default).
    #[inline]
    pub fn colored(&mut self, on: bool) -> &mut Self {
        if on {
            self.view.insert(View::COLORED);
        } else {
            self.view.remove(View::COLORED);
        }
        self
    }

    /// Show detailed output (on by default).
    #[inline]
    pub fn detailed(&mut self, on: bool) -> &mut Self {
        if on {
            self.view.insert(View::DETAILED);
        } else {
            self.view.remove(View::DETAILED);
        }
        self
    }

    /// Show the position of the command (on by default).
    #[inline]
    pub fn position(&mut self, on: bool) -> &mut Self {
        if on {
            self.view.insert(View::POSITION);
        } else {
            self.view.remove(View::POSITION);
        }
        self
    }

    /// Show the saved command (on by default).
    #[inline]
    pub fn saved(&mut self, on: bool) -> &mut Self {
        if on {
            self.view.insert(View::SAVED);
        } else {
            self.view.remove(View::SAVED);
        }
        self
    }
}

impl<'a, R> Display<'a, History<R>> {
    /// Show the history as a graph (off by default).
    #[inline]
    pub fn graph(&mut self, on: bool) -> &mut Self {
        if on {
            self.view.insert(View::GRAPH);
        } else {
            self.view.remove(View::GRAPH);
        }
        self
    }
}

impl<'a, R> Display<'a, Record<R>> {
    #[inline]
    fn fmt_list(&self, f: &mut fmt::Formatter, at: At, meta: &Meta<R>) -> fmt::Result {
        self.view.mark(f)?;
        self.view.position(f, at, false)?;
        if self.view.contains(View::DETAILED) {
            #[cfg(feature = "chrono")]
            self.view.timestamp(f, &meta.timestamp)?;
        }
        self.view.current(
            f,
            at,
            At {
                branch: 0,
                cursor: self.data.cursor(),
            },
        )?;
        self.view.saved(
            f,
            at,
            self.data.saved.map(|saved| At {
                branch: 0,
                cursor: saved,
            }),
        )?;
        if self.view.contains(View::DETAILED) {
            writeln!(f)?;
            self.view.message(f, meta, 0)
        } else {
            f.write_char(' ')?;
            self.view.message(f, meta, 0)?;
            writeln!(f)
        }
    }
}

impl<'a, R> Display<'a, History<R>> {
    #[inline]
    fn fmt_list(
        &self,
        f: &mut fmt::Formatter,
        at: At,
        meta: &Meta<R>,
        level: usize,
    ) -> fmt::Result {
        self.view.mark(f)?;
        self.view.position(f, at, true)?;
        if self.view.contains(View::DETAILED) {
            #[cfg(feature = "chrono")]
            self.view.timestamp(f, &meta.timestamp)?;
        }
        self.view.current(
            f,
            at,
            At {
                branch: self.data.root(),
                cursor: self.data.cursor(),
            },
        )?;
        self.view.saved(
            f,
            at,
            self.data
                .record
                .saved
                .map(|saved| At {
                    branch: self.data.root(),
                    cursor: saved,
                })
                .or(self.data.saved),
        )?;
        if self.view.contains(View::DETAILED) {
            writeln!(f)?;
            self.view.message(f, meta, level)
        } else {
            f.write_char(' ')?;
            self.view.message(f, meta, level)?;
            writeln!(f)
        }
    }

    #[inline]
    fn fmt_graph(
        &self,
        f: &mut fmt::Formatter,
        at: At,
        meta: &Meta<R>,
        level: usize,
    ) -> fmt::Result {
        for (&i, branch) in self
            .data
            .branches
            .iter()
            .filter(|(_, branch)| branch.parent == at)
        {
            for (j, cmd) in branch.commands.iter().enumerate().rev() {
                let at = At {
                    branch: i,
                    cursor: j + branch.parent.cursor + 1,
                };
                self.fmt_graph(f, at, cmd, level + 1)?;
            }
            for j in 0..level {
                self.view.edge(f, j)?;
                f.write_char(' ')?;
            }
            self.view.split(f, level)?;
            writeln!(f)?;
        }
        for i in 0..level {
            self.view.edge(f, i)?;
            f.write_char(' ')?;
        }
        self.fmt_list(f, at, meta, level)
    }
}

impl<'a, T: 'a> From<&'a T> for Display<'a, T> {
    #[inline]
    fn from(data: &'a T) -> Self {
        Display {
            data,
            view: View::default(),
        }
    }
}

impl<'a, R> fmt::Display for Display<'a, Record<R>> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, cmd) in self.data.commands.iter().enumerate().rev() {
            let at = At {
                branch: 0,
                cursor: i + 1,
            };
            self.fmt_list(f, at, cmd)?;
        }
        Ok(())
    }
}

impl<'a, R> fmt::Display for Display<'a, History<R>> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, cmd) in self.data.record.commands.iter().enumerate().rev() {
            let at = At {
                branch: self.data.root(),
                cursor: i + 1,
            };
            if self.view.contains(View::GRAPH) {
                self.fmt_graph(f, at, cmd, 0)?;
            } else {
                self.fmt_list(f, at, cmd, 0)?;
            }
        }
        Ok(())
    }
}

bitflags! {
    struct View: u8 {
        const COLORED   = 0b_0000_0001;
        const CURRENT   = 0b_0000_0010;
        const DETAILED  = 0b_0000_0100;
        const GRAPH     = 0b_0000_1000;
        const LIGATURES = 0b_0001_0000;
        const MARK      = 0b_0010_0000;
        const POSITION  = 0b_0100_0000;
        const SAVED     = 0b_1000_0000;
    }
}

impl Default for View {
    #[inline]
    fn default() -> Self {
        View::CURRENT | View::DETAILED | View::MARK | View::POSITION | View::SAVED
    }
}

impl View {
    #[inline]
    fn message(self, f: &mut fmt::Formatter, msg: &impl ToString, level: usize) -> fmt::Result {
        let msg = msg.to_string();
        let lines = msg.lines();
        if self.contains(View::DETAILED) {
            for line in lines {
                for i in 0..level + 1 {
                    self.edge(f, i)?;
                    f.write_char(' ')?;
                }
                writeln!(f, "{}", line.trim())?;
            }
        } else if let Some(line) = lines.map(|s| s.trim()).find(|s| !s.is_empty()) {
            f.write_str(&line)?;
        }
        Ok(())
    }

    #[inline]
    fn mark(self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.contains(View::MARK) {
            f.write_str("*")
        } else {
            Ok(())
        }
    }

    #[inline]
    fn edge(self, f: &mut fmt::Formatter, level: usize) -> fmt::Result {
        if self.contains(View::COLORED) {
            write!(f, "{}", "|".color(color(level)))
        } else {
            f.write_str("|")
        }
    }

    #[inline]
    fn split(self, f: &mut fmt::Formatter, level: usize) -> fmt::Result {
        if self.contains(View::COLORED) {
            write!(
                f,
                "{}{}",
                "|".color(color(level)),
                "/".color(color(level + 1))
            )
        } else {
            f.write_str("|/")
        }
    }

    #[inline]
    fn position(self, f: &mut fmt::Formatter, at: At, use_branch: bool) -> fmt::Result {
        if self.contains(View::POSITION) {
            if self.contains(View::COLORED) {
                let position = if use_branch {
                    format!("[{}:{}]", at.branch, at.cursor)
                } else {
                    format!("[{}]", at.cursor)
                };
                write!(f, " {}", position.yellow())
            } else if use_branch {
                write!(f, " [{}:{}]", at.branch, at.cursor)
            } else {
                write!(f, " [{}]", at.cursor)
            }
        } else {
            Ok(())
        }
    }

    #[inline]
    fn current(self, f: &mut fmt::Formatter, at: At, current: At) -> fmt::Result {
        if self.contains(View::CURRENT) && at == current {
            if self.contains(View::COLORED) {
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
        if self.contains(View::SAVED) && saved.map_or(false, |saved| saved == at) {
            if self.contains(View::COLORED) {
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
    fn timestamp(self, f: &mut fmt::Formatter, timestamp: &DateTime<impl TimeZone>) -> fmt::Result {
        let local = Local.from_utc_datetime(&timestamp.naive_utc());
        if self.contains(View::COLORED) {
            let ts = format!("[{}]", local.format("%T %F"));
            write!(f, " {}", ts.yellow())
        } else {
            write!(f, " [{}]", local.format("%T %F"))
        }
    }
}

#[inline]
fn color(i: usize) -> Color {
    match i % 6 {
        0 => Color::Red,
        1 => Color::Blue,
        2 => Color::Magenta,
        3 => Color::Yellow,
        4 => Color::Green,
        5 => Color::Cyan,
        _ => unreachable!(),
    }
}
