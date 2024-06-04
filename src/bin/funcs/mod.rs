#![allow(dead_code)]
use nix::libc;
use regex::Regex;
use walkdir::WalkDir;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;
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
    let file = File::open("/root/arch-flux/selected_disk.cfg")?;
    let mut reader = BufReader::new(file);
    let mut contents = String::new();

    reader.read_to_string(&mut contents)?;

    if contents.is_empty() {
        eprintln!("Disk not found in /root/arch-flux/selected_disk.cfg, did you run the disk format utility, or forgot to input the disk manually?");
        std::process::exit(1);
    } else {
        let input = contents.replace("\n", ""); // Incase someone uses Vim to manually input the disk.

        let ssd = Regex::new(r"/dev/[s,v]d[a-z]").unwrap().find(&input);
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
    let file_path = "/root/arch-flux/user_selections.cfg";
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

pub fn replace_text (path: &str, old: &str, new: &str) -> io::Result<()> {
    let file_content = fs::read_to_string(path)?;
    let new_content = file_content.replace(old, new);
    fs::write(path, new_content)?;
    Ok(())
}

pub fn get_march() -> Result<String, String> {
    let output = Command::new("gcc")
        .args(&["-march=native", "-Q", "--help=target"])
        .output()
        .map_err(|e| e.to_string())?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    let re = Regex::new(r"-march=\s*([^\s]+)").map_err(|e| e.to_string())?;

    if let Some(caps) = re.captures(&output_str) {
        if let Some(march) = caps.get(1) {
            Ok(march.as_str().trim().to_string())
        } else {
            Err("CPU architecture not found".to_string())
        }
    } else {
        Err("Failed to match regex".to_string())
    }
}

// Copy files and directories recursively from src to dest.
pub fn copy_recursively(src: &Path, dest: &Path) -> anyhow::Result<()> {
    for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let src_path = entry.path();
        let relative_path = src_path.strip_prefix(src)?;
        let dest_path = dest.join(relative_path);

        if src_path.is_dir() {
            fs::create_dir_all(&dest_path)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&src_path, &dest_path)?;
        }
    }
    Ok(())
}
