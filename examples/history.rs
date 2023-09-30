use chrono::{DateTime, Local};
use std::io;
use std::time::SystemTime;
use undo::{Add, At, History};

fn custom_st_fmt(_: SystemTime, at: SystemTime) -> String {
    let dt = DateTime::<Local>::from(at);
    dt.format("%H:%M:%S").to_string()
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut target = String::new();
    let mut history = History::<_>::builder().limit(10).capacity(10).build();

    loop {
        println!(
            "Enter a string. Use '<' to undo, '>' to redo, '*' to save, '+' to switch to next branch, '-' to switch to previous branch, and '! i-j' for goto: "
        );
        let mut buf = String::new();
        let n = stdin.read_line(&mut buf)?;
        if n == 0 {
            return Ok(());
        }

        // Clears the terminal.
        print!("{}c", 27 as char);

        let mut chars = buf.trim().chars();
        while let Some(c) = chars.next() {
            match c {
                '!' => {
                    let tail = chars.collect::<String>();
                    let mut at = tail
                        .trim()
                        .split('-')
                        .filter_map(|n| n.parse::<usize>().ok());

                    let root = at.next().unwrap_or_default();
                    let index = at.next().unwrap_or_default();
                    history.go_to(&mut target, At::new(root, index));
                    break;
                }
                '<' => {
                    history.undo(&mut target);
                }
                '>' => {
                    history.redo(&mut target);
                }
                '*' => {
                    history.set_saved(true);
                }
                '+' => {
                    if let Some(at) = history.next_branch_head() {
                        history.go_to(&mut target, at);
                    }
                }
                '-' => {
                    if let Some(at) = history.prev_branch_head() {
                        history.go_to(&mut target, at);
                    }
                }
                _ => {
                    history.edit(&mut target, Add(c));
                }
            }
        }

        println!("{}\n", history.display().set_st_fmt(&custom_st_fmt));
        println!("Target: {target}");
    }
}
