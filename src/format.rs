#![cfg_attr(not(feature = "colored"), allow(unused_variables))]

use crate::At;
use alloc::string::ToString;
#[cfg(feature = "chrono")]
use chrono::{DateTime, Local, Utc};
use core::fmt::{self, Write};
#[cfg(feature = "colored")]
use {
    alloc::format,
    colored::{Color, Colorize},
};

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
            f.write_str(&line)?;
        }
        Ok(())
    }

    pub fn mark(self, f: &mut fmt::Formatter, level: usize) -> fmt::Result {
        #[cfg(feature = "colored")]
        if self.colored {
            write!(f, "{} ", "*".color(color_of_level(level)))
        } else {
            f.write_str("* ")
        }
        #[cfg(not(feature = "colored"))]
        f.write_str("* ")
    }

    pub fn edge(self, f: &mut fmt::Formatter, level: usize) -> fmt::Result {
        #[cfg(feature = "colored")]
        if self.colored {
            write!(f, "{}", "|".color(color_of_level(level)))
        } else {
            f.write_char('|')
        }
        #[cfg(not(feature = "colored"))]
        f.write_char('|')
    }

    pub fn split(self, f: &mut fmt::Formatter, level: usize) -> fmt::Result {
        #[cfg(feature = "colored")]
        if self.colored {
            write!(
                f,
                "{}{}",
                "|".color(color_of_level(level)),
                "/".color(color_of_level(level + 1))
            )
        } else {
            f.write_str("|/")
        }
        #[cfg(not(feature = "colored"))]
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
                write!(f, "{}", position.yellow().bold())
            } else if use_branch {
                write!(f, "{}:{}", at.branch, at.current)
            } else {
                write!(f, "{}", at.current)
            }
            #[cfg(not(feature = "colored"))]
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
                    write!(
                        f,
                        " {}{}{} {}{}",
                        "(".yellow(),
                        "current".cyan().bold(),
                        ",".yellow(),
                        "saved".green().bold(),
                        ")".yellow()
                    )
                } else {
                    f.write_str(" (current, saved)")
                }
                #[cfg(not(feature = "colored"))]
                f.write_str(" (current)")
            }
            (true, false) => {
                #[cfg(feature = "colored")]
                if self.colored {
                    write!(
                        f,
                        " {}{}{}",
                        "(".yellow(),
                        "current".cyan().bold(),
                        ")".yellow()
                    )
                } else {
                    f.write_str(" (current)")
                }
                #[cfg(not(feature = "colored"))]
                f.write_str(" (current)")
            }
            (false, true) => {
                #[cfg(feature = "colored")]
                if self.colored {
                    write!(
                        f,
                        " {}{}{}",
                        "(".yellow(),
                        "saved".green().bold(),
                        ")".yellow()
                    )
                } else {
                    f.write_str(" (saved)")
                }
                #[cfg(not(feature = "colored"))]
                f.write_str(" (saved)")
            }
            (false, false) => Ok(()),
        }
    }

    #[cfg(feature = "chrono")]
    pub fn timestamp(self, f: &mut fmt::Formatter, timestamp: &DateTime<Utc>) -> fmt::Result {
        let rfc2822 = timestamp.with_timezone(&Local).to_rfc2822();
        #[cfg(feature = "colored")]
        if self.colored {
            write!(f, " {}", rfc2822.yellow())
        } else {
            write!(f, " {}", rfc2822)
        }
        #[cfg(not(feature = "colored"))]
        write!(f, " {}", rfc2822)
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
