#![cfg_attr(not(feature = "colored"), allow(unused_variables))]

use crate::At;
use alloc::string::ToString;
#[cfg(feature = "colored")]
use colored::{Color, Colorize};
use core::fmt::{self, Write};
#[cfg(feature = "std")]
use std::time::SystemTime;

#[cfg(feature = "std")]
pub(crate) fn default_st_fmt(now: SystemTime, at: SystemTime) -> String {
    let elapsed = now.duration_since(at).unwrap_or_else(|e| e.duration());
    format!("{elapsed:.1?}")
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct Format {
    #[cfg(feature = "colored")]
    pub colored: bool,
    pub detailed: bool,
    pub head: bool,
    pub saved: bool,
}

impl Default for Format {
    fn default() -> Self {
        Format {
            #[cfg(feature = "colored")]
            colored: true,
            detailed: true,
            head: true,
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
        self.text(f, "* ", level)
    }

    pub fn edge(self, f: &mut fmt::Formatter, level: usize) -> fmt::Result {
        self.text(f, "|", level)
    }

    pub fn split(self, f: &mut fmt::Formatter, level: usize) -> fmt::Result {
        self.text(f, "|", level)?;
        self.text(f, "/", level + 1)
    }

    pub fn position(self, f: &mut fmt::Formatter, at: At, use_branch: bool) -> fmt::Result {
        #[cfg(feature = "colored")]
        if self.colored {
            let position = if use_branch {
                alloc::format!("{}:{}", at.root, at.index)
            } else {
                alloc::format!("{}", at.index)
            };
            return write!(f, "{}", position.yellow());
        }
        if use_branch {
            write!(f, "{}:{}", at.root, at.index)
        } else {
            write!(f, "{}", at.index)
        }
    }

    pub fn labels(
        self,
        f: &mut fmt::Formatter,
        at: At,
        head: At,
        saved: Option<At>,
    ) -> fmt::Result {
        match (
            self.head && at == head,
            self.saved && matches!(saved, Some(saved) if saved == at),
        ) {
            (true, true) => {
                #[cfg(feature = "colored")]
                if self.colored {
                    return write!(
                        f,
                        " {}{}{} {}{}",
                        "[".yellow(),
                        "HEAD".cyan(),
                        ",".yellow(),
                        "SAVED".green(),
                        "]".yellow()
                    );
                }
                f.write_str(" [HEAD, SAVED]")
            }
            (true, false) => {
                #[cfg(feature = "colored")]
                if self.colored {
                    return write!(f, " {}{}{}", "[".yellow(), "HEAD".cyan(), "]".yellow());
                }
                f.write_str(" [HEAD]")
            }
            (false, true) => {
                #[cfg(feature = "colored")]
                if self.colored {
                    return write!(f, " {}{}{}", "[".yellow(), "SAVED".green(), "]".yellow());
                }
                f.write_str(" [SAVED]")
            }
            (false, false) => Ok(()),
        }
    }

    #[cfg(feature = "std")]
    pub fn elapsed(self, f: &mut fmt::Formatter, string: String) -> fmt::Result {
        #[cfg(feature = "colored")]
        if self.colored {
            return write!(f, " {}", string.yellow());
        }
        write!(f, " {string}")
    }

    pub fn text(self, f: &mut fmt::Formatter, text: &str, level: usize) -> fmt::Result {
        #[cfg(feature = "colored")]
        if self.colored {
            return write!(f, "{}", text.color(color_of_level(level)));
        }
        f.write_str(text)
    }
}

#[cfg(feature = "colored")]
fn color_of_level(level: usize) -> Color {
    match level % 6 {
        0 => Color::Cyan,
        1 => Color::Red,
        2 => Color::Magenta,
        3 => Color::Yellow,
        4 => Color::Green,
        5 => Color::Blue,
        _ => unreachable!(),
    }
}
