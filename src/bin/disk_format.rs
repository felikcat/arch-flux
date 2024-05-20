use cryptsetup_rs::device::{deactivate, init};
use nix::libc::{self};
use nix::unistd::close;
use nix::{self, ioctl_read_bad};
use regex::Regex;
use std::fmt::{Display, Formatter, Result};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};

#[cfg(any(unix, target_os = "wasi"))]
use std::os::fd::{AsRawFd, RawFd};

mod funcs;

fn main() {
    let mut wrong_option = false;
    let mut wrong_disk = false;
    let mut selected_disk = "/dev/null".to_string();

    loop {
        disk_selection(&mut wrong_option, &mut wrong_disk, &mut selected_disk);
        if !wrong_disk && !wrong_disk {
            break;
        }
    }

    disk_editing(&mut selected_disk);
}

fn disk_selection(wrong_option: &mut bool, wrong_disk: &mut bool, selected_disk: &mut String) {
    // Clear terminal
    print!("{esc}c", esc = 27 as char);

    if *wrong_option {
        println!("NOTICE: Please enter 'y' or 'n'.\n");
    }
    if *wrong_disk {
        println!("NOTICE: An invalid disk has been selected, try again.\n")
    }

    *wrong_option = false;
    *wrong_disk = false;

    funcs::terminal("lsblk -o PATH,MODEL,PARTLABEL,FSTYPE,FSVER,SIZE,FSUSE%,FSAVAIL,MOUNTPOINTS");

    let input = funcs::prompt("\nExample disks: /dev/sda, /dev/nvme0n1.\nInput your desired disk, then press ENTER: ");

    let ssd = Regex::new(r"/dev/sd[a-z]").unwrap().find(&input);
    let nvme = Regex::new(r"/dev/(nvme|mmc)([0-9])n1").unwrap().find(&input);

    struct RegexMatch<'a>(&'a str); // Store

    impl<'a> Display for RegexMatch<'a> {
        fn fmt(&self, format: &mut Formatter) -> Result {
            write!(format, "{}", self.0)
        }
    }

    match ssd.or(nvme) {
        Some(match_str) => {
            let regex_match = RegexMatch(match_str.as_str());
            println!("\nSelected disk: {}\n", regex_match);
            *selected_disk = regex_match.to_string();
        }
        None => {
            *wrong_disk = true;
            return;
        }
    }

    let input = funcs::prompt("Are you sure [y/n]: ");

    match input.to_lowercase().as_ref() {
        "y" if input.len() == 1 => return,
        "n" if input.len() == 1 => disk_selection(wrong_option, wrong_disk, selected_disk),
        _ => {
            *wrong_option = true;
            return;
        }
    }
}

fn wipe_disk(device_path: &str) -> io::Result<()> {
    let mut device = OpenOptions::new().read(true).write(true).open(device_path)?;

    // https://github.com/strace/strace/blob/master/src/linux/64/ioctls_inc.h
    ioctl_read_bad!(blksszget, 0x1268, libc::c_int);

    let mut sector_size = 0;
    let fd: RawFd = device.as_raw_fd();

    // TODO: Add error handling
    let _ = unsafe { blksszget(fd, &mut sector_size) };

    let buffer = vec![0u8; sector_size.try_into().unwrap()];
    
    device.write_all(&buffer)?; // Write zeros to disk.
    device.sync_all()?; // Ensure writes are flushed to disk.

    let _ = close(fd).unwrap();

    println!("Sector size: {}", sector_size);

    Ok(())
}

fn disk_editing(selected_disk: &str) {
    // Close these two LUKS containers if opened prior.
    File::create("/dev/mapper/cleanit").unwrap();
    let path = init("/dev/mapper/cleanit").unwrap();
    let _ = deactivate(path, "cleanit");

    File::create("/dev/mapper/root").unwrap();
    let path = init("/dev/mapper/root").unwrap();
    let _ = deactivate(path, "root");

    wipe_disk(selected_disk);
}
