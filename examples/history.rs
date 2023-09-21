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
            "Enter a string. Use '<' to undo, '>' to redo, '*' to save, and '! i-j' for goto: "
        );
        let mut string = String::new();
        let n = stdin.read_line(&mut string)?;
        if n == 0 {
            return Ok(());
        }

        // Clears the terminal.
        print!("{}c", 27 as char);

        let mut chars = string.trim().chars();
        while let Some(c) = chars.next() {
            if c == '!' {
                let rest = chars.collect::<String>();
                let mut at = rest
                    .trim()
                    .split('-')
                    .filter_map(|n| n.parse::<usize>().ok());

                if let (Some(root), Some(index)) = (at.next(), at.next()) {
                    history.go_to(&mut target, At::new(root, index));
                } else {
                    println!("Expected input as '! i-j', e.g. '! 1-5'.");
                }
                break;
            } else if c == '<' {
                history.undo(&mut target);
            } else if c == '>' {
                history.redo(&mut target);
            } else if c == '*' {
                history.set_saved(true);
            } else {
                history.edit(&mut target, Add(c));
            }
        }

        println!("{}\n", history.display().set_st_fmt(&custom_st_fmt));
        println!("Target: {target}");
    }
}
