use std::io::{self, Write};
use std::process::Command;

pub fn prompt(description: &str) -> String {
    print!("{description}");

    let mut s = String::new();

    let _ = io::stdout().flush();
    io::stdin()
        .read_line(&mut s)
        .expect("Failed to read line");

    s.trim().to_string()
}

pub fn terminal(description: &str) {
    let output = Command::new("sh")
        .arg("-c")
        .arg(description)
        .output()
        .expect("Failed to execute");

    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("{}", stdout);
}
