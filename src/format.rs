#![cfg_attr(not(feature = "colored"), allow(unused_variables))]

use crate::At;
#[cfg(feature = "colored")]
use colored::{Color, Colorize};
use std::fmt::{self, Write};
use std::time::SystemTime;

#[derive(Copy, Clone, Debug)]
pub(crate) struct Format {
    #[cfg(feature = "colored")]
    pub colored: bool,
    pub current: bool,
    pub detailed: bool,
    pub position: bool,
    pub saved: bool,
}

impl Default for Format {
    fn default() -> Self {
        Format {
            #[cfg(feature = "colored")]
            colored: true,
            current: true,
            detailed: true,
            position: true,
            saved: true,
        }
    }
}

impl Format {
    pub fn message(
        self,
        f: &mut fmt::Formatter,
        msg: &impl ToString,
        level: Option<usize>,
    ) -> fmt::Result {
        let msg = msg.to_string();
        let lines = msg.lines();
        if self.detailed {
            for line in lines {
                if let Some(level) = level {
                    for i in 0..=level {
                        self.edge(f, i)?;
                        f.write_char(' ')?;
                    }
                }
                writeln!(f, "{}", line.trim())?;
            }
        } else if let Some(line) = lines.map(str::trim).find(|s| !s.is_empty()) {
            f.write_str(line)?;
        }
        Ok(())
    }

    pub fn mark(self, f: &mut fmt::Formatter, level: usize) -> fmt::Result {
        #[cfg(feature = "colored")]
        if self.colored {
            return write!(f, "{} ", "*".color(color_of_level(level)));
        }
        f.write_str("* ")
    }

    pub fn edge(self, f: &mut fmt::Formatter, level: usize) -> fmt::Result {
        #[cfg(feature = "colored")]
        if self.colored {
            return write!(f, "{}", "|".color(color_of_level(level)));
        }
        f.write_char('|')
    }

    pub fn split(self, f: &mut fmt::Formatter, level: usize) -> fmt::Result {
        #[cfg(feature = "colored")]
        if self.colored {
            return write!(
                f,
                "{}{}",
                "|".color(color_of_level(level)),
                "/".color(color_of_level(level + 1))
            );
        }
        f.write_str("|/")
    }

    pub fn position(self, f: &mut fmt::Formatter, at: At, use_branch: bool) -> fmt::Result {
        if self.position {
            #[cfg(feature = "colored")]
            if self.colored {
                let position = if use_branch {
                    format!("{}:{}", at.branch, at.current)
                } else {
                    format!("{}", at.current)
                };
                return write!(f, "{}", position.yellow().bold());
            }
            if use_branch {
                write!(f, "{}:{}", at.branch, at.current)
            } else {
                write!(f, "{}", at.current)
            }
        } else {
            Ok(())
        }
    }

    pub fn labels(
        self,
        f: &mut fmt::Formatter,
        at: At,
        current: At,
        saved: Option<At>,
    ) -> fmt::Result {
        match (
            self.current && at == current,
            self.saved && saved.map_or(false, |saved| saved == at),
        ) {
            (true, true) => {
                #[cfg(feature = "colored")]
                if self.colored {
                    return write!(
                        f,
                        " {}{}{} {}{}",
                        "(".yellow(),
                        "current".cyan().bold(),
                        ",".yellow(),
                        "saved".green().bold(),
                        ")".yellow()
                    );
                }
                f.write_str(" (current, saved)")
            }
            (true, false) => {
                #[cfg(feature = "colored")]
                if self.colored {
                    return write!(
                        f,
                        " {}{}{}",
                        "(".yellow(),
                        "current".cyan().bold(),
                        ")".yellow()
                    );
                }
                f.write_str(" (current)")
            }
            (false, true) => {
                #[cfg(feature = "colored")]
                if self.colored {
                    return write!(
                        f,
                        " {}{}{}",
                        "(".yellow(),
                        "saved".green().bold(),
                        ")".yellow()
                    );
                }
                f.write_str(" (saved)")
            }
            (false, false) => Ok(()),
        }
    }

    pub fn elapsed(
        self,
        f: &mut fmt::Formatter,
        now: SystemTime,
        earlier: SystemTime,
    ) -> fmt::Result {
        let elapsed = now.duration_since(earlier).unwrap_or_else(|e| e.duration());
        let string = format!("{elapsed:.1?}");
        #[cfg(feature = "colored")]
        if self.colored {
            return write!(f, " {}", string.yellow());
        }
        write!(f, " {string}")
    }
}

#[cfg(feature = "colored")]
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
