#![allow(dead_code)]
use nix::libc;
use regex::Regex;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::process::{Command, Output, Stdio};

pub fn prompt(description: &str) -> String {
    print!("{description}");

    io::stdout().flush().expect("Failed to flush stdout");

    let mut s = String::new();
    io::stdin().read_line(&mut s).expect("Failed to read line");

    s.trim().to_string()
}

pub fn prompt_u8(description: &str) -> Vec<u8> {
    print!("{description}");

    io::stdout().flush().expect("Failed to flush stdout");

    let mut buffer = Vec::new();
    let mut reader = BufReader::new(io::stdin());

    reader.read_until(b'\n', &mut buffer).expect("Failed to read line");

    if let Some(&b'\n') = buffer.last() {
        buffer.pop(); // Remove newline
        if buffer.last() == Some(&b'\r') {
            buffer.pop(); // Remove carriage return
        }
    }

    buffer
}

pub fn run_shell_command(command: &str) -> std::io::Result<Output> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(Stdio::piped()) // piped to not override the output.
        .stderr(Stdio::inherit())
        .output()?;

        if output.status.success() {
            println!("{}", String::from_utf8_lossy(&output.stdout));
            Ok(output)
        } else {
            eprintln!(
                "Error executing {}: {}",
                command,
                String::from_utf8_lossy(&output.stderr)
            );
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Shell command execution failed",
            ))
        }
}

pub fn run_command(command: &str, args: &[&str]) -> std::io::Result<Output> {
    let output = Command::new(command)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()?;

    if output.status.success() {
        println!("{}", String::from_utf8_lossy(&output.stdout));
        Ok(output)
    } else {
        eprintln!(
            "Error executing {}: {}",
            command,
            String::from_utf8_lossy(&output.stderr)
        );
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Command execution failed",
        ))
    }
}

pub fn umount(target: &str, flags: libc::c_int) -> Result<(), String> {
    let target_c = std::ffi::CString::new(target).map_err(|_| "Failed to create CString")?;
    let action = unsafe { libc::umount2(target_c.as_ptr(), flags) };

    if action == 0 {
        Ok(())
    } else {
        Err(format!("Failed to unmount: {}", std::io::Error::last_os_error()))
    }
}

pub fn archiso_check() -> io::Result<()> {
    fn exit_message() {
        eprintln!("\nDo not run the Arch Flux installer outside of the Arch Linux ISO!\n");
    }

    let file = match File::open("/etc/mkinitcpio.d/linux.preset") {
        Ok(file) => file,
        Err(_) => {
            exit_message();
            std::process::exit(1);
        }
    };

    let mut reader = BufReader::new(file);
    let mut contents = String::new();

    reader.read_to_string(&mut contents)?;

    if !contents.contains("archiso") {
        exit_message();
        std::process::exit(1);
    }

    Ok(())
}

pub fn fetch_disk() -> io::Result<String> {
    let file = File::open("/root/selected_disk.cfg")?;
    let mut reader = BufReader::new(file);
    let mut contents = String::new();

    reader.read_to_string(&mut contents)?;

    if contents.is_empty() {
        eprintln!("Disk not found in /root/selected_disk.cfg, did you run the disk format utility, or forgot to input the disk manually?");
        std::process::exit(1);
    } else {
        let input = contents.replace("\n", ""); // Incase someone uses Vim to manually input the disk.

        let ssd = Regex::new(r"/dev/sd[a-z]").unwrap().find(&input);
        let nvme = Regex::new(r"/dev/(nvme|mmc)([0-9])n1").unwrap().find(&input);

        let input = if let Some(ssd) = ssd {
            ssd.as_str().to_string()
        } else if let Some(nvme) = nvme {
            nvme.as_str().to_string()
        } else {
            eprintln!("Invalid disk format");
            std::process::exit(1);
        };
        Ok(input)
    }
}

pub fn create_sub_volumes(subvol_list: &[String]) -> io::Result<()> {
    for subvol in subvol_list {
        let path = format!("/mnt/@{}", subvol);
        if let Err(err) = run_command("btrfs", &["subvolume", "create", &path]) {
            eprintln!("Failed to create subvolume {}: {}", subvol, err);
        } else {
            println!("Successfully created subvolume: {}", subvol);
        }
    }
    Ok(())
}

pub fn config_write(value: &str, line: &str, file_path: &str) -> io::Result<()> {
    let file_content = fs::read_to_string(file_path)?;

    let formatted_entry = format!("{}{}", line, value).trim().to_string();

    let mut lines: Vec<String> = file_content.lines().map(|s| s.to_string()).collect();

    if let Some(index) = lines.iter().position(|entry| entry.starts_with(line)) {
        lines[index] = formatted_entry;
    } else if !formatted_entry.is_empty() {
        lines.push(formatted_entry);
    }

    let new_contents = lines.join("\n");
    let final_contents = if new_contents.is_empty() {
        new_contents
    } else {
        new_contents + "\n"
    };

    fs::write(file_path, final_contents)?;

    Ok(())
}

pub fn touch_file(path: &str) -> io::Result<()> {
    match OpenOptions::new().create(true).write(true).open(path) {
        Ok(_) => Ok(()),
        Err(err) => Err(err),
    }
}

pub fn find_option(option: &str) -> Result<String, Box<dyn std::error::Error>> {
    let file_path = "/root/user_selections.cfg";
    let file_contents = std::fs::read_to_string(file_path)?;
    let re = regex::Regex::new(&format!(r"{}=(\w+)", option))?;
    let layout = re
        .captures(&file_contents)
        .ok_or(format!("Failed to find {}", option))?
        .get(1)
        .ok_or(format!("Failed to extract {}", option))?
        .as_str()
        .to_string();
    Ok(layout)
}
