use history::At;
use std::fmt::{self, Write};
use History;

#[derive(Debug)]
pub struct Display<'a, T: 'a> {
    data: &'a T,
    view: View,
}

impl<'a, R> Display<'a, History<R>> {
    #[inline]
    fn list(
        &self,
        f: &mut fmt::Formatter,
        at: At,
        cmd: &impl fmt::Display,
        level: usize,
    ) -> fmt::Result {
        let mark = self.view.mark();
        let position = self.view.position(at);
        let current = self.view.current(
            at,
            At {
                branch: self.data.root(),
                cursor: self.data.cursor(),
            },
        );
        let saved = self.view.saved(
            at,
            self.data
                .record
                .saved
                .map(|saved| At {
                    branch: self.data.root(),
                    cursor: saved,
                }).or(self.data.saved),
        );
        write!(f, "{}{}{}{}", mark, position, current, saved)?;
        let msg = self.view.message(&cmd, level)?;
        if self.view.contains(View::DETAILED) {
            write!(f, "\n{}", msg)
        } else {
            writeln!(f, " {}", msg)
        }
    }

    #[inline]
    fn graph(
        &self,
        f: &mut fmt::Formatter,
        at: At,
        cmd: &impl fmt::Display,
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
                self.graph(f, at, cmd, level + 1)?;
            }
            for _ in 0..level {
                write!(f, "{} ", self.view.edge())?;
            }
            writeln!(f, "{}", self.view.split())?;
        }
        for _ in 0..level {
            write!(f, "{} ", self.view.edge())?;
        }
        self.list(f, at, cmd, level)
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

impl<'a, R> fmt::Display for Display<'a, History<R>> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, cmd) in self.data.record.commands.iter().enumerate().rev() {
            let at = At {
                branch: self.data.root(),
                cursor: i + 1,
            };
            if self.view.contains(View::GRAPH) {
                self.graph(f, at, cmd, 0)?;
            } else {
                self.list(f, at, cmd, 0)?;
            }
        }
        Ok(())
    }
}

bitflags! {
    struct View: u8 {
        const COLORS    = 0b00000001;
        const CURRENT   = 0b00000010;
        const DETAILED  = 0b00000100;
        const GRAPH     = 0b00001000;
        const LIGATURES = 0b00010000;
        const MARK      = 0b00100000;
        const POSITION  = 0b01000000;
        const SAVED     = 0b10000000;
    }
}

impl Default for View {
    #[inline]
    fn default() -> Self {
        View::all()
    }
}

impl View {
    #[inline]
    fn message(&self, cmd: &impl ToString, level: usize) -> Result<String, fmt::Error> {
        let cmd = cmd.to_string();
        let lines = cmd.lines();
        if self.contains(View::DETAILED) {
            let mut msg = String::new();
            for line in lines {
                for _ in 0..level + 1 {
                    write!(msg, "{} ", self.edge())?;
                }
                writeln!(msg, "{}", line.trim())?;
            }
            Ok(msg)
        } else {
            Ok(lines
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .next()
                .map_or_else(String::new, |s| s.to_string()))
        }
    }

    #[inline]
    fn mark(&self) -> &str {
        if self.contains(View::MARK) {
            "*"
        } else {
            ""
        }
    }

    #[inline]
    fn edge(&self) -> &str {
        "|"
    }

    #[inline]
    fn split(&self) -> &str {
        "|/"
    }

    #[inline]
    fn position(&self, at: At) -> String {
        if self.contains(View::POSITION) {
            format!(" [{}:{}]", at.branch, at.cursor)
        } else {
            String::new()
        }
    }

    #[inline]
    fn current(&self, at: At, current: At) -> &str {
        if self.contains(View::CURRENT) && at == current {
            " (current)"
        } else {
            ""
        }
    }

    #[inline]
    fn saved(&self, at: At, saved: Option<At>) -> &str {
        if self.contains(View::SAVED) && saved.map_or(false, |saved| saved == at) {
            " (saved)"
        } else {
            ""
        }
    }
}
